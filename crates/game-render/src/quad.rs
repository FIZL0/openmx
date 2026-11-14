//! **The quad's chassis: the nodes MX vs. ATV Unleashed's ATV models carry where a bike has six.**
//!
//! A bike's chassis is one fork and one swingarm ([`crate::vehicle::Nodes`]). The three ATV chassis
//! models on SLUS_211.04 (`ATVS_R_Chasis`, `atv_s_chasis`, `250R_atv_chasis`, each named by its own
//! motion set's `SLTFile`, MD37c) share one skeleton of 43 nodes, rooted at a node spelt
//! `250r_atv_chasis` in all three:
//!
//! ```text
//!   root ─┬─ AarmUpperLeft ─┬─ AarmUpperLeftNode
//!         │                 └─ LeftHub ── WheelLeftFront, LeftHubNode, LeftUpperHubNode,
//!         │                               TieRodLeftNode, FrontLeftParticle01
//!         ├─ AarmUpperRight ── (the same, Right)
//!         ├─ AarmLowerLeft/Right ── AarmLower…Node, FrontLower…Strut
//!         ├─ SwingArm ─┬─ RearAxle ── WheelLeftRear, WheelRightRear
//!         │            └─ RearLowerStrut, RearLeft/RightParticle01
//!         ├─ HandleBars ── BrakeHandle, ClutchHandle, Dummy_Hand_L/R
//!         └─ TieRodLeft/Right, FrontUpper…Strut, RearUpperStrut, Dummy_Foot_L/R, ShadowBody
//! ```
//!
//! (the tree as openmx-mods' `import-atv check` prints it, the same in all three; which of these the
//! original drives from its step, and how, is MD37b's). [`Nodes::of`] finds them by name, as the
//! game does, case-insensitively.
//!
//! **MD138:** [`pose`] poses them from the step — the port's own rig, matched by eye, as the cars'
//! ([`crate::car::pose`]): the original's ATV measure step is not ported. [`seat`] puts the rider on
//! the pack's ATV motion set ([`RIDING`], [`STOPPED`], [`AIR`]).

use crate::anim::{Bound, Set};
use crate::car::{Tire, aim, compose, find, origin, spin_and_stretch, turn};
use crate::mat::{M4, v3};
use crate::psg::Model;
use crate::rider::{Anchors, Body};
use crate::skin::{Clip, Skeleton};

pub const BARS: &str = "HandleBars";
pub const SWINGARM: &str = "SwingArm";
pub const REAR_AXLE: &str = "RearAxle";
/// The four wheels: front left, front right, rear left, rear right.
pub const WHEELS: [&str; 4] = ["WheelLeftFront", "WheelRightFront", "WheelLeftRear", "WheelRightRear"];
/// The front hubs, left and right: each carries its wheel and turns with the tie rod.
pub const HUBS: [&str; 2] = ["LeftHub", "RightHub"];
/// The A-arms: upper left, upper right, lower left, lower right.
pub const A_ARMS: [&str; 4] = ["AarmUpperLeft", "AarmUpperRight", "AarmLowerLeft", "AarmLowerRight"];
pub const TIE_RODS: [&str; 2] = ["TieRodLeft", "TieRodRight"];

/// Where each of the quad's moving parts is in a chassis skeleton; `None` for a name it lacks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Nodes {
    pub bars: Option<usize>,
    pub swingarm: Option<usize>,
    pub rear_axle: Option<usize>,
    pub wheels: [Option<usize>; 4],
    pub hubs: [Option<usize>; 2],
    pub a_arms: [Option<usize>; 4],
    pub tie_rods: [Option<usize>; 2],
}

impl Nodes {
    pub fn of(skeleton: &Skeleton) -> Nodes {
        Nodes {
            bars: skeleton.find(BARS),
            swingarm: skeleton.find(SWINGARM),
            rear_axle: skeleton.find(REAR_AXLE),
            wheels: WHEELS.map(|n| skeleton.find(n)),
            hubs: HUBS.map(|n| skeleton.find(n)),
            a_arms: A_ARMS.map(|n| skeleton.find(n)),
            tie_rods: TIE_RODS.map(|n| skeleton.find(n)),
        }
    }

    /// Every name [`Nodes::of`] looked for and did not find.
    pub fn missing(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        let mut one = |at: Option<usize>, name: &'static str| {
            if at.is_none() {
                out.push(name);
            }
        };
        one(self.bars, BARS);
        one(self.swingarm, SWINGARM);
        one(self.rear_axle, REAR_AXLE);
        for (at, name) in self.wheels.iter().zip(WHEELS) {
            one(*at, name);
        }
        for (at, name) in self.hubs.iter().zip(HUBS) {
            one(*at, name);
        }
        for (at, name) in self.a_arms.iter().zip(A_ARMS) {
            one(*at, name);
        }
        for (at, name) in self.tie_rods.iter().zip(TIE_RODS) {
            one(*at, name);
        }
        out
    }
}

/// The front struts, left and right: the upper on the chassis, the lower on the lower A-arm.
pub const FRONT_STRUTS: [[&str; 2]; 2] = [["FrontUpperLeftStrut", "FrontLowerLeftStrut"], ["FrontUpperRightStrut", "FrontLowerRightStrut"]];
/// The rear strut: the upper on the chassis, the lower on the swingarm.
pub const REAR_STRUT: [&str; 2] = ["RearUpperStrut", "RearLowerStrut"];
/// Where each lower A-arm meets its hub: under the hub, so the lower arm follows it up.
pub const LOWER_JOINTS: [&str; 2] = ["LeftHubNode", "RightHubNode"];
/// Where each tie rod meets its hub.
pub const TIE_ROD_ENDS: [&str; 2] = ["TieRodLeftNode", "TieRodRightNode"];

/// The chassis posed by its four wheels (in [`WHEELS`]' order: spin, steer, compression; the scale
/// is not read — a quad's tires do not stretch) and the bars turned by `bars` radians, in the model's
/// own space and node order. At rest, the model as it is.
///
/// The port's own rig, as the baja's ([`crate::car::pose`]): each upper A-arm swung about its own z
/// so its hub rises by the wheel's compression, the hub turned back level and steered about y, the
/// lower arm aimed at the hub's lower joint, the tie rod at its end on the hub, the swingarm about
/// its x so the rear axle rises by the more compressed rear wheel, the wheels spun, the handlebars
/// turned, then each strut pair aimed at each other. A node the model lacks is left as it is.
pub fn pose(model: &Model, wheels: &[Tire; 4], bars: f32) -> Vec<M4> {
    let mut local: Vec<M4> = model.nodes.iter().map(|n| n.local).collect();
    let bind = compose(model, &local);
    let at = |name: &str| find(model, name);
    let (x, y, z) = (v3(1.0, 0.0, 0.0), v3(0.0, 1.0, 0.0), v3(0.0, 0.0, 1.0));
    for side in 0..2 {
        let wheel = &wheels[side];
        let (Some(upper), Some(hub)) = (at(A_ARMS[side]), at(HUBS[side])) else { continue };
        let reach = crate::car::into(bind[upper], origin(bind[hub]));
        // Up on each side: the hub lies out along the arm's x one way or the other.
        let length = reach.len();
        let swing = if length > 1e-6 { (wheel.compression / length).clamp(-1.0, 1.0).asin() } else { 0.0 } * reach.x.signum();
        turn(&mut local[upper], z, swing);
        turn(&mut local[hub], z, -swing);
        turn(&mut local[hub], y, wheel.steer);
        if let Some(w) = at(WHEELS[side]) {
            spin_and_stretch(&mut local[w], wheel, false);
        }
    }
    if let (Some(arm), Some(axle)) = (at(SWINGARM), at(REAR_AXLE)) {
        let up = wheels[2].compression.max(wheels[3].compression);
        // Risen by `up` on the circle the axle swings on about the swingarm's pivot.
        let (pivot, rest) = (origin(bind[arm]), origin(bind[axle]));
        let (r, dy) = ((rest.y - pivot.y).hypot(rest.z - pivot.z), rest.y + up - pivot.y);
        let dz = (r * r - dy * dy).max(0.0).sqrt() * (rest.z - pivot.z).signum();
        let now = compose(model, &local);
        aim(&mut local, arm, &now, &bind, v3(rest.x, pivot.y + dy, pivot.z + dz), rest, Some(x));
    }
    for k in 2..4 {
        if let Some(w) = at(WHEELS[k]) {
            spin_and_stretch(&mut local[w], &wheels[k], false);
        }
    }
    if let Some(b) = at(BARS) {
        turn(&mut local[b], y, bars);
    }
    let now = compose(model, &local);
    for side in 0..2 {
        if let (Some(lower), Some(joint)) = (at(A_ARMS[2 + side]), at(LOWER_JOINTS[side])) {
            aim(&mut local, lower, &now, &bind, origin(now[joint]), origin(bind[joint]), Some(z));
        }
        if let (Some(rod), Some(end)) = (at(TIE_RODS[side]), at(TIE_ROD_ENDS[side])) {
            aim(&mut local, rod, &now, &bind, origin(now[end]), origin(bind[end]), None);
        }
    }
    let now = compose(model, &local);
    for [u, l] in [FRONT_STRUTS[0], FRONT_STRUTS[1], REAR_STRUT] {
        let (Some(u), Some(l)) = (at(u), at(l)) else { continue };
        aim(&mut local, u, &now, &bind, origin(now[l]), origin(bind[l]), None);
        aim(&mut local, l, &now, &bind, origin(now[u]), origin(bind[u]), None);
    }
    compose(model, &local)
}

/// The ATV motion set's riding row, forward, middle and back: each clip a sweep of the turn's lean
/// over its length.
pub const RIDING: [&str; 3] = ["ATV_turn_riding_Fwd", "ATV_turn_riding_Mid", "ATV_turn_riding_Bck"];
/// Its row for a quad standing still.
pub const STOPPED: [&str; 3] = ["ATV_turn_stop_Fwd", "ATV_turn_stop_Mid", "ATV_turn_stop_Bck"];
/// Its row in the air.
pub const AIR: [&str; 3] = ["ATV_air_Fwd", "ATV_air_Mid", "ATV_air_Bck"];

/// Bind `clip` into the rider's `set` under `name`, unless a clip of that name is bound already. The
/// ATV motion set names the MX rider's skeleton (`mrider.Slt`), so its clips drive the bike's rider
/// as they are.
pub fn bind(set: &mut Set, name: &str, clip: &Clip) {
    if set.id(name).is_none() {
        let joints = clip.joints(&set.skeleton);
        set.clips.push(Bound { name: name.to_owned(), member: name.to_owned(), clip: clip.clone(), joints });
    }
}

/// Pose the rider's `set` on one `row` ([`RIDING`], [`STOPPED`], [`AIR`]): at `phase` (0 to 1) of
/// the clips' length for the turn's lean, mixed from the middle clip towards the forward one as
/// `fore_aft` goes to -1 and the back one as it goes to 1. False if the row is not bound ([`bind`]).
pub fn seat(set: &mut Set, row: [&str; 3], phase: f32, fore_aft: f32) -> bool {
    let (Some(fwd), Some(mid), Some(bck)) = (set.id(row[0]), set.id(row[1]), set.id(row[2])) else {
        return false;
    };
    let lean = fore_aft.clamp(-1.0, 1.0);
    let toward = if lean < 0.0 { fwd } else { bck };
    let (a, b) = (&set.clips[mid.0 as usize], &set.clips[toward.0 as usize]);
    let time = phase.clamp(0.0, 1.0) * a.clip.duration();
    a.clip.pose_mixed(&b.clip, time, 1.0 - lean.abs(), &a.joints, &mut set.pose);
    true
}

/// The rider's node matrices on the quad: [`seat`]ed on `row`, held to the chassis' `anchors` (its
/// own pegs and grips, in the chassis' space, which is the body's) at full strength with both feet
/// on, and the body's own pose, hold and feet put back afterwards, as the plane's pilot
/// ([`crate::biplane::pilot_palette`]). `None` if the row is not bound.
pub fn rider_palette(body: &mut Body, anchors: &Anchors, row: [&str; 3], phase: f32, fore_aft: f32) -> Option<Vec<M4>> {
    let (pose, hold, feet) = (body.rig.set.pose.clone(), body.hold, body.feet);
    body.hold = 1.0;
    body.feet = Default::default();
    let palette = seat(&mut body.rig.set, row, phase, fore_aft).then(|| body.palette(Some(anchors)).0);
    body.rig.set.pose = pose;
    body.hold = hold;
    body.feet = feet;
    palette
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mat::V3;
    use crate::psg::Node;

    fn node(name: &str, parent: Option<usize>, at: V3) -> Node {
        let mut local = M4::IDENTITY;
        local.0[3] = [at.x, at.y, at.z, 1.0];
        Node { name: name.into(), local, parent, center: at, half_extents: v3(1.0, 1.0, 1.0), radius: 1.0 }
    }

    /// The chassis in miniature: one front corner, the swingarm and rear axle, the bars, a strut pair.
    fn chassis() -> Model {
        let nodes = vec![
            node("250r_atv_chasis", None, v3(0.0, 0.0, 0.0)),
            node(A_ARMS[1], Some(0), v3(0.3, 1.0, 1.8)),
            node(HUBS[1], Some(1), v3(1.05, 0.0, 0.0)),
            node(WHEELS[1], Some(2), v3(0.3, -0.2, 0.0)),
            node(LOWER_JOINTS[1], Some(2), v3(0.0, -0.4, 0.0)),
            node(A_ARMS[3], Some(0), v3(0.2, 0.6, 1.8)),
            node(TIE_RODS[1], Some(0), v3(0.2, 0.9, 1.6)),
            node(TIE_ROD_ENDS[1], Some(2), v3(0.0, 0.0, -0.2)),
            node(SWINGARM, Some(0), v3(0.0, 1.21, -0.66)),
            node(REAR_AXLE, Some(8), v3(0.0, -0.63, -1.33)),
            node(WHEELS[3], Some(9), v3(0.9, 0.0, 0.0)),
            node(BARS, Some(0), v3(0.0, 2.32, 1.36)),
            node(FRONT_STRUTS[1][0], Some(0), v3(0.5, 1.6, 1.8)),
            node(FRONT_STRUTS[1][1], Some(5), v3(0.6, 0.0, 0.0)),
        ];
        Model { nodes, materials: Vec::new(), meshes: Vec::new(), groups: 1, group_distance: vec![f32::MAX] }
    }

    fn at(p: &[M4], i: usize) -> V3 {
        p[i].point(v3(0.0, 0.0, 0.0))
    }

    #[test]
    fn at_rest_the_quad_is_its_model() {
        let m = chassis();
        let p = pose(&m, &[Tire::REST; 4], 0.0);
        for (i, posed) in p.iter().enumerate() {
            let w = m.world(i);
            for r in 0..4 {
                for c in 0..4 {
                    assert!((posed.0[r][c] - w.0[r][c]).abs() < 1e-5, "node {i}: {posed:?} against {w:?}");
                }
            }
        }
    }

    #[test]
    fn compression_lifts_the_wheels_and_the_lower_arm_follows() {
        let m = chassis();
        let mut wheels = [Tire::REST; 4];
        wheels[1].compression = 0.3;
        wheels[3].compression = 0.25;
        let (rest, p) = (pose(&m, &[Tire::REST; 4], 0.0), pose(&m, &wheels, 0.0));
        let hub = at(&p, 2).sub(at(&rest, 2));
        assert!((hub.y - 0.3).abs() < 1e-3, "the hub rose {hub:?}");
        let axle = at(&p, 9).sub(at(&rest, 9));
        assert!((axle.y - 0.25).abs() < 1e-3, "the rear axle rose {axle:?}");
        // The lower arm points at the hub's lower joint as it did at rest.
        let along = p[5].point(crate::car::into(rest[5], at(&rest, 4))).sub(at(&p, 5)).norm();
        let is = at(&p, 4).sub(at(&p, 5)).norm();
        assert!(along.dot(is) > 0.9999, "the lower arm points {along:?}, the joint is {is:?}");
        assert!(p[2].0[0][1].abs() < 1e-4, "the hub tipped: {:?}", p[2].0[0]);
    }
}
