//! The biplane, drawn: freeride's other vehicle, its model and textures off the disc, the prop that
//! goes with the state it is in, the pilot in its seat, and the list of what it would emit.
//!
//! **Where it lives.** Everything the plane is drawn from is in `machine.res`: the model
//! `biplane\anim\biplane.psg`, the two prop cones `propconeblur.psg` and `propconebent.psg`, and the
//! textures under `biplane\tex\` (`pitts_body01`, `pitts_wing01`, `pitts_panel01`,
//! `pitts_windshield01`, `guns`, `biplaneprop`, `biplanepropbroken`, the `02` liveries and an `_ss`
//! material of each). A freeride track's second archive (`free06_2.res`) carries the model and the
//! two cones again, plus the plane's `.phy`, its camera presets and its five emitters — but none of
//! the textures, which is why the plane is loaded with `machine.res` behind the track's archives.
//!
//! **Which model.** The vehicle spawner picks one of four by a livery index the caller hands it:
//! 0 `biplane`, 1 `biplane2`, 2 `biplane_ss`, 3 `biplane2_ss` ([`MODELS`]). The four are the same
//! mesh — 24 nodes, five materials, 107,161 bytes each — and differ only in the materials they name:
//! `pitts_body01`/`pitts_wing01` against `pitts_body02`/`pitts_wing02`, and the `_ss` set, which is
//! the one-pass material of each for a split screen.
//!
//! **The prop.** It is not part of the plane's model. The plane loads the two cones as models of their
//! own and, every frame, puts one of them on the model's `PropDummy` node and hides the other: the
//! blurred cone while the plane is whole, the bent one once it has crashed ([`Biplane::parts`]). The
//! cones carry their own spinner material — `biplaneprop` on the blur, `biplanepropbroken` on the
//! bent one. The disc's material sort table (`startup.res`'s `appstart\mat_stbl.txt`) also lists a
//! `BiplaneProp02` and a `BiplanePropBroken2`, but no archive on the disc has a `.psm` or a `.pst` of
//! either name and no model names them, so there is nothing to swap them in from.
//!
//! **The pilot.** The plane names four nodes for a rider's hands and feet — `Dummy_Foot_L_pos`,
//! `Dummy_Foot_R_pos`, `Dummy_Hand_L_pos`, `Dummy_Hand_R_pos` — and caches them when it loads, the
//! same way a bike caches the four anchors on its grips and pegs. The body is the rider's own, posed by
//! the two clips every rider archive ships for the plane, `biplane_roll_fwd` and `biplane_roll_bck`,
//! with its limbs aimed at those four nodes ([`Biplane::anchors`], [`seat`]). Which of the two and
//! how far in is the stick's: the aileron picks how far into both clips, the elevator mixes them, each
//! eased by the body's own lag (`game_sim::biplane_anim::Lean`, S35 and V78).
//!
//! Unlike a bike's, the plane's clips do not carry the body into the seat: they are made about the
//! rider's own root, and the root is put [`PILOT_SEAT`] up and back from the plane's origin, turned
//! with it and not at all against it ([`Biplane::pilot`]). That offset was read off the original: on
//! all four of the biplane recording's states that were looked at — parked, banking, level, turning
//! — the root of the body's node tree sits at exactly (0, 3.5, -3.0) in the plane's space with the
//! plane's own rotation. The plane writes a (0, 3, -3) of its own at load; where the other half foot
//! of height comes from was not traced. With the clip's limbs from that root, the feet land half a
//! foot from the plane's foot anchors and the hands a fifth of a foot from its hand anchors.
//!
//! **The effects.** `tracks\biplaneparticles.dat` attaches the plane's emitters to its nodes; it is read
//! here ([`Emitter`]) so the list is the disc's own, and the shell's `particles.rs` emits and draws from
//! it (P58).

use crate::anim::Set;
use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::rider::Anchors;
use crate::vehicle::about;
use crate::world::Assets;
use game_sim::biplane_anim::{self, Surfaces};
use game_sim::motion::Animations;

/// The plane's model for each livery the spawner can be asked for, in the spawner's own order.
pub const MODELS: [&str; 4] = ["biplane", "biplane2", "biplane_ss", "biplane2_ss"];

/// The prop cone drawn while the plane is whole: a blurred disc of a spinning prop.
pub const PROP: &str = "propconeblur";
/// The prop cone drawn once the plane has crashed: the blades bent back.
pub const PROP_BROKEN: &str = "propconebent";
/// The node the cone in use is put on.
pub const PROP_NODE: &str = "PropDummy";

/// The archive the plane's textures are in. The model is in a freeride track's second archive as
/// well; the textures are only here.
pub const ARCHIVE: &str = "machine.res";

/// The four nodes the plane holds a rider's limbs to, in the order the rider's limb solver takes its
/// targets ([`crate::rider::LIMBS`]): the two feet, then the two hands.
pub const SEAT: [&str; 4] = ["Dummy_Foot_L_pos", "Dummy_Foot_R_pos", "Dummy_Hand_L_pos", "Dummy_Hand_R_pos"];

/// The pilot's two clips, which every rider archive carries beside the bike set: the body leaning
/// into a roll one way and the other.
pub const PILOT_CLIPS: [&str; 2] = ["biplane_roll_fwd", "biplane_roll_bck"];

/// Where the pilot's root goes, in the plane's own space: up and back from the plane's origin to the
/// cockpit. Read off the original (see the module docs), not out of a file.
pub const PILOT_SEAT: V3 = v3(0.0, 3.5, -3.0);

/// The emitter list the plane's particles are attached by.
pub const PARTICLES: &str = "biplaneparticles.dat";

/// A loaded plane: which model it is, and where on it the prop and the pilot go.
pub struct Biplane {
    /// The name the plane's model is loaded under in [`Assets`].
    pub model: String,
    /// `PropDummy`'s transform in the plane's own space, where the model has the node.
    pub prop: Option<M4>,
    /// Where the plane wants a pilot's feet and hands, in the pilot's own space — the plane's four
    /// seat nodes, less [`PILOT_SEAT`] — which is the space the body is skinned in.
    pub anchors: Anchors,
    /// `PropDummy`'s index among the model's nodes: where a [`Biplane::pose`] has the turned prop.
    pub prop_node: Option<usize>,
}

impl Biplane {
    /// Load livery `livery` of the plane and both prop cones into `assets`, from the first of
    /// `archives` to hold each file. `None` if the plane's model is not there.
    pub fn load(assets: &mut Assets, archives: &[Res], livery: usize) -> Option<Biplane> {
        let model = MODELS.get(livery).copied().unwrap_or(MODELS[0]);
        if !assets.load(archives, model) {
            return None;
        }
        // A plane with no cone still flies; it is drawn without one.
        assets.load(archives, PROP);
        assets.load(archives, PROP_BROKEN);
        let loaded = &assets.models[model].model;
        Some(Biplane {
            model: model.to_string(),
            prop: node(loaded, PROP_NODE),
            anchors: SEAT.map(|name| node(loaded, name).map(|m| m.point(v3(0.0, 0.0, 0.0)).sub(PILOT_SEAT))),
            prop_node: loaded.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(PROP_NODE)),
        })
    }

    /// What to draw for a plane placed at `at`: the plane, and the cone that goes with whether it
    /// has `crashed` on its `PropDummy` — which is the original's per-frame choice, made off the same
    /// flag its crash sets.
    ///
    /// The cone is put at the node's *bind* transform. The original turns `PropDummy` about its own
    /// axis by the engine's prop angle before it copies the node onto the cone; nothing in the port
    /// turns a prop yet, so the blur is drawn at the angle the model was built at.
    pub fn parts(&self, at: M4, crashed: bool) -> Vec<(&str, M4)> {
        let mut out = vec![(self.model.as_str(), at)];
        if let Some(prop) = self.prop {
            out.push((if crashed { PROP_BROKEN } else { PROP }, prop.mul(at)));
        }
        out
    }

    /// [`Biplane::parts`] with the prop turned (P31): `prop` is the engine's prop angle, or `None`
    /// once the plane is wrecked, as `game_sim::biplane_anim::Surfaces::prop` gives it. The surface
    /// step replaces `PropDummy`'s rotation with one about z by the angle and keeps its position
    /// (S35), and the blur is stood there; a wrecked prop is the bent cone at the bind transform.
    ///
    /// The replacement is made on the node's model-space transform, which is its local one only
    /// while its parents have no rotation of their own; that was not checked on the model.
    pub fn turned_parts(&self, at: M4, prop: Option<f32>) -> Vec<(&str, M4)> {
        let Some(angle) = prop else {
            return self.parts(at, true);
        };
        let mut out = vec![(self.model.as_str(), at)];
        if let Some(bind) = self.prop {
            let (s, c) = angle.sin_cos();
            let turned = M4([[c, s, 0.0, 0.0], [-s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], bind.0[3]]);
            out.push((PROP, turned.mul(at)));
        }
        out
    }

    /// Where to draw the pilot for a plane placed at `at`: its root in the seat, turned with the
    /// plane.
    pub fn pilot(&self, at: M4) -> M4 {
        let mut seat = M4::IDENTITY;
        seat.0[3] = [PILOT_SEAT.x, PILOT_SEAT.y, PILOT_SEAT.z, 1.0];
        seat.mul(at)
    }
}

impl Biplane {
    /// The plane's nodes posed by one frame's moving parts (V38), in the plane's own space and in
    /// the model's node order: the matrices the GPU skins the plane with and the software path skins
    /// its copy into ([`crate::skin::skin`]). Empty if the model is not in `assets`.
    pub fn pose(&self, assets: &Assets, surfaces: &Surfaces) -> Vec<M4> {
        assets.models.get(&self.model).map_or(Vec::new(), |l| pose(&l.model, surfaces))
    }

    /// [`Biplane::parts`] off a [`Biplane::pose`]: the plane, and the cone [`Surfaces::cone`] names
    /// stood at the posed `PropDummy` — turned by the prop while the plane flies, at its rest once
    /// it is wrecked, as the surface step leaves it.
    pub fn posed_parts(&self, at: M4, pose: &[M4], surfaces: &Surfaces) -> Vec<(&str, M4)> {
        let mut out = vec![(self.model.as_str(), at)];
        let prop = self.prop_node.and_then(|i| pose.get(i).copied()).or(self.prop);
        if let Some(prop) = prop {
            let cone = if surfaces.cone == biplane_anim::CONE_BENT { PROP_BROKEN } else { PROP };
            out.push((cone, prop.mul(at)));
        }
        out
    }

    /// A node of the plane's model in the plane's own space, by name: where an emitter of
    /// [`PARTICLES`] rides (P58). `None` if the model is not in `assets` or has no such node.
    pub fn node(&self, assets: &Assets, name: &str) -> Option<M4> {
        node(&assets.models.get(&self.model)?.model, name)
    }
}

/// The plane's model posed by the surface step (`game_sim::biplane_anim::surfaces`, S35): each node's
/// transform in the model's own space, the moving parts' local transforms changed the way the
/// original's step changes its bones, and everything else at the model's own.
///
/// - The four wing flaps ([`biplane_anim::FLAPS`]): the rest transform turned about its hinge,
///   `R(hinge, a) · rest` in the row-vector order, the position the rest's. The hinge is the rest's
///   first row, normalised — what S35's test stands in for the hinge the plane keeps beside it. A
///   zero angle is not turned.
/// - The rear flaps, the rudder and the prop ([`biplane_anim::REAR`], [`biplane_anim::RUDDER`],
///   [`biplane_anim::PROP`]): the rotation replaced by one about x, y and z, the position kept. A
///   wrecked prop is left at its rest.
///
/// The original changes the bones' own matrices, which on this model are their model-space ones too:
/// every moving part hangs straight off the root, unturned. The pose is composed through the parents
/// all the same, so a model built otherwise is still drawn the way its bones were changed.
pub fn pose(model: &Model, s: &Surfaces) -> Vec<M4> {
    let find = |name: &str| model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name));
    let mut local: Vec<M4> = model.nodes.iter().map(|n| n.local).collect();
    for (name, a) in biplane_anim::FLAPS.iter().zip(s.flaps) {
        if let (Some(i), true) = (find(name), a.0 != 0.0) {
            let rest = local[i];
            let hinge = v3(rest.0[0][0], rest.0[0][1], rest.0[0][2]).norm();
            local[i] = about(hinge, a.0).mul(rest);
        }
    }
    let x = v3(1.0, 0.0, 0.0);
    let y = v3(0.0, 1.0, 0.0);
    let z = v3(0.0, 0.0, 1.0);
    let mut turned: Vec<(&str, V3, f32)> = biplane_anim::REAR.iter().map(|&n| (n, x, s.rear.0)).collect();
    turned.push((biplane_anim::RUDDER, y, s.rudder.0));
    if let Some(prop) = s.prop {
        turned.push((biplane_anim::PROP, z, prop.0));
    }
    for (name, axis, a) in turned {
        if let Some(i) = find(name) {
            let r = about(axis, a);
            for row in 0..3 {
                local[i].0[row][..3].copy_from_slice(&r.0[row][..3]);
            }
        }
    }
    (0..model.nodes.len())
        .map(|node| {
            let (mut m, mut at, mut guard) = (M4::IDENTITY, Some(node), 0);
            while let Some(i) = at.filter(|&i| i < local.len() && guard <= local.len()) {
                m = m.mul(local[i]);
                at = model.nodes[i].parent;
                guard += 1;
            }
            m
        })
        .collect()
}

/// One drawn frame's moving parts between two ticks' (V79): `was` the tick before, `now` the last,
/// `t` how far the frame is from one to the other, as the plane's placement is slid.
///
/// The original eases its surfaces with the rate limits of the plane's control law
/// (`game_sim::biplane::controls`: the ailerons at 9 rad/s, the elevator at 9 a second, the rudder at
/// 1 rad/s, `biplane.phy`), a straight ramp a tick at a time, and draws each 30 Hz tick as it is —
/// the ailerons step 0.3 rad a tick. The picture here runs above the tick rate, so the ramp is drawn
/// between its ticks too: each angle straight from `was` to `now`, which is the ramp the control law
/// makes, cut finer. The prop goes the short way round, since the engine wraps its angle and turns it
/// under half a turn a tick (0.76 rad in the human's flight). A wrecked prop (`None` either side) and
/// the cone are `now`'s: a crash is a switch, not a move.
pub fn between(was: &Surfaces, now: &Surfaces, t: f32) -> Surfaces {
    use ps2_float::F;
    let t = t.clamp(0.0, 1.0);
    let f = |a: F, b: F| F(a.0 + (b.0 - a.0) * t);
    let prop = match (was.prop, now.prop) {
        (Some(a), Some(b)) => {
            let turn = std::f32::consts::TAU;
            let d = (b.0 - a.0).rem_euclid(turn);
            let d = if d > turn / 2.0 { d - turn } else { d };
            Some(F(b.0 - d * (1.0 - t)))
        }
        _ => now.prop,
    };
    Surfaces {
        flaps: std::array::from_fn(|i| f(was.flaps[i], now.flaps[i])),
        rear: f(was.rear, now.rear),
        rudder: f(was.rudder, now.rudder),
        prop,
        cone: now.cone,
    }
}

/// A node's transform in its model's own space, by name, ignoring case as the engine's lookup does.
fn node(model: &Model, name: &str) -> Option<M4> {
    let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
    Some(model.world(i))
}

/// Pose a rider's body as the pilot, as the original's two-clip selection plays the plane's record
/// (`game_sim::biplane_anim::Lean`, V78): `biplane_roll_bck` sampled at `phase` of its own length,
/// `biplane_roll_fwd` at the same second, and the two mixed by `mix` — 1 all of the back roll, 0 all
/// of the forward one, the way the bike's two-clip blends are drawn ([`Set::apply`]). A centred stick
/// is `(0.5, 0.5)`. Loads the clips into the body's own set on first use. False if either clip is not
/// in the rider's archive, and the pose is then left alone.
pub fn seat(set: &mut Set, phase: f32, mix: f32) -> bool {
    for name in PILOT_CLIPS {
        set.load(name);
    }
    let (Some(fwd), Some(bck)) = (set.id(PILOT_CLIPS[0]), set.id(PILOT_CLIPS[1])) else {
        return false;
    };
    let (fwd, bck) = (fwd.0 as usize, bck.0 as usize);
    let (a, b) = (&set.clips[bck], &set.clips[fwd]);
    a.clip.pose_mixed(&b.clip, phase * a.clip.duration(), mix, &a.joints, &mut set.pose);
    true
}

/// The pilot's node matrices for a body that also rides a bike (V39): [`seat`]ed at the lean's
/// `(phase, mix)` (V78), held on to the plane's `anchors` at full strength with both feet on, and the
/// body's own pose, hold and feet put back afterwards, so the bike's rider takes up where they left
/// off when the player switches back. In the body's own space, which [`Biplane::pilot`] places in the
/// world; the GPU draws them as they are, the software path skins them ([`crate::skin::skin`]).
/// `None` if the rider's archive has no [`PILOT_CLIPS`].
pub fn pilot_palette(body: &mut crate::rider::Body, anchors: &Anchors, (phase, mix): (f32, f32)) -> Option<Vec<M4>> {
    let (pose, hold, feet) = (body.rig.set.pose.clone(), body.hold, body.feet);
    body.hold = 1.0;
    body.feet = Default::default();
    let palette = seat(&mut body.rig.set, phase, mix).then(|| body.palette(Some(anchors)).0);
    body.rig.set.pose = pose;
    body.hold = hold;
    body.feet = feet;
    palette
}

/// One emitter the plane attaches: a section of `biplaneparticles.dat`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Emitter {
    /// The section's own name: `Left Exhaust`, `Front Left Tire Dust`.
    pub name: String,
    /// `Exhaust`, `Dust`, `Generic`, `HelicopterDust`, `CollisionModel`.
    pub kind: String,
    /// The `.emt` the particles are described by, as the file spells it.
    pub emitter: String,
    /// The node of the plane's model it rides on, if it rides on one; the wreck's two ride on the
    /// collision model instead.
    pub node: Option<String>,
    /// When it is switched off: `Wrecking`, `NoContact | ObjectContact`, and so on, as written.
    pub disable: String,
    /// An exhaust's `MaxSpeed`: the airspeed, feet a second, at which it stops (P58). 0 where the
    /// entry does not say.
    pub max_speed: f32,
    /// `HasCollision`: the collision model's particles meet the ground (P61). The wreck's chunks say
    /// `TRUE`; off where the entry does not say.
    pub has_collision: bool,
    /// Every `key = value` of the section, as written: the keys a type reads for itself (a tire
    /// dust's `AttachedToTire` and `MinCompressionThreshold`, the prop wash's fade heights, P60).
    pub keys: Vec<(String, String)>,
}

impl Emitter {
    /// A key of the section, ignoring case.
    pub fn key(&self, key: &str) -> Option<&str> {
        self.keys.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v.as_str())
    }
}

/// Read the emitter list: one [`Emitter`] per `[section]`, in the file's order.
pub fn emitters(text: &str) -> Vec<Emitter> {
    let mut out: Vec<Emitter> = Vec::new();
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            out.push(Emitter { name: name.to_string(), ..Emitter::default() });
            continue;
        }
        let (Some((key, value)), Some(e)) = (line.split_once('='), out.last_mut()) else {
            continue;
        };
        let value = value.trim().to_string();
        e.keys.push((key.trim().to_string(), value.clone()));
        match key.trim().to_ascii_lowercase().as_str() {
            "type" => e.kind = value,
            "emitter" => e.emitter = value,
            "attachedtosoultree" => e.node = Some(value),
            "disable" => e.disable = value,
            "maxspeed" => e.max_speed = value.parse().unwrap_or(0.0),
            "hascollision" => e.has_collision = crate::ini::switch(&value),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "\
; [Your Emitter Name]
; Type = Exhaust | Dust
[Vehicle Dust]
Type = CollisionModel
AttachedToCollisionModel = 0
Disable = NoContact | ObjectContact
Emitter = Biplanewreckdust.emt
[Left Exhaust]
Type = Exhaust
AttachedToSoultree = Biplane_Exhaust_L
Disable = Wrecking
Emitter = BiplaneExhaust.emt
MaxSpeed = 35.0
";

    /// A ramp the control law makes over two ticks, drawn four frames a tick (V79): every frame moves
    /// each surface a quarter of the tick's step, the same way, so nothing jumps between frames; the
    /// frame on a tick is that tick; the prop goes the short way across its wrap; a crash switches.
    #[test]
    fn frames_between_ticks_glide_along_the_ramp() {
        use ps2_float::F;
        let tick = |aileron: f32, elevator: f32, rudder: f32, prop: f32| {
            biplane_anim::surfaces(F(elevator), F(0.5236), F(aileron), F(rudder), F(prop), false)
        };
        let ticks = [tick(0.0, 0.0, 0.0, 5.9), tick(0.3, 0.3, 0.0333, 0.375), tick(0.6, 0.6, 0.0667, 1.133)];
        let mut frames = Vec::new();
        for pair in ticks.windows(2) {
            for k in 0..4 {
                frames.push(between(&pair[0], &pair[1], k as f32 / 4.0));
            }
        }
        frames.push(ticks[2]);
        let wrap = |d: f32| (d + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
        for (k, f) in frames.windows(2).enumerate() {
            let (a, b) = (&f[0], &f[1]);
            assert!((b.flaps[0].0 - a.flaps[0].0 - 0.075).abs() < 1e-5, "frame {k}: flap {:?} -> {:?}", a.flaps, b.flaps);
            assert!((b.flaps[2].0 - a.flaps[2].0 + 0.075).abs() < 1e-5, "frame {k}: right flap");
            assert!((b.rear.0 - a.rear.0 + 0.075 * 0.5236).abs() < 1e-5, "frame {k}: rear flap");
            assert!((b.rudder.0 - a.rudder.0 + 0.0333 / 4.0).abs() < 1e-3, "frame {k}: rudder");
            let turned = wrap(b.prop.unwrap().0 - a.prop.unwrap().0);
            assert!(turned > 0.15 && turned < 0.2, "frame {k}: the prop turns on, {turned}");
        }
        assert_eq!(between(&ticks[0], &ticks[1], 1.0), ticks[1]);
        assert_eq!(between(&ticks[0], &ticks[1], 0.0).flaps, ticks[0].flaps);
        let wrecked = biplane_anim::surfaces(F(0.6), F(0.5236), F(0.6), F(0.0), F(1.0), true);
        let shown = between(&ticks[1], &wrecked, 0.5);
        assert_eq!((shown.prop, shown.cone), (None, biplane_anim::CONE_BENT));
    }

    #[test]
    fn the_emitter_list_reads_one_entry_a_section() {
        let list = emitters(LIST);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Vehicle Dust");
        assert_eq!(list[0].node, None);
        assert_eq!(list[0].disable, "NoContact | ObjectContact");
        assert_eq!(list[1].kind, "Exhaust");
        assert_eq!(list[1].emitter, "BiplaneExhaust.emt");
        assert_eq!(list[1].node.as_deref(), Some("Biplane_Exhaust_L"));
    }
}
