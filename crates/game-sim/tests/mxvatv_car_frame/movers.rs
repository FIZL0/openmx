//! MD131: the tire step's contact velocity on what a tire stands on. The contact sort's struck objects
//! and kept movers stay in its hits, and each is laid out in free RAM as SLUS-21104 reads it: an object
//! on a rigid body or on a chassis with its transform, a mover with its transform and its two virtuals
//! (hooked, answered from the script). The port's [`Port`] answers [`Call::StandsOn`] from the same
//! script; the object's centre of mass the port writes back ([`Call::ObjectCofg`]) equals what the
//! original wrote there, and the movers' virtuals were asked as often.

use super::*;
use game_sim::atv_car::{Holder, Under};
use game_sim::contact::Node;

/// Free RAM past the sort's ([`sort::END`]): the movers' vtable and its two virtuals' hooks, and
/// each hit's body, chassis and transforms.
const VTABLE: u32 = sort::SCRATCH + 0x5800;
pub(crate) const MOVER_VELOCITY: u32 = sort::SCRATCH + 0x5900;
pub(crate) const MOVER_SPIN: u32 = sort::SCRATCH + 0x5910;
fn other_body(i: usize) -> u32 {
    sort::SCRATCH + 0x6000 + 0x300 * i as u32
}
fn chassis(i: usize) -> u32 {
    sort::SCRATCH + 0x7000 + 0x300 * i as u32
}
fn object_node(i: usize) -> u32 {
    sort::SCRATCH + 0x8000 + 0x200 * i as u32
}
fn mover_node(i: usize) -> u32 {
    sort::SCRATCH + 0x9000 + 0x200 * i as u32
}

/// A vector within `size` a lane, its `w` stirred.
fn stirred(r: &mut Rng, size: f32) -> Vf {
    let w = r.range(-1.0, 1.0);
    r.vec(size, w)
}

/// A transform: rows a turn (each `w` stirred), anywhere.
fn node(r: &mut Rng) -> Node {
    let q = r.quat();
    let row = |v: [f32; 3], r: &mut Rng| {
        let v = rotate(q, v);
        Vf::new(v[0], v[1], v[2], r.range(-1.0, 1.0))
    };
    Node { rows: [row([1.0, 0.0, 0.0], r), row([0.0, 1.0, 0.0], r), row([0.0, 0.0, 1.0], r)], origin: stirred(r, 50.0) }
}

/// What each hit's object and mover are: an object on a rigid body or on a chassis, half each; a
/// mover; all moving and turning, their `w`s stirred.
pub(crate) fn generate_unders(r: &mut Rng, s: &mut Script) {
    for i in 0..SORT_HITS {
        let cofg = stirred(r, 2.0);
        let on = if r.chance(0.5) {
            Holder::Body { position: r.vec(50.0, 1.0), velocity: stirred(r, 30.0), quat: r.quat(), rates: stirred(r, 5.0) }
        } else {
            Holder::Chassis { cofg: stirred(r, 1.0), velocity: stirred(r, 30.0), rates: stirred(r, 3.0), node: node(r) }
        };
        s.objects[i] = Under::Object { cofg, on };
        s.movers[i] = Under::Mover { node: node(r), origin: stirred(r, 5.0), spin: stirred(r, 3.0), velocity: stirred(r, 30.0) };
    }
}

/// The objects' centres of mass turned into the world before the frame: a word nobody has written.
pub(crate) const STALE_COFG: Vf = Vf([7.0, -7.0, 7.0, 7.0]);

/// Lay `s`'s objects and movers out where the sort's hits point.
pub(crate) fn put_unders(ee: &mut Ee, s: &Script) {
    let put_node = |ee: &mut Ee, at: u32, n: &Node| {
        for (k, row) in n.rows.iter().enumerate() {
            put_v(ee, at + 0x70 + 0x10 * k as u32, *row);
        }
        put_v(ee, at + 0xa0, n.origin);
        ee.set_u32(at + 0xf0, 0); // no parent
        ee.set_u32(at + 0x100, 1); // up to date
    };
    for i in 0..SORT_HITS {
        let (o, m) = (sort::object(i), sort::mover(i));
        let Under::Object { cofg, on } = s.objects[i] else { unreachable!() };
        put_v(ee, o + 0x80, cofg);
        put_v(ee, o + 0x90, STALE_COFG);
        // A chassis and transform there too, which a body hides.
        ee.set_u32(o + 0xe0, chassis(i));
        ee.set_u32(o + 0xe8, object_node(i));
        match on {
            Holder::Body { position, velocity, quat, rates } => {
                ee.set_u32(o + 0xe4, other_body(i));
                let b = other_body(i);
                put_v(ee, b + 0x70, position);
                put_v(ee, b + 0x80, velocity);
                put_v(ee, b + 0x190, quat);
                put_v(ee, b + 0x1d0, rates);
            }
            Holder::Chassis { cofg, velocity, rates, node } => {
                ee.set_u32(o + 0xe4, 0);
                put_v(ee, chassis(i) + 0x180, cofg);
                put_v(ee, chassis(i) + 0x250, velocity);
                put_v(ee, chassis(i) + 0x280, rates);
                put_node(ee, object_node(i), &node);
            }
        }
        let Under::Mover { node, origin, .. } = s.movers[i] else { unreachable!() };
        ee.set_u32(m + 4, VTABLE);
        ee.set_u32(m + 0x244, mover_node(i));
        put_v(ee, m + 0x180, origin);
        put_node(ee, mover_node(i), &node);
    }
    ee.set_u32(VTABLE + 0xc0, MOVER_VELOCITY);
    ee.set_u32(VTABLE + 0xc4, MOVER_SPIN);
}

/// The movers' two virtuals, answered from the script by the mover asked; each call kept in `asked`
/// (`true` the spin) to compare with the port's.
pub(crate) fn hook_movers(ee: &mut Ee, script: Shared<Script>, asked: Shared<Vec<bool>>) {
    for (at, spin) in [(MOVER_VELOCITY, false), (MOVER_SPIN, true)] {
        let (script, asked) = (script.clone(), asked.clone());
        ee.hooks.insert(
            at,
            Box::new(move |ee| {
                let m = ee.r(5) as u32;
                let i = (0..SORT_HITS).position(|i| sort::mover(i) == m).expect("a mover of ours");
                let Under::Mover { spin: s, velocity: v, .. } = script.borrow().movers[i] else { unreachable!() };
                put_v(ee, ee.r(4) as u32, if spin { s } else { v });
                asked.borrow_mut().push(spin);
            }),
        );
    }
}

/// What [`Port`] answers [`Call::StandsOn`] with: the first word's object, else the second's mover.
pub(crate) fn under(s: &Script, on: [u32; 2]) -> Under {
    let find = |w: u32, at: fn(usize) -> u32| (0..SORT_HITS).position(|i| at(i) == w).expect("a word of ours");
    if on[0] != 0 {
        s.objects[find(on[0], sort::object)]
    } else {
        s.movers[find(on[1], sort::mover)]
    }
}

/// **The tire step on movers is MX Unleashed's**: the sort, the probes and the tire step unhooked, the
/// sort's hits keeping their struck objects and kept movers, over 6,000 frames: every word equal, as
/// in [`pieces`]; each object's world centre of mass as the original wrote it; the movers' virtuals
/// asked the same number of times, spin first. Each kind of ground is seen, standing on it and handed
/// it.
#[test]
fn tire_step_on_movers_is_mx_unleasheds() {
    let unhooked = [SORT_CONTACTS, PROBE, TIRE_STEP];
    let Some((_, seen)) = run_with(0x0131_0001, 6000, &unhooked, true) else { return };
    for what in ["under: a body", "under: a chassis", "under: a mover", "under: an object over a mover", "tire: handed a mover's"] {
        let k = seen.get(what).copied().unwrap_or(0);
        assert!(k >= 20, "{what} seen only {k} times: {seen:?}");
    }
}
