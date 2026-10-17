//! **Where the renderer actually put the bike, against where the solver put it.**
//!
//! The ground step answers three questions every frame and the drawing side is supposed to agree with
//! all three: the chassis is *here* ([`game_sim::ride::Ride::position`]), each wheel is touching the
//! ground *there* ([`game_sim::wheel::Wheel::contact_point`]), and the chassis' own grips and pegs are
//! where the rider's hands and feet belong ([`crate::rider::anchors_of`]). None of the three was
//! checked. V20 found two of them wrong at once, and both were invisible in the solver's numbers —
//! which is why this module exists: the comparisons are a function the tests call, not a thing to
//! squint at in a screenshot.
//!
//! ## What was wrong
//!
//! **The drawn chassis origin was not the ride's.** Every `bike_*.mot` on this disc animates exactly
//! six nodes — the root and the five the drives write — and the root channel is real: it is the bike
//! thrown out from under the rider during a crossup or a nacnac, and it is at the origin at both ends
//! of every clip. But a bike archive has no clip for any grounded state ([`crate::vehicle::Chassis`]'
//! own notes, and V9's journal), so the grounded rows of the table name nothing, the blend writes no
//! pose at all, and the *last* pose stays in the buffer. A ride that is airborne for a moment at the
//! spawn lands with the airborne row's root channel frozen into it — a foot and a half sideways and
//! most of a foot up — and from then on the whole bike is drawn off its own physics, which reads as a
//! bike buried in the dirt with neither wheel on the ground and a rider standing above it.
//! [`poses_the_chassis`] is the test the drives now make before they read the pose buffer.
//!
//! **The rider's hands were aimed at the bind pose.** The shell read the chassis' four anchors once,
//! when nothing animated the chassis; since V9 the steering drive turns `Handlebars`, and both hand
//! anchors hang off it. [`grips`] reads them off the live pose, which is the same thing the
//! original's rider does with the four nodes its vehicle cached.
//!
//! Nothing here is on a drawing path: it is measurement, and [`grips`] and [`poses_the_chassis`], both
//! of which are one read each.

use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::rider::{Anchors, Body, LIMBS, anchors_of, hold_on};
use crate::vehicle::Chassis;
use game_sim::motion::Player;
use game_sim::ride::Ride;
use game_sim::vehicle_anim::Blend;
use ps2_float::Vf;

/// Where the shell draws the bike: the chassis' transform, built from the ride's own placement.
///
/// The one definition of it, because a test that built its own would not be testing the shell. The
/// simulation keeps positions and axes as [`ps2_float::Vf`] and the renderer as [`V3`]; this is the
/// seam, and `forward` is the model's `+Z` and `up` its `+Y`, which is the convention every `.scn`
/// placement is in ([`M4::placement`]).
pub fn placed(ride: &Ride) -> M4 {
    let (forward, up) = ride.axes();
    M4::placement(at(ride.position()), at(forward), at(up))
}

/// A simulation vector as the renderer's own.
fn at(v: Vf) -> V3 {
    v3(v.x(), v.y(), v.z())
}

/// Does this frame's blend name a clip at all?
///
/// A bike's clip table has holes in it — the 36 grounded rows of the pose grid are the rider's clips
/// and no bike archive ships one — and a blend whose every handle is empty draws nothing. That is not
/// the same as a clip that has run out, which holds the pose it ended on and must keep it; it is a
/// state with no animation, and the bike's shape there is the one it was modelled in with the drives
/// written over it. So the drives read the pose buffer only when the frame put something in it, and
/// the rest shape otherwise ([`Chassis::drive`]).
///
/// Mode 0 is the one blend that does not carry its own handle: the clip it samples is whatever the
/// motion object has on, which is `player`'s.
pub fn poses_the_chassis(blend: Blend, player: &Player) -> bool {
    match blend {
        Blend::Free => player.clip.is_some(),
        Blend::One { clip, .. } => clip.is_some(),
        Blend::Two { a, b, .. } | Blend::Pair { a, b, .. } => a.is_some() || b.is_some(),
        Blend::Four { a, b, c, d, .. } => {
            a.is_some() || b.is_some() || c.is_some() || d.is_some()
        }
    }
}

/// Where the chassis wants the body's hands and feet **this frame**: the four dummy anchors at its
/// grips and its pegs, read off the live pose rather than off the bind one.
///
/// `Dummy_Hand_L` and `Dummy_Hand_R` are children of `Handlebars`, which the steering drive rotates,
/// so a rider aimed at the bind pose's grips lets go of the bars as soon as the bike is steered —
/// and a clip that poses the chassis moves all four. The original caches the four *nodes* when the
/// vehicle loads and reads their world transforms every frame, which is this.
pub fn grips(chassis: &Chassis) -> Anchors {
    anchors_of(&chassis.stance.set.skeleton, &chassis.world)
}

/// One drawn wheel against the contact the ground solver found under it.
#[derive(Clone, Copy, Debug)]
pub struct Tyre {
    /// The lowest point of everything drawn under the wheel's node, in world space — the tire's own
    /// vertices, not a radius out of the `.phy`.
    pub lowest: V3,
    /// Where the suspension's ray put the contact, in world space.
    pub contact: V3,
    /// How far the lowest drawn point is above the plane the contact is on, in feet. Negative is a
    /// wheel in the dirt; a wheel this solver says is loaded should be near zero.
    pub clearance: f32,
}

/// The three things the drawing side has to agree with the ground step about, measured on one frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Seat {
    /// How far the drawn chassis origin is from [`game_sim::ride::Ride::position`], in feet.
    pub origin: f32,
    /// The two wheels the solver says are in contact, front then rear. `None` for a wheel that is in
    /// the air — there is no contact to compare it with — or whose node the model has not got.
    pub tyres: [Option<Tyre>; 2],
    /// How far each of the body's four anchors ended from the chassis' own after the limbs were
    /// solved, in feet, in [`LIMBS`]' order: both feet, then both hands. `None` for a limb that was
    /// not solved, and for every one of them when there is no body.
    pub held: [Option<f32>; 4],
}

/// Measure one drawn frame against the ground step that produced it.
///
/// `model` is the chassis' own `.psg`, which is where the wheels' vertices are; `body` the rider as
/// the frame posed it, or `None` for a bike with nobody on it. Everything is in world space and in
/// the disc's unit, which is a foot.
pub fn measure(chassis: &Chassis, model: &Model, ride: &Ride, body: Option<&Body>) -> Seat {
    let at = placed(ride);
    let origin = match chassis.nodes.root.and_then(|j| chassis.world.get(j)) {
        Some(root) => at.point(root.point(v3(0.0, 0.0, 0.0))),
        // No root node is no pose at all: the model is drawn at the bike's transform as it is.
        None => at.point(v3(0.0, 0.0, 0.0)),
    };
    let place = placed(ride).point(v3(0.0, 0.0, 0.0));
    Seat {
        origin: origin.sub(place).len(),
        tyres: [
            tyre(chassis, model, &ride.front, chassis.nodes.front, at),
            tyre(chassis, model, &ride.rear, chassis.nodes.rear, at),
        ],
        held: match body {
            Some(body) => held(body, &grips(chassis)),
            None => [None; 4],
        },
    }
}

/// One wheel's comparison, for a wheel the solver has loaded.
fn tyre(
    chassis: &Chassis,
    model: &Model,
    wheel: &game_sim::ride::Wheel,
    node: Option<usize>,
    at: M4,
) -> Option<Tyre> {
    if !wheel.mv.contact {
        return None;
    }
    let normal = at_norm(wheel.w.contact_normal);
    let contact = v3(
        wheel.w.contact_point.x(),
        wheel.w.contact_point.y(),
        wheel.w.contact_point.z(),
    );
    let lowest = lowest_under(model, &chassis.world, at, node?, normal)?;
    Some(Tyre { lowest, contact, clearance: lowest.sub(contact).dot(normal) })
}

/// The contact normal, normalised and as the renderer's own vector.
fn at_norm(v: Vf) -> V3 {
    let n = v3(v.x(), v.y(), v.z());
    match n.len() > 0.0 {
        true => n.norm(),
        false => v3(0.0, 1.0, 0.0),
    }
}

/// The lowest drawn point of one node's subtree, measured along `up`.
///
/// A `.psg` vertex names the node that places it, so "the wheel" is every vertex of the finest level
/// of detail whose node is this one or hangs off it — the tire and its rim, and nothing of the fork
/// above them. `world` is the chassis' live joint array and `at` the bike's own transform.
pub fn lowest_under(model: &Model, world: &[M4], at: M4, node: usize, up: V3) -> Option<V3> {
    let mine = subtree(model, node);
    let mut best: Option<(f32, V3)> = None;
    for mesh in model.meshes.iter().filter(|m| m.group == 0) {
        for strip in &mesh.strips {
            for v in &strip.verts {
                if !mine.get(v.node).copied().unwrap_or(false) {
                    continue;
                }
                let p = match world.get(v.node) {
                    Some(m) => at.point(m.point(v.pos)),
                    None => at.point(v.pos),
                };
                let height = p.dot(up);
                if best.is_none_or(|(low, _)| height < low) {
                    best = Some((height, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Which nodes are this one or hang off it, as a flag per node.
fn subtree(model: &Model, node: usize) -> Vec<bool> {
    let mut mine = vec![false; model.nodes.len()];
    if node >= mine.len() {
        return mine;
    }
    mine[node] = true;
    // The tree is stored parent-before-child on every `.psg` on this disc, so one pass down the list
    // closes it; a child listed before its parent simply does not join, which costs a vertex or two
    // of the measurement and never a wrong answer.
    for (i, n) in model.nodes.iter().enumerate() {
        if let Some(p) = n.parent {
            if mine.get(p).copied().unwrap_or(false) {
                mine[i] = true;
            }
        }
    }
    mine
}

/// How far the body's four anchors end from the chassis' own, with the limbs solved onto them: what
/// the drawn rider's hands and feet actually do, rather than what the clip asked for.
///
/// The same call the drawing makes ([`Body::skin_into_at`]), without the skinning — the solve is on a
/// copy of the joint array either way, so this measures the frame that is drawn and does not change
/// it.
fn held(body: &Body, targets: &Anchors) -> [Option<f32>; 4] {
    let mut world = body.rig.world();
    hold_on(&body.rig.set.skeleton, &mut world, targets, 1.0)
}

/// The names of the four limbs, for a failure message that says which one let go.
pub fn limb_names() -> [&'static str; 4] {
    [LIMBS[0].anchor, LIMBS[1].anchor, LIMBS[2].anchor, LIMBS[3].anchor]
}
