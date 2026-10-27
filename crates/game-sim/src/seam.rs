//! **Over a seam, everything that moves** (MD44, the port's own): what [`crate::rider::Rider::shift`]
//! does to the chassis, done to the other bodies a world that wraps carries across a seam — the
//! plane's ([`crate::flight::Flight::wrap_round`]), the riding ragdoll's bones, and a wreck's bike
//! and ragdoll.
//!
//! Each one moves its **places** along `x` and `z` by `by`, exactly the sum `v + by` on each: the
//! centres of mass and where they were at the last substep, nodes' origins, contact points, hold
//! targets and search centres. Velocities, forces, rates, orientations and everything kept in a
//! body's own frame are not places, and are left alone. Every function here is a no-op on a world
//! that does not wrap, because nothing calls it there.

use crate::biplane::Body;
use crate::contact_search::Record;
use crate::math::Mat4;
use crate::ragdoll::Ragdoll;
use crate::ragdoll_body::Actor;
use crate::ragdoll_ride::Down;
use crate::ragdoll_riding::Riding;
use crate::ride::Crash;
use ps2_float::{F, Vf};

/// `v` moved along `x` and `z` by `by`; `y` and `w` as they were.
pub fn moved(v: Vf, by: [F; 2]) -> Vf {
    Vf::new((F(v.x()) + by[0]).0, v.y(), (F(v.z()) + by[1]).0, v.w())
}

/// A matrix's origin row moved; its axes are directions.
pub fn matrix(m: &mut Mat4, by: [F; 2]) {
    m.0[3] = moved(m.0[3], by);
}

/// A dynamics body: its centre of mass, now and at the start of the last substep, so the next
/// substep's sweep back measures the same move.
pub fn body(b: &mut Body, by: [F; 2]) {
    b.position = moved(b.position, by);
    b.last_position = moved(b.last_position, by);
}

/// A collision object's record: its node's origin, the last sweep's two ends, and every contact's
/// world point (its own two points are in the anchoring body's frame).
pub fn record(r: &mut Record, by: [F; 2]) {
    r.node.origin = moved(r.node.origin, by);
    r.swept_from = moved(r.swept_from, by);
    r.swept_to = moved(r.swept_to, by);
    for c in &mut r.contacts {
        c.point = moved(c.point, by);
    }
}

/// A ragdoll: every bone's body, and where each hold that follows nothing is held. A joint's pin
/// holds two bones together in their own frames, and moves with them.
pub fn ragdoll(r: &mut Ragdoll, by: [F; 2]) {
    for b in &mut r.bones {
        body(&mut b.body, by);
    }
    for p in r.pins.iter_mut().filter(|p| p.second.is_none()) {
        p.target = moved(p.target, by);
    }
}

/// A ragdoll's bone contacts: the point each was last pushed away from.
fn bone_contacts(cs: &mut [crate::ragdoll_contact::Contact], by: [F; 2]) {
    for c in cs {
        c.point = moved(c.point, by);
    }
}

/// The body actor the riding ragdoll is: its ragdoll, its contacts, where its four holds hold and
/// the bike's frame it is boxed in.
pub fn actor(a: &mut Actor, by: [F; 2]) {
    ragdoll(&mut a.ragdoll, by);
    bone_contacts(&mut a.contacts, by);
    for t in &mut a.hold_targets {
        *t = moved(*t, by);
    }
    matrix(&mut a.bike_frame, by);
}

/// **The riding ragdoll** (OPENMX_RIDING_RAGDOLL=1): its actor, and the skeleton's drawn matrices
/// from its last pose and the one before, which the next pose and a wreck's entry measure the
/// bones' move from.
pub fn riding(r: &mut Riding, by: [F; 2]) {
    actor(&mut r.actor, by);
    for m in r.drawn.iter_mut().chain(r.was.iter_mut()).flatten() {
        matrix(m, by);
    }
}

/// **A wreck's ragdoll**: its bones, their contacts, and the bones' search of the wreck bike —
/// where its sphere last stood and the centre the list was gathered round, so the list the bike
/// moved with is still the list. Where the holds held is in the wreck bike's frame.
pub fn down(d: &mut Down, by: [F; 2]) {
    ragdoll(&mut d.ragdoll, by);
    bone_contacts(&mut d.contacts, by);
    d.search.stood = d.search.stood.map(|s| moved(s, by));
    d.search.centre = moved(d.search.centre, by);
}

/// **A wreck**: the wreck bike's body, its collision record and its collider's centre of gravity,
/// the transform's origin, and the rider's point.
pub fn crash(c: &mut Crash, by: [F; 2]) {
    body(&mut c.bike.body, by);
    record(&mut c.bike.record, by);
    c.bike.collider.cofg_world = moved(c.bike.collider.cofg_world, by);
    c.origin = moved(c.origin, by);
    c.rider = moved(c.rider, by);
}

/// Which way a point is past a world's seams on one axis: `1` at or past the far one, `-1` short of
/// the near one, `0` inside or on an axis that does not wrap (and for `NaN`).
pub fn lap(on: bool, v: f32, side: F) -> i8 {
    if !on {
        0
    } else if F(v) >= side {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

/// What a lap moves a point by: back a width past the far seam, on a width short of the near one.
pub fn by(lap: i8, side: F) -> F {
    match lap {
        1 => F::ZERO - side,
        -1 => side,
        _ => F::ZERO,
    }
}
