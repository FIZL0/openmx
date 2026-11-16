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
    /// The quad's own clips (`ATV_Bike_*`), by the name after the prefix, lowercase.
    pub machine_clips: Vec<(String, Clip)>,
    /// MD138a: the rider's stunt table as the quad's clips read it ([`stunts`]), each row's clip in
    /// [`Look::machine_clips`] and its rider's clip in [`Look::clips`] (`None` for none), and the
    /// riding pose's three clips (fore, middle, back) off the build's motion set.
    pub stunts: Vec<quad::Stunt>,
    pub stunt_clips: Vec<Option<usize>>,
    pub rider_stunts: Vec<Option<usize>>,
    pub pose_clips: Vec<Clip>,
}

/// The stunts the original's quad has a row for: the table's first 46, the ids below its riding pose.
const STUNTS: usize = game_sim::quad::RIDING_POSE as usize;

/// The quad's clip table off the pack's stunt table (MD138a), as the original binds it: a row of the
/// first [`STUNTS`] marked `UsedForBike` takes the quad's `ATV_Bike_<Name>` when the pack has it,
/// and one with `HasAnimation` the rider's `ATV_rider_<Name>`. The rows, and each one's place in
/// `machine` and in `rider` (the motion sets' clips by name).
pub fn stunts(rows: &[game_sim::trick_pack::Row], machine: &[(String, Clip)], rider: &[(String, Clip)]) -> (Vec<quad::Stunt>, Vec<Option<usize>>, Vec<Option<usize>>) {
    let (mut table, mut on_quad, mut on_rider) = (Vec::new(), Vec::new(), Vec::new());
    for r in rows.iter().take(STUNTS) {
        let name = &r.stunt.name;
        let own = r.stunt.for_bike.then(|| machine.iter().position(|(n, _)| n.eq_ignore_ascii_case(name))).flatten();
        let ridden = r.has_animation.then(|| rider.iter().position(|(n, _)| n.eq_ignore_ascii_case(&format!("ATV_rider_{name}")))).flatten();
        table.push(quad::Stunt { name: name.clone(), duration: r.stunt.duration.0, machine: own.map(|i| machine[i].1.duration()) });
        on_quad.push(own);
        on_rider.push(ridden);
    }
    (table, on_quad, on_rider)
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
    let (stunts, stunt_clips, rider_stunts) = stunts(&pack.stunts()?, &machine_clips, &clips);
    let pose_clips = pack
        .build_clips(&build.mcf)?
        .into_iter()
        .filter(|(n, _)| n.to_ascii_lowercase().starts_with("atv_bike_air_"))
        .map(|(_, c)| c)
        .collect();
    Ok(Look { sub: build.sub.clone(), chassis, parts, clips, machine_clips, stunts, stunt_clips, rider_stunts, pose_clips })
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

/// The chassis posed by `wheels` over its clip ([`chassis_locals`], [`quad::pose_from`]); empty if
/// it is not in `assets`.
pub fn pose(look: &Look, assets: &Assets, wheels: &[Tire; 4], atv: &AtvRide) -> Vec<M4> {
    assets.models.get(&look.chassis).map_or(Vec::new(), |l| quad::pose_from(&l.model, chassis_locals(look, &l.model, atv), wheels, bars(wheels)))
}

/// The quad's clip by its place in the clip table ([`quad::clip_of`]).
fn clip(look: &Look, id: i32) -> Option<&Clip> {
    quad::clip_of(id, |i| look.stunt_clips.get(i).copied().flatten().map(|k| &look.machine_clips[k].1), |k| look.pose_clips.get(k))
}

/// The chassis' locals as this frame's clip leaves them (MD138a, [`quad::Driver::played`]), faded
/// from the one before the last stop over the model's own: the model's own with no clip.
pub fn chassis_locals(look: &Look, model: &game_render::psg::Model, atv: &AtvRide) -> Vec<M4> {
    let skeleton = game_render::skin::Skeleton::of(model);
    let rest = skeleton.nodes(&game_render::skin::Pose::new(model.nodes.len()));
    let driver = atv.ridden.as_ref().and_then(|q| q.clips.as_ref());
    let Some(played) = driver.and_then(|d| d.played) else { return model.nodes.iter().map(|n| n.local).collect() };
    let under = driver.and_then(|d| d.under).and_then(|u| quad::lay(u, &skeleton, &rest, |id| clip(look, id))).map_or(rest.clone(), |p| skeleton.nodes(&p));
    match quad::lay(played, &skeleton, &under, |id| clip(look, id)) {
        Some(p) => skeleton.locals(&p),
        None => model.nodes.iter().map(|n| n.local).collect(),
    }
}

/// The names of this frame's clips, for the trace: the chassis' (`ATV_Bike_<name>` or the riding
/// pose's) and the rider's stunt's.
pub fn clip_names(look: &Look, atv: &AtvRide) -> (String, String) {
    let Some(d) = atv.ridden.as_ref().and_then(|q| q.clips.as_ref()) else { return ("none".into(), "none".into()) };
    let name = |id: i32| -> String {
        let p = game_sim::quad::POSE_CLIPS;
        match d.name(id) {
            Some(n) if clip(look, id).is_some() => format!("ATV_Bike_{n}"),
            _ if (p[0]..=p[2]).contains(&id) => ["ATV_Bike_Air_Fwd", "ATV_Bike_Air_Mid", "ATV_Bike_Air_Bck"][(id - p[0]) as usize].into(),
            _ => "none".into(),
        }
    };
    let chassis = d.played.map_or("none".into(), |p| name(p.clip()));
    let rider = rider_stunt(look, atv).map_or("none".into(), |(n, _)| n.to_owned());
    (chassis, rider)
}

/// The rider's stunt clip playing (MD138a): its name in [`Look::clips`] and how far into it.
pub fn rider_stunt<'a>(look: &'a Look, atv: &AtvRide) -> Option<(&'a str, f32)> {
    let (id, t) = atv.ridden.as_ref()?.clips.as_ref()?.rider?;
    let k = (*look.rider_stunts.get(usize::try_from(id).ok()?)?)?;
    Some((look.clips[k].0.as_str(), t))
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
pub fn actors<'a>(look: &'a Look, assets: &Assets, wheels: &[Tire; 4], atv: &AtvRide, at: M4, alpha: f32, poses: &mut Vec<Vec<M4>>) -> Vec<Actor<'a>> {
    let pose = pose(look, assets, wheels, atv);
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
pub fn skinned<'a>(look: &'a Look, copy: &'a str, assets: &mut Assets, wheels: &[Tire; 4], atv: &AtvRide, at: M4) -> Vec<(&'a str, M4)> {
    let pose = pose(look, assets, wheels, atv);
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

/// Which of the motion set's rows the rider is on, and where on it: the original's choice (MD165,
/// [`quad::Motion`]) — the row it is mostly on by its middle clip (`none` on a stunt, the flail or
/// the pause; the reverse pair by its leading clip), the eased lean (where in the clips, a half at
/// rest) and the eased fore and aft (0 forward, a half at rest). The stopped row at rest for a quad
/// not out or without its clips.
pub fn row(atv: &AtvRide) -> (&'static str, f32, f32) {
    match motion(atv) {
        Some(m) => (m.row().unwrap_or("none"), m.phase, m.mix),
        None => (quad::STOPPED[1], 0.5, 0.5),
    }
}

/// The rider's motion, while the quad is out with its clips.
pub fn motion(atv: &AtvRide) -> Option<&quad::Motion> {
    Some(&atv.ridden.as_ref()?.clips.as_ref()?.motion)
}

/// The length of the get-ready pause's clip in the look's motion set (0 without it).
pub fn pause_length(look: &Look) -> f32 {
    look.clips.iter().find(|(n, _)| n.eq_ignore_ascii_case(quad::PAUSE_CLIP)).map_or(0.0, |(_, c)| c.duration())
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
    let posed = quad::pose_from(loaded, chassis_locals(look, loaded, atv), &wheels, bars(&wheels));
    let anchors = game_render::rider::anchors_of(&game_render::skin::Skeleton::of(loaded), &posed);
    let stunt = rider_stunt(look, atv);
    match motion(atv) {
        Some(m) => quad::rider_palette_motion(body, &anchors, m, stunt).or_else(|| quad::rider_palette_stunt(body, &anchors, quad::STOPPED, 0.5, 0.0, stunt)),
        None => quad::rider_palette_stunt(body, &anchors, quad::STOPPED, 0.5, 0.0, stunt),
    }
}

/// `--trace-picture`'s line for the quad drawn this frame: the renderer, the chassis, where its
/// origin stands, each wheel's spin/steer/compression, the parts drawn on it and the row the rider is
/// on.
pub fn trace(p: usize, renderer: &str, look: &Look, at: M4, wheels: &[Tire; 4], atv: &AtvRide) {
    let o = at.0[3];
    let w: Vec<String> = wheels.iter().map(|t| format!("{:.2}/{:.2}/{:.2}", t.spin, t.steer, t.compression)).collect();
    let (row, phase, fore_aft) = row(atv);
    let (chassis, stunt) = clip_names(look, atv);
    println!(
        "picture atv: seat {p} {renderer} {} at {:.2} {:.2} {:.2} wheels {} parts {} row {} {:.2} {:.2} clip {chassis} stunt {stunt}",
        look.chassis,
        o[0],
        o[1],
        o[2],
        w.join(" "),
        look.parts.len(),
        row,
        phase,
        fore_aft
    );
}
