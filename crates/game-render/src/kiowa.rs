//! The Kiowa, drawn (V113): the helicopter's model in each of its skins, its main and tail rotors
//! turned by the rotor's angle, the blurred blades while the rotor is whole and the bent tail rotor
//! once it is wrecked, and the name of its camera presets.
//!
//! **Where it lives.** All of it is in `machine.res`: the model `helicopter\anim\kiowa.psg` and its
//! three other skins, the blur and bent parts beside it (`bladeblur`, `bladebent`, `tailrotorblur`,
//! `tailrotorbent`), and the textures under `helicopter\tex\` (`kiowa_body01`, `kiowa_tail01`,
//! `kiowa_windows01`, the `02` livery, the `_ss` materials and `kiowa_blades`). The camera presets
//! are in each freeride track's second archive (`free09_2.res`'s `appstart\helocpresets.ini`, the
//! file the helicopter's camera object reads as `AppStart\HeloCPresets.ini`), in the bike's and the
//! plane's format.
//!
//! **Which model.** The helicopter is built in one of four skins by the event's skin index: 0
//! `Kiowa`, 1 `Kiowa2`, 2 `Kiowa_ss`, 3 `Kiowa2_ss` ([`MODELS`]; a fifth index hangs the original's
//! load). They are one mesh naming different materials: `kiowa_body02`/`kiowa_tail02` for the
//! second, the one-pass `_ss` set for a split screen.
//!
//! **The rotors.** The model carries the hub and the rotor head but no blades. The helicopter's
//! init loads the blades as models of their own and keeps them beside the model's nodes:
//!
//! | parts | at | shown |
//! |---|---|---|
//! | four `bladeblur` | `Blade_Node01`..`04` | while the main rotor is whole |
//! | `tailrotorblur` | `TailRotor_Node` | while the helicopter is whole |
//! | `tailrotorbent` | `TailRotor_Node` | once it is wrecked |
//! | four `bladebent` | thrown loose (rigid bodies of their own) | once the blades are thrown |
//!
//! Every frame the MX helicopter's measure runs the rotor's angle on at 27 radians a second
//! (`game_sim::kiowa::spin_blades`), replaces `RotorSpindle`'s rotation with a turn of that angle
//! about y and `TailRotor_Node`'s with the same angle about x, and then copies each blade node's
//! world matrix onto the blade model hung there ([`Kiowa::pose`], [`Kiowa::parts`]). The wreck hides
//! the tail's blur and shows its bent one, and, unless the blades were thrown already, throws the
//! four main blades, which hides their blurs; the put-back shows the blurs again and hides the bent
//! tail ([`Rotor`]). While the main blades are hidden the angle stops and nothing is turned, but the
//! bent tail is still put on its node.
//!
//! There is no swap by speed: the blur models are the only blades the whole rotor is ever drawn
//! with, the rotor turning at its one rate from the moment the helicopter is built.
//!
//! The blades thrown are not drawn here: where they fly is the wreck's (S220a1).

use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::vehicle::about;
use crate::world::Assets;

/// The helicopter's model for each skin, in the spawner's own order.
pub const MODELS: [&str; 4] = ["kiowa", "kiowa2", "kiowa_ss", "kiowa2_ss"];

/// One main blade, blurred: hung on each of [`BLADES`].
pub const BLADE_BLUR: &str = "bladeblur";
/// One main blade, bent: what the wreck throws, four of them.
pub const BLADE_BENT: &str = "bladebent";
/// The tail rotor, blurred.
pub const TAIL_BLUR: &str = "tailrotorblur";
/// The tail rotor, bent, once wrecked.
pub const TAIL_BENT: &str = "tailrotorbent";

/// The node the main rotor turns about, by the rotor's angle about y.
pub const SPINDLE: &str = "RotorSpindle";
/// The four nodes the main blades hang on, under the spindle.
pub const BLADES: [&str; 4] = ["Blade_Node01", "Blade_Node02", "Blade_Node03", "Blade_Node04"];
/// The tail rotor's node, turned by the same angle about x, which both tail parts hang on.
pub const TAIL: &str = "TailRotor_Node";

/// The helicopter's camera presets, in a freeride track's second archive.
pub const PRESETS: &str = "helocpresets.ini";

/// The archive the helicopter's models and textures are in.
pub const ARCHIVE: &str = "machine.res";

/// The four nodes the helicopter holds a pilot's limbs to: the two feet, then the two hands, as
/// the plane's ([`crate::biplane::SEAT`]).
pub const SEAT: [&str; 4] = ["Dummy_Foot_L_pos", "Dummy_Foot_R_pos", "Dummy_Hand_L_pos", "Dummy_Hand_R_pos"];

/// What the rotors are showing: the angle they are turned by and which of the parts are up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rotor {
    /// The rotor's angle, radians: `RotorSpindle` turns by it about y and `TailRotor_Node` about x.
    pub angle: f32,
    /// The main blades' blurs are up: false once they are thrown.
    pub blades: bool,
    /// The tail is the bent one: from the wreck to the put-back.
    pub wrecked: bool,
}

impl Rotor {
    /// The rotor as the helicopter is built: whole, at its rest angle.
    pub const WHOLE: Rotor = Rotor { angle: 0.0, blades: true, wrecked: false };

    /// One drawn frame's rotor between two ticks': the angle run on straight from `was` to `now` by
    /// `t`, the short way round, as the plane's prop is ([`crate::biplane::between`]); which parts are
    /// up is `now`'s.
    pub fn between(was: &Rotor, now: &Rotor, t: f32) -> Rotor {
        let turn = std::f32::consts::TAU;
        let d = (now.angle - was.angle).rem_euclid(turn);
        let d = if d > turn / 2.0 { d - turn } else { d };
        Rotor { angle: now.angle - d * (1.0 - t.clamp(0.0, 1.0)), ..*now }
    }
}

/// A loaded helicopter: which model it is, and where its rotor nodes are among the model's.
#[derive(Clone, Debug)]
pub struct Kiowa {
    /// The name the model is loaded under in [`Assets`].
    pub model: String,
    /// `RotorSpindle`'s index among the model's nodes.
    pub spindle: Option<usize>,
    /// The four blade nodes' indices, where the model has them.
    pub blades: [Option<usize>; 4],
    /// `TailRotor_Node`'s index.
    pub tail: Option<usize>,
}

impl Kiowa {
    /// Load skin `skin` of the helicopter and its four blade parts into `assets`, from the first of
    /// `archives` to hold each file. `None` if the model is not there.
    pub fn load(assets: &mut Assets, archives: &[Res], skin: usize) -> Option<Kiowa> {
        let model = MODELS.get(skin).copied().unwrap_or(MODELS[0]);
        if !assets.load(archives, model) {
            return None;
        }
        for part in [BLADE_BLUR, BLADE_BENT, TAIL_BLUR, TAIL_BENT] {
            assets.load(archives, part);
        }
        // The blurs are see-through discs whose materials write depth, as the plane's prop cone is
        // (V107): left so, the disc hides what is drawn behind it after it.
        for blur in [BLADE_BLUR, TAIL_BLUR] {
            if let Some(loaded) = assets.models.get_mut(blur) {
                for s in loaded.surfaces.iter_mut().filter(|s| s.blend) {
                    s.depth_write = false;
                }
            }
        }
        Kiowa::found(assets, skin)
    }

    /// Skin `skin` of the helicopter as [`Kiowa::load`] left it in `assets`, without reading
    /// anything. `None` if it was not loaded.
    pub fn found(assets: &Assets, skin: usize) -> Option<Kiowa> {
        let model = MODELS.get(skin).copied().unwrap_or(MODELS[0]);
        let loaded = &assets.models.get(model)?.model;
        let find = |name: &str| loaded.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name));
        Some(Kiowa { model: model.to_string(), spindle: find(SPINDLE), blades: BLADES.map(find), tail: find(TAIL) })
    }

    /// The model's nodes posed by `rotor` (in the model's own space and node order): what the GPU
    /// skins the helicopter with and the software path skins a copy into. Empty if the model is not
    /// in `assets`.
    pub fn pose(&self, assets: &Assets, rotor: &Rotor) -> Vec<M4> {
        assets.models.get(&self.model).map_or(Vec::new(), |l| pose(&l.model, rotor))
    }

    /// What to draw for a helicopter placed at `at` posed by `pose` ([`Kiowa::pose`]): the helicopter,
    /// then the rotor parts `rotor` has up, each at its node's posed matrix.
    pub fn parts(&self, at: M4, pose: &[M4], rotor: &Rotor) -> Vec<(&'static str, M4)> {
        let node = |i: Option<usize>| i.and_then(|i| pose.get(i)).map(|m| m.mul(at));
        let mut out = Vec::new();
        if rotor.blades {
            out.extend(self.blades.iter().filter_map(|&b| node(b)).map(|m| (BLADE_BLUR, m)));
        }
        if let Some(m) = node(self.tail) {
            out.push((if rotor.wrecked { TAIL_BENT } else { TAIL_BLUR }, m));
        }
        out
    }
}

/// The helicopter's model posed by the rotor: `RotorSpindle`'s rotation replaced by a turn of the
/// rotor's angle about y and `TailRotor_Node`'s by one about x, their positions kept (the original's
/// node rotation write, on the nodes' own matrices), everything composed down the tree. With the main
/// blades thrown the original stops writing them, and the nodes keep the last turn they were given:
/// the angle the rotor stopped at, which `game_sim::kiowa::spin_blades` no longer runs on.
pub fn pose(model: &Model, rotor: &Rotor) -> Vec<M4> {
    let find = |name: &str| model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name));
    let mut local: Vec<M4> = model.nodes.iter().map(|n| n.local).collect();
    for (name, axis) in [(SPINDLE, v3(0.0, 1.0, 0.0)), (TAIL, v3(1.0, 0.0, 0.0))] {
        if let Some(i) = find(name) {
            let r = about(axis, rotor.angle);
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

/// Where a node of the model sits in the model's own space, by name: the seat's anchors, the
/// exhausts the rotor wash and smoke would ride.
pub fn node_at(model: &Model, name: &str) -> Option<V3> {
    let i = model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name))?;
    Some(model.world(i).point(v3(0.0, 0.0, 0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::psg::Node;

    fn node(name: &str, parent: Option<usize>, at: V3) -> Node {
        let mut local = M4::IDENTITY;
        local.0[3] = [at.x, at.y, at.z, 1.0];
        Node { name: name.into(), local, parent, center: at, half_extents: v3(1.0, 1.0, 1.0), radius: 1.0 }
    }

    /// A Kiowa's tree in miniature: the root, the spindle up the mast, a blade out along +x under it,
    /// and the tail rotor back along -z.
    fn heli() -> Model {
        Model {
            nodes: vec![
                node("Kiowa", None, v3(0.0, 0.0, 0.0)),
                node(SPINDLE, Some(0), v3(0.0, 10.0, 0.0)),
                node(BLADES[0], Some(1), v3(5.0, 0.0, 0.0)),
                node(TAIL, Some(0), v3(1.0, 6.0, -30.0)),
            ],
            materials: Vec::new(),
            meshes: Vec::new(),
            groups: 1,
            group_distance: vec![f32::MAX],
        }
    }

    fn close(a: V3, b: V3) -> bool {
        (a.x - b.x).abs() < 1e-4 && (a.y - b.y).abs() < 1e-4 && (a.z - b.z).abs() < 1e-4
    }

    #[test]
    fn the_spindle_turns_its_blades_about_the_mast_and_the_tail_about_x() {
        let m = heli();
        let quarter = std::f32::consts::FRAC_PI_2;
        let p = pose(&m, &Rotor { angle: quarter, ..Rotor::WHOLE });
        let blade = p[2].point(v3(0.0, 0.0, 0.0));
        // Out along x at rest, a quarter turn about y puts the blade's root on the z axis, at the
        // spindle's height, the same distance out.
        assert!(close(blade, v3(0.0, 10.0, -5.0)) || close(blade, v3(0.0, 10.0, 5.0)), "{blade:?}");
        // The tail stays where it is and turns about x: its own y goes to ±z.
        assert!(close(p[3].point(v3(0.0, 0.0, 0.0)), v3(1.0, 6.0, -30.0)));
        let up = p[3].point(v3(0.0, 1.0, 0.0)).sub(v3(1.0, 6.0, -30.0));
        assert!(up.y.abs() < 1e-4 && (up.z.abs() - 1.0).abs() < 1e-4, "{up:?}");
        // At rest it is the model's own.
        let rest = pose(&m, &Rotor::WHOLE);
        assert!(close(rest[2].point(v3(0.0, 0.0, 0.0)), v3(5.0, 10.0, 0.0)));
    }

    #[test]
    fn the_parts_follow_the_wreck() {
        let k = Kiowa { model: "kiowa".into(), spindle: Some(1), blades: [Some(2), None, None, None], tail: Some(3) };
        let p = pose(&heli(), &Rotor::WHOLE);
        let names = |r: Rotor| k.parts(M4::IDENTITY, &p, &r).into_iter().map(|(n, _)| n).collect::<Vec<_>>();
        assert_eq!(names(Rotor::WHOLE), [BLADE_BLUR, TAIL_BLUR]);
        assert_eq!(names(Rotor { wrecked: true, ..Rotor::WHOLE }), [BLADE_BLUR, TAIL_BENT]);
        assert_eq!(names(Rotor { blades: false, wrecked: true, ..Rotor::WHOLE }), [TAIL_BENT]);
    }

    #[test]
    fn a_frame_between_ticks_turns_the_short_way() {
        let tau = std::f32::consts::TAU;
        let was = Rotor { angle: tau - 0.2, ..Rotor::WHOLE };
        let now = Rotor { angle: 0.7, ..Rotor::WHOLE };
        let half = Rotor::between(&was, &now, 0.5);
        assert!((half.angle - 0.25).abs() < 1e-5, "{}", half.angle);
        assert_eq!(Rotor::between(&was, &now, 1.0).angle, 0.7);
    }
}
