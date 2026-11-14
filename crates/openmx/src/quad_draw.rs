//! The quad as the shell draws it (MD138): one build's chassis model (a `.sub` of the pack's, its
//! parts and its textures) at the quad's matrix ([`crate::vehicle::AtvRide::matrix`]), its wheels,
//! hubs, A-arms, struts and bars posed from the step (`game_render::quad::pose`), and the rider on
//! it, posed by the pack's ATV motion set and held to the chassis' own pegs and grips. The quad rode
//! undrawn from MD37d to here. Offline only, as the quad is.

use game_render::car::Tire;
use game_render::gpu::Actor;
use game_render::mat::M4;
use game_render::quad;
use game_render::scene::Part;
use game_render::skin::Clip;
use game_render::world::Assets;

use crate::machine_packs::{Build, Pack};
use crate::vehicle::AtvRide;

/// What one quad is drawn with, loaded once when the quad is picked.
pub struct Look {
    /// The build's `.sub`, as the pack lists it.
    pub sub: String,
    /// The chassis model's key in [`Assets`], and the parts on it.
    pub chassis: String,
    pub parts: Vec<Part>,
    /// The ATV motion set's clips by name, which [`rider`] binds into the rider's own set.
    pub clips: Vec<(String, Clip)>,
    /// The quad's own clips (`ATV_Bike_*`), by the name after the prefix: read, and played by
    /// nothing yet — the quad starts no stunt (MD123).
    pub machine_clips: Vec<(String, Clip)>,
}

/// The build a quad of `phy` is drawn as: the first of the pack's whose chassis is the one
/// [`crate::vehicle::atv_chassis`] rides it on, a player's (`ATV_…`, not an AI's or a ghost's)
/// where there is one.
pub fn build<'a>(pack: &'a Pack, phy: &str) -> Option<&'a Build> {
    let chassis = crate::vehicle::atv_chassis(phy).0.trim_end_matches(".psg");
    let fits: Vec<&Build> = pack.builds.iter().filter(|b| b.chassis.eq_ignore_ascii_case(chassis)).collect();
    let stem = |b: &Build| b.sub.rsplit(['\\', '/']).next().unwrap_or("").to_ascii_lowercase();
    let player = |b: &&Build| {
        let s = stem(b);
        s.starts_with("atv_") && !s.contains("_ai") && !s.contains("ghost")
    };
    fits.iter().copied().find(player).or_else(|| fits.first().copied())
}

/// Load the quad of `atv` (its pack among `packs`) into `assets`: the build's chassis and parts
/// repainted by its `.sub`, and the rider's and the quad's clips. A line for the log either way.
pub fn load(assets: &mut Assets, packs: &[Pack], atv: &AtvRide) -> Result<Look, String> {
    let pack = packs.iter().find(|p| p.id == atv.pack).ok_or_else(|| format!("no pack {}", atv.pack))?;
    let build = build(pack, &atv.phy).ok_or_else(|| format!("{} has no build on {}", pack.id, crate::vehicle::atv_chassis(&atv.phy).0))?;
    let vehicle = pack.vehicle(build)?;
    let res = std::slice::from_ref(&pack.files);
    let chassis = format!("quad:{}", build.chassis.to_ascii_lowercase());
    if !assets.load_repainted(res, &build.chassis, &chassis, &vehicle.materials) {
        return Err(format!("{}: {} did not load", build.sub, build.chassis));
    }
    let parts = vehicle
        .parts
        .iter()
        .filter_map(|part| {
            let key = format!("quad:{}", part.model.to_ascii_lowercase());
            assets.load_repainted(res, &part.model, &key, &vehicle.materials).then(|| Part { model: key, placement: part.placement.clone() })
        })
        .collect();
    let clips = pack.rider_clips()?.0;
    let machine_clips = pack.machine_clips()?;
    Ok(Look { sub: build.sub.clone(), chassis, parts, clips, machine_clips })
}

/// The four wheels as the rig reads them off the quad out: each wheel's spin angle, its steer and its
/// suspension's compression. At rest for a quad not out.
pub fn wheels(atv: &AtvRide) -> [Tire; 4] {
    let Some(q) = atv.ridden.as_ref() else { return [Tire::REST; 4] };
    let b = &q.ride.bike;
    std::array::from_fn(|k| Tire { spin: b.spins[k].angle.0, steer: b.wheels[k].steer_angle.0, compression: b.wheels[k].compression.0, scale: 1.0 })
}

/// How far the bars turn: the front wheels' mean steer.
pub fn bars(wheels: &[Tire; 4]) -> f32 {
    (wheels[0].steer + wheels[1].steer) / 2.0
}

/// Where the quad out is drawn: its matrix, the model's placement (rows x, y, z, origin).
pub fn placed(atv: &AtvRide) -> Option<M4> {
    let m = atv.matrix()?;
    Some(M4(std::array::from_fn(|r| std::array::from_fn(|c| m.0[r].0[c]))))
}

/// The chassis posed by `wheels` ([`quad::pose`]); empty if it is not in `assets`.
pub fn pose(look: &Look, assets: &Assets, wheels: &[Tire; 4]) -> Vec<M4> {
    assets.models.get(&look.chassis).map_or(Vec::new(), |l| quad::pose(&l.model, wheels, bars(wheels)))
}

/// Where each part hangs: its placement node's posed matrix, or the chassis' origin for `Bike` (or a
/// node the chassis lacks).
fn part_at(look: &Look, assets: &Assets, pose: &[M4], part: &Part, at: M4) -> M4 {
    let node = assets.models.get(&look.chassis).and_then(|l| l.model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(&part.placement)));
    match node.and_then(|i| pose.get(i)) {
        Some(m) => m.mul(at),
        None => at,
    }
}

/// The quad placed at `at` as the GPU draws it: the chassis skinned by its pose, pushed on the end of
/// `poses`, which the actor names, and each part on its node. A machine casts no shadow.
pub fn actors<'a>(look: &'a Look, assets: &Assets, wheels: &[Tire; 4], at: M4, alpha: f32, poses: &mut Vec<Vec<M4>>) -> Vec<Actor<'a>> {
    let pose = pose(look, assets, wheels);
    let mut out: Vec<Actor<'a>> = look
        .parts
        .iter()
        .map(|part| Actor { model: part.model.as_str(), at: part_at(look, assets, &pose, part, at), pose: None, lod_from: None, alpha, casts: false })
        .collect();
    let posed = (!pose.is_empty()).then(|| {
        poses.push(pose);
        poses.len() - 1
    });
    out.push(Actor { model: look.chassis.as_str(), at, pose: posed, lod_from: None, alpha, casts: false });
    out
}

/// The quad as the software path draws it: the chassis skinned by its pose into `copy`, the seat's
/// own copy (made here the first time), and the parts on their nodes, as [`crate::car_draw::skinned`]
/// the cars.
pub fn skinned<'a>(look: &'a Look, copy: &'a str, assets: &mut Assets, wheels: &[Tire; 4], at: M4) -> Vec<(&'a str, M4)> {
    let pose = pose(look, assets, wheels);
    let mut out: Vec<(&'a str, M4)> = look.parts.iter().map(|part| (part.model.as_str(), part_at(look, assets, &pose, part, at))).collect();
    if !assets.models.contains_key(copy)
        && let Some(own) = assets.models.get(&look.chassis).cloned()
    {
        assets.models.insert(copy.to_string(), own);
    }
    match (assets.models.get_mut(copy), pose.is_empty()) {
        (Some(loaded), false) => {
            loaded.tris = game_render::skin::skin(&loaded.model, &pose, 0);
            out.push((copy, at));
        }
        _ => out.push((look.chassis.as_str(), at)),
    }
    out
}

/// Which of the motion set's rows the rider is on, and where on it: the air's while the quad is in
/// the air, the stopped one's under a foot a second, else the riding one's; the turn's lean the
/// stick's sideways (0 left, 1 right), the fore and aft the stick's (forward -1). The original's
/// choice is not ported (MD138's journal): this is the port's, matched by eye.
pub fn row(atv: &AtvRide) -> ([&'static str; 3], f32, f32) {
    let Some(q) = atv.ridden.as_ref() else { return (quad::STOPPED, 0.5, 0.0) };
    let q = &q.ride.q;
    let row = match (q.airborne, q.speed_len.0 < 1.0) {
        (true, _) => quad::AIR,
        (false, true) => quad::STOPPED,
        (false, false) => quad::RIDING,
    };
    (row, 0.5 + 0.5 * q.lean.0.clamp(-1.0, 1.0), q.lean_fore_aft.0.clamp(-1.0, 1.0))
}

/// The rider's node matrices on the quad, in the quad's space (the clips' own), held to the posed
/// chassis' pegs and grips: [`quad::rider_palette`] with the motion set bound into the body's set
/// first. `None` for a body without the rows or a chassis not loaded.
pub fn rider(look: &Look, body: &mut game_render::rider::Body, atv: &AtvRide, assets: &Assets) -> Option<Vec<M4>> {
    for (name, clip) in &look.clips {
        quad::bind(&mut body.rig.set, name, clip);
    }
    let loaded = &assets.models.get(&look.chassis)?.model;
    let wheels = wheels(atv);
    let posed = quad::pose(loaded, &wheels, bars(&wheels));
    let anchors = game_render::rider::anchors_of(&game_render::skin::Skeleton::of(loaded), &posed);
    let (row, phase, fore_aft) = row(atv);
    quad::rider_palette(body, &anchors, row, phase, fore_aft)
}

/// `--trace-picture`'s line for the quad drawn this frame: the renderer, the chassis, where its
/// origin stands, each wheel's spin/steer/compression, the parts drawn on it and the row the rider is
/// on.
pub fn trace(p: usize, renderer: &str, look: &Look, at: M4, wheels: &[Tire; 4], atv: &AtvRide) {
    let o = at.0[3];
    let w: Vec<String> = wheels.iter().map(|t| format!("{:.2}/{:.2}/{:.2}", t.spin, t.steer, t.compression)).collect();
    let (row, phase, fore_aft) = row(atv);
    println!(
        "picture atv: seat {p} {renderer} {} at {:.2} {:.2} {:.2} wheels {} parts {} row {} {:.2} {:.2}",
        look.chassis,
        o[0],
        o[1],
        o[2],
        w.join(" "),
        look.parts.len(),
        row[1],
        phase,
        fore_aft
    );
}
