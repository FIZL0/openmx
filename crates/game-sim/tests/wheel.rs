//! S16 — the wheel, proved against the original.
//!
//! The same shape as `tests/move.rs`, one level down: the harness lays a wheel out in the
//! interpreter's RAM, writes a generated state into it, runs the original, then writes the *same*
//! state again, applies the port over the top and compares **every word** of the wheel object. A field
//! the original writes and the port does not model is a difference, not an omission nobody notices.
//!
//! What a wheel reaches outside itself is read-only and so is not compared: its owner, the owner's
//! transform, and the world it asks for the ground. [`only_touched_the_regions`] is what says the
//! original never wrote to any of them.
//!
//! # The ground query
//!
//! The suspension ray's one call is the world's "where is the ground under this point" query, which
//! eighty-four places in the game make and no part of this crate ports. It is hooked, and the hook is
//! this proof's instrument twice over: it serves the answers the generator chose — so the ray's
//! several passes can be driven through every one of its exits — and it records the point it was
//! handed, which is the port's own arithmetic and is compared against the port's returned call list.
//!
//! # The curve table
//!
//! The five-degree tolerance the ray's loop stops on is one entry of the engine's shared
//! `{cos, sin}` table, so the table has to exist before the original's suspension runs at all: with
//! the global still null the tolerance reads as garbage below zero and the loop never runs once. The
//! harness builds it with the game's own builder and hands the port the same 6000 entries, which is
//! what makes the comparison bit-exact rather than approximate.

use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::wheel::{self, Call, Chassis, Wheel};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::RefCell;
use std::rc::Rc;

/// The wheel, laid out past anything the executable uses, and the three things it points at. The
/// length is the object's own size, and the whole of it is compared.
const WHEEL: u32 = 0x0130_0000;
const OWNER: u32 = 0x0131_0000;
/// What a proof compares and what it allows the original to write. The suspension and the
/// integration may not touch the chassis at all, so they are proved against the wheel alone and a
/// write to the owner would be caught; the tire forces are the step that pushes into the chassis, so
/// that proof compares both objects word for word.
type Regions = &'static [(&'static str, u32, u32)];
const WHEEL_ONLY: Regions = &[("wheel", WHEEL, 0x8c0)];
const WHEEL_AND_CHASSIS: Regions = &[("wheel", WHEEL, 0x8c0), ("chassis", OWNER, 0x2a0)];
const XFORM: u32 = 0x0132_0000;
const WORLD: u32 = 0x0133_0000;
const CURVE: u32 = 0x0140_0000;

/// One generated case: the wheel's state, the frame it hangs in, and the answers the ground query will
/// give — one per pass the ray is allowed.
#[derive(Clone, Debug)]
struct State {
    w: Wheel,
    c: Chassis,
    m: [Vf; 4],
    dt: F,
    answers: Vec<(Vf, Vf)>,
    /// The answers the chassis' transform gives, in the order the tire forces ask for them: the
    /// contact's normal and the tire's force turned into the chassis' frame, the force's point
    /// likewise, and the chassis' up turned out into the world.
    vectors: Vec<Vf>,
    /// The suspension geometry, in host arithmetic, for the coverage count alone — the proof itself
    /// never looks at these. `denom` is how squarely the contact's plane faces the suspension line
    /// and `along` where the line crosses it; the two of them pick the three ways the tire forces
    /// can come out with no compression at all.
    denom_hint: f32,
    along_hint: f32,
}

fn put_f(ee: &mut Ee, a: u32, v: F) {
    ee.set_f32(a, v.0);
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in [v.x(), v.y(), v.z(), v.w()].into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}
fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// Zero every byte of the wheel, so a byte the original writes and the port does not cannot hide
/// behind something left over from the case before.
fn reset(ee: &mut Ee, regions: Regions) {
    for &(_, base, len) in regions {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
}

/// The pointers out of the wheel, and the transform the owner holds. The wheel's own are rewritten by
/// [`poke`], because [`reset`] clears the words they live in.
fn wire(ee: &mut Ee) {
    ee.set_u32(WHEEL + 0x1e4, OWNER);
    ee.set_u32(OWNER + 0x254, XFORM);
}

/// Write a state into the wheel, at the offsets the original keeps it at.
fn poke(ee: &mut Ee, s: &State) {
    wire(ee);
    let w = &s.w;
    ee.set_u32(WHEEL + 0x1e0, if w.has_ground { WORLD } else { 0 });
    put_v(ee, WHEEL + 0xa0, w.mount);
    put_v(ee, WHEEL + 0x150, w.mount_world);
    put_v(ee, WHEEL + 0x90, w.probe);
    put_v(ee, WHEEL + 0x100, w.probe_end);
    put_v(ee, WHEEL + 0x160, w.probe_end_world);
    put_f(ee, WHEEL + 0x214, w.probe_length);
    put_v(ee, WHEEL + 0x200, w.ground_normal);
    ee.set_u32(WHEEL + 0xec, w.ray_steps as u32);
    ee.set_u32(WHEEL + 0xd0, w.ray_hit as u32);
    ee.set_u32(WHEEL + 0xd4, w.swept_contact as u32);
    ee.set_u32(WHEEL + 0xd8, w.terrain_wins as u32);
    put_v(ee, WHEEL + 0x120, w.contact_point);
    put_v(ee, WHEEL + 0x130, w.contact_normal);
    ee.set_u32(WHEEL + 0x38, w.steered as u32);
    ee.set_u32(WHEEL + 0x3c, w.balancing as u32);
    put_f(ee, WHEEL + 0x6c, w.steer_torque);
    put_f(ee, WHEEL + 0x70, w.steer_damping);
    put_f(ee, WHEEL + 0x78, w.steer_inv_inertia);
    put_f(ee, WHEEL + 0x7c, w.max_steer_angle);
    put_f(ee, WHEEL + 0x80, w.min_steer_angle);
    put_f(ee, WHEEL + 0x8ac, w.steer_angle);
    put_f(ee, WHEEL + 0x8b0, w.steer_rate);
    put_f(ee, WHEEL + 0x48, w.max_compression);
    put_f(ee, WHEEL + 0x4c, w.max_damper_speed);
    put_f(ee, WHEEL + 0x50, w.extension_rate);
    put_f(ee, WHEEL + 0x54, w.spring_rate);
    put_f(ee, WHEEL + 0x58, w.damping_out);
    put_f(ee, WHEEL + 0x5c, w.damping_in);
    ee.set_u32(WHEEL + 0xdc, w.contact as u32);
    put_f(ee, WHEEL + 0xf0, w.compression);
    put_f(ee, WHEEL + 0xf4, w.last_compression);
    put_f(ee, WHEEL + 0xf8, w.spring_force);
    put_v(ee, WHEEL + 0x140, w.load_point);
    put_v(ee, WHEEL + 0x170, w.contact_velocity);
    put_v(ee, WHEEL + 0x180, w.chassis_velocity);
    // The chassis, which the tire forces push into.
    put_v(ee, OWNER + 0x130, s.c.force);
    put_v(ee, OWNER + 0x140, s.c.torque);
    put_v(ee, OWNER + 0x170, s.c.accel);
    put_v(ee, OWNER + 0x190, s.c.cofg);
    put_v(ee, OWNER + 0x260, s.c.vel);
    // The frame, which lives in the owner's transform and is never written.
    for (k, row) in s.m.iter().enumerate() {
        put_v(ee, XFORM + 0x80 + 0x10 * k as u32, *row);
    }
}

/// The wheel as the port sees it, read back out of RAM. Used to check that [`poke`] is the exact
/// inverse of this — a state the harness cannot read back is a field the proof is not really testing.
fn peek(ee: &Ee) -> Wheel {
    Wheel {
        has_ground: ee.u32_at(WHEEL + 0x1e0) != 0,
        mount: get_v(ee, WHEEL + 0xa0),
        mount_world: get_v(ee, WHEEL + 0x150),
        probe: get_v(ee, WHEEL + 0x90),
        probe_end: get_v(ee, WHEEL + 0x100),
        probe_end_world: get_v(ee, WHEEL + 0x160),
        probe_length: F(ee.f32_at(WHEEL + 0x214)),
        ground_normal: get_v(ee, WHEEL + 0x200),
        ray_steps: ee.i32_at(WHEEL + 0xec),
        ray_hit: ee.i32_at(WHEEL + 0xd0),
        swept_contact: ee.i32_at(WHEEL + 0xd4),
        terrain_wins: ee.i32_at(WHEEL + 0xd8),
        contact_point: get_v(ee, WHEEL + 0x120),
        contact_normal: get_v(ee, WHEEL + 0x130),
        steered: ee.u32_at(WHEEL + 0x38) != 0,
        balancing: ee.u32_at(WHEEL + 0x3c) != 0,
        steer_torque: F(ee.f32_at(WHEEL + 0x6c)),
        steer_damping: F(ee.f32_at(WHEEL + 0x70)),
        steer_inv_inertia: F(ee.f32_at(WHEEL + 0x78)),
        max_steer_angle: F(ee.f32_at(WHEEL + 0x7c)),
        min_steer_angle: F(ee.f32_at(WHEEL + 0x80)),
        steer_angle: F(ee.f32_at(WHEEL + 0x8ac)),
        steer_rate: F(ee.f32_at(WHEEL + 0x8b0)),
        max_compression: F(ee.f32_at(WHEEL + 0x48)),
        max_damper_speed: F(ee.f32_at(WHEEL + 0x4c)),
        extension_rate: F(ee.f32_at(WHEEL + 0x50)),
        spring_rate: F(ee.f32_at(WHEEL + 0x54)),
        damping_out: F(ee.f32_at(WHEEL + 0x58)),
        damping_in: F(ee.f32_at(WHEEL + 0x5c)),
        contact: ee.u32_at(WHEEL + 0xdc) != 0,
        compression: F(ee.f32_at(WHEEL + 0xf0)),
        last_compression: F(ee.f32_at(WHEEL + 0xf4)),
        spring_force: F(ee.f32_at(WHEEL + 0xf8)),
        load_point: get_v(ee, WHEEL + 0x140),
        contact_velocity: get_v(ee, WHEEL + 0x170),
        chassis_velocity: get_v(ee, WHEEL + 0x180),
    }
}

/// The chassis, likewise.
fn peek_chassis(ee: &Ee) -> Chassis {
    Chassis {
        force: get_v(ee, OWNER + 0x130),
        torque: get_v(ee, OWNER + 0x140),
        accel: get_v(ee, OWNER + 0x170),
        cofg: get_v(ee, OWNER + 0x190),
        vel: get_v(ee, OWNER + 0x260),
    }
}

/// [`poke`] is the exact inverse of [`peek`] — a field one of them forgets is a field the proofs are
/// not really testing, since the comparison below would see the same leftover word on both sides.
fn round_trips(ee: &mut Ee, s: &State) {
    reset(ee, WHEEL_AND_CHASSIS);
    poke(ee, s);
    assert_eq!(&peek(ee), &s.w, "the harness cannot read back what it wrote");
    assert_eq!(&peek_chassis(ee), &s.c, "the harness cannot read back the chassis it wrote");
}

/// Every word of every region, in one vector, for the word-for-word comparison.
fn snapshot(ee: &Ee, regions: Regions) -> Vec<u32> {
    let mut v = Vec::new();
    for &(_, base, len) in regions {
        for o in (0..len).step_by(4) {
            v.push(ee.u32_at(base + o));
        }
    }
    v
}

/// Which field a differing word is, in the original's terms.
fn where_is(word: usize, regions: Regions) -> String {
    let mut i = word * 4;
    for &(name, _, len) in regions {
        if (i as u32) < len {
            return format!("{name} + {i:#x}");
        }
        i -= len as usize;
    }
    "past the end".into()
}

/// Nothing the original writes may land outside the wheel: a write to the owner, to the transform or
/// to a global would be state the port cannot have, and this is the test that would say so. The stack
/// frame is allowed — the point and the normal the query answers with live in it.
fn only_touched_the_regions(ee: &Ee, regions: Regions) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        let inside = regions.iter().any(|&(_, base, len)| a >= base && b <= base + len)
            || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!(
                "the original wrote {a:#010x}..{b:#010x}, which is none of the compared objects or the stack"));
        }
    }
    Ok(())
}

/// Compare two snapshots and say which field differs.
fn same(name: &str, k: u32, was: &[u32], now: &[u32], s: &State, regions: Regions) {
    if was == now {
        return;
    }
    let i = was.iter().zip(now).position(|(a, b)| a != b).unwrap();
    let (a, b) = (was[i], now[i]);
    panic!(
        "{name}, state {k}: {} is {a:#010x} ({}) in the original and {b:#010x} ({}) in the port\n\
         and {} word(s) differ in all\n{s:#?}",
        where_is(i, regions),
        f32::from_bits(a),
        f32::from_bits(b),
        was.iter().zip(now).filter(|(a, b)| a != b).count(),
    );
}

// --- the tracer -----------------------------------------------------------------------------------

/// What the ground query's hook records, and the answers it serves. One per machine, shared with the
/// hook.
#[derive(Default)]
struct Tape {
    calls: Vec<Call>,
    /// The (point, normal) the query answers with, in order, and how far through them it is. The
    /// generator chooses these and the port's closure is handed the same list, so both sides see the
    /// same world.
    answers: Vec<(Vf, Vf)>,
    at: usize,
    /// The same for the chassis' transform: one answer per question, in order, shared with the port.
    vectors: Vec<Vf>,
    vat: usize,
}

/// A machine with a wheel wired up, the curve table built by the game's own builder, and the ground
/// query hooked.
fn harness() -> Option<(Ee, CurveTable, Rc<RefCell<Tape>>)> {
    let mut ee = game_or_skip()?;
    // The shared {cos, sin} table, where the harness wants it rather than wherever the allocator
    // would have put it. Without it the suspension's tolerance reads garbage and its loop never runs.
    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, CURVE as u64)));
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    ee.hooks.remove(&addr("mem_alloc").unwrap());
    assert_eq!(ee.u32_at(addr("curve_table_ptr").unwrap()), CURVE, "the table went somewhere else");
    let entries: Vec<[f32; 2]> =
        (0..CURVE_ENTRIES as u32).map(|i| [ee.f32_at(CURVE + 8 * i), ee.f32_at(CURVE + 8 * i + 4)]).collect();
    let curve = CurveTable::from_entries(&entries);

    wire(&mut ee);
    let tape = Rc::new(RefCell::new(Tape::default()));
    let t = tape.clone();
    ee.hooks.insert(
        addr("ground_query").unwrap(),
        Box::new(move |ee| {
            let (point, normal) = (ee.r(5) as u32, ee.r(6) as u32);
            assert_eq!(ee.r(4) as u32, WORLD, "the query went to something that is not the world");
            let mut t = t.borrow_mut();
            t.calls.push(Call::GroundQuery(get_v(ee, point)));
            let (p, n) = t.answers[t.at.min(t.answers.len() - 1)];
            t.at += 1;
            put_v(ee, point, p);
            put_v(ee, normal, n);
        }),
    );
    // The two halves of the integration that are functions of their own. Each records the `dt` it was
    // handed, which is how the port's claim about what it passes on gets proved.
    for (name, make) in [("wheel_surface_drag", Call::SurfaceDrag as fn(F) -> Call),
                         ("wheel_spin_integrate", Call::SpinIntegrate)] {
        let t = tape.clone();
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee| {
                assert_eq!(ee.r(4) as u32, WHEEL, "a callee was handed something that is not the wheel");
                t.borrow_mut().calls.push(make(F(ee.f(12))));
            }),
        );
    }
    // The chassis' transform, asked three questions. Each writes its answer into the out pointer the
    // caller handed it, which is how the generator's own vectors get into the original's arithmetic,
    // and records the vector it was asked about — the port's own work, compared below.
    for (name, make) in [("transform_unrotate", Call::Unrotate as fn(Vf) -> Call),
                         ("transform_rotate", Call::Rotate), ("transform_unpoint", Call::Unpoint)] {
        let t = tape.clone();
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee| {
                let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
                assert_eq!(ee.r(5) as u32, XFORM, "a transform call went to something else");
                let mut t = t.borrow_mut();
                // The vector asked about is read out of the original's own memory, which is what
                // proves the constant `+y` the bottom-out push rotates: nothing here supplies it.
                t.calls.push(make(get_v(ee, v)));
                let answer = t.vectors[t.vat.min(t.vectors.len() - 1)];
                t.vat += 1;
                put_v(ee, out, answer);
            }),
        );
    }
    // The three with no answer. The contact velocity writes five fields of the wheel in the original
    // and nothing here, so both sides read the generated ones and see the same numbers; the journal
    // entry says why that is the honest way round.
    let t = tape.clone();
    ee.hooks.insert(
        addr("wheel_contact_velocity").unwrap(),
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WHEEL, "the contact velocity was asked about another wheel");
            t.borrow_mut().calls.push(Call::ContactVelocity);
        }),
    );
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_sync").unwrap(),
        Box::new(move |ee| {
            assert_eq!((ee.r(4) as u32, ee.r(5)), (XFORM, 0), "the transform sync's arguments changed");
            t.borrow_mut().calls.push(Call::TransformSync);
        }),
    );
    let t = tape.clone();
    ee.hooks.insert(
        addr("wheel_hinge_force").unwrap(),
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, WHEEL, "the hinge was handed another wheel");
            assert_eq!(ee.f(12), 1.0, "the hinge's force scale is not one");
            let (force, point) = (ee.r(5) as u32, ee.r(6) as u32);
            t.borrow_mut().calls.push(Call::HingeForce(get_v(ee, force), get_v(ee, point)));
        }),
    );
    Some((ee, curve, tape))
}

/// Run one case through the original and hand back the wheel afterwards, the words of it, and what it
/// called.
fn original(ee: &mut Ee, tape: &Rc<RefCell<Tape>>, s: &State, entry: &str, regions: Regions)
            -> (Vec<u32>, Vec<Call>) {
    reset(ee, regions);
    poke(ee, s);
    *tape.borrow_mut() =
        Tape { calls: Vec::new(), answers: s.answers.clone(), at: 0, vectors: s.vectors.clone(), vat: 0 };
    ee.call(addr(entry).unwrap(), &[WHEEL as u64], &[s.dt.0], 200_000);
    only_touched_the_regions(ee, regions).unwrap();
    (snapshot(ee, regions), std::mem::take(&mut tape.borrow_mut().calls))
}

// --- the generator --------------------------------------------------------------------------------

/// A generator. Plain host arithmetic: these are only inputs, and every one of them goes into the
/// original and into the port alike.
struct Gen(u64);

#[allow(clippy::float_arithmetic)]
impl Gen {
    fn bits(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
    fn unit(&mut self) -> f32 {
        self.bits() as f32 / 4294967296.0
    }
    fn f(&mut self, lo: f32, hi: f32) -> F {
        F(lo + (hi - lo) * self.unit())
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    /// A direction: a unit vector most of the time, occasionally exactly nothing, so that what the
    /// VU's reciprocal square root does with a zero length is proved rather than avoided.
    fn dir(&mut self) -> Vf {
        if self.one_in(31) {
            return Vf::new(0.0, 0.0, 0.0, self.f(-1.0, 1.0).0);
        }
        let v = self.vec(-1.0, 1.0);
        let n = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z()).sqrt().max(1e-6);
        Vf::new(v.x() / n, v.y() / n, v.z() / n, self.f(-1.0, 1.0).0)
    }
    /// A frame: three axes near enough to orthonormal to be a bike's, and an origin.
    fn frame(&mut self) -> [Vf; 4] {
        [self.dir(), self.dir(), self.dir(), Vf::new(self.f(-200.0, 200.0).0, self.f(-20.0, 20.0).0,
                                                     self.f(-200.0, 200.0).0, 1.0)]
    }
    /// A point through a frame, in host arithmetic: where the generator puts the contact so that the
    /// suspension's geometry comes out on the scale a real wheel's does.
    fn through(m: &[Vf; 4], p: [f32; 3]) -> [f32; 3] {
        let lane = |v: Vf, k: usize| [v.x(), v.y(), v.z()][k];
        [0, 1, 2].map(|k| {
            lane(m[3], k) + lane(m[0], k) * p[0] + lane(m[1], k) * p[1] + lane(m[2], k) * p[2]
        })
    }
    /// One case.
    fn state(&mut self) -> State {
        let m = self.frame();
        let normal = self.dir();
        let (mount, probe) = (self.vec(-2.0, 2.0), self.vec(-2.0, 2.0));
        // Where the suspension line runs, as the tire forces will rebuild it: from the probe's end
        // back up to the mount. The contact goes near the probe's end, which is what makes `along` a
        // fraction rather than a thousand, and the compression the length of a suspension and not of
        // a map.
        let mw = Self::through(&m, [mount.x(), mount.y(), mount.z()]);
        let pe = Self::through(&m, [mount.x() + probe.x(), mount.y() + probe.y(), mount.z() + probe.z()]);
        let off = self.dir();
        let reach = self.f(-3.0, 3.0).0;
        let contact_point = Vf::new(pe[0] + off.x() * reach, pe[1] + off.y() * reach,
                                    pe[2] + off.z() * reach, self.f(-1.0, 1.0).0);
        let contact_normal = self.dir();
        let n = [contact_normal.x(), contact_normal.y(), contact_normal.z()];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let denom_hint = dot([0, 1, 2].map(|k| mw[k] - pe[k]), n);
        let along_hint = dot([0, 1, 2].map(|k| [contact_point.x(), contact_point.y(), contact_point.z()][k] - pe[k]), n)
            / denom_hint;

        // The answers the query will give. Each pass compares its answer's normal with the last, so
        // repeating one ends the loop: this is what drives the ray through one pass, a few, and all
        // five of the passes it is allowed.
        let stop_after = if self.one_in(3) { 1 + self.bits() % 5 } else { 9 };
        let mut answers: Vec<(Vf, Vf)> = Vec::new();
        for i in 0..6 {
            let p = Vf::new(self.f(-200.0, 200.0).0, self.f(-20.0, 20.0).0, self.f(-200.0, 200.0).0,
                            self.f(-1.0, 1.0).0);
            let n = if i as u32 + 1 == stop_after {
                if i == 0 { normal } else { answers[i - 1].1 }
            } else {
                self.dir()
            };
            answers.push((p, n));
        }
        // What the transform answers: a direction half the time and a point the other half, since
        // two of the three questions are about directions and one is about a point.
        let vectors = (0..6).map(|_| if self.one_in(2) { self.dir() } else { self.vec(-5.0, 5.0) }).collect();
        State {
            w: Wheel {
                has_ground: !self.one_in(17),
                mount,
                mount_world: self.vec(-200.0, 200.0),
                probe,
                probe_end: self.vec(-2.0, 2.0),
                probe_end_world: self.vec(-200.0, 200.0),
                probe_length: self.f(0.1, 1.5),
                ground_normal: normal,
                ray_steps: (self.bits() % 7) as i32,
                ray_hit: self.one_in(2) as i32,
                swept_contact: self.one_in(2) as i32,
                terrain_wins: self.one_in(2) as i32,
                contact_point,
                contact_normal,
                // A travel most cases bottom out against, and one in three wide enough that they do
                // not. Never negative: a negative travel is the one input that would make the
                // original read the register its division never wrote (see `wheel.rs`).
                max_compression: if self.one_in(3) { self.f(0.0, 4.0) } else { self.f(0.0, 0.4) },
                max_damper_speed: self.f(0.0, 8.0),
                extension_rate: self.f(0.0, 20.0),
                spring_rate: self.f(0.0, 4000.0),
                damping_out: self.f(0.0, 300.0),
                damping_in: self.f(0.0, 300.0),
                // The three the step overwrites before it reads them. Generated anyway: if the
                // original read one of them, the comparison would say so.
                contact: self.one_in(2),
                compression: self.f(-1.0, 3.0),
                last_compression: self.f(-0.5, 3.0),
                spring_force: self.f(-100.0, 100.0),
                load_point: self.vec(-5.0, 5.0),
                contact_velocity: self.vec(-10.0, 10.0),
                chassis_velocity: self.vec(-10.0, 10.0),
                steered: !self.one_in(3),
                balancing: !self.one_in(3),
                steer_torque: self.f(-400.0, 400.0),
                steer_damping: self.f(0.0, 20.0),
                steer_inv_inertia: self.f(0.0, 60.0),
                // Wide enough that both ends of the lock are reached, and sometimes crossed over so
                // that a minimum above the maximum proves which clamp wins.
                max_steer_angle: self.f(-0.9, 0.9),
                min_steer_angle: self.f(-0.9, 0.9),
                steer_angle: self.f(-1.2, 1.2),
                steer_rate: self.f(-8.0, 8.0),
            },
            c: Chassis {
                force: self.vec(-4000.0, 4000.0),
                torque: self.vec(-4000.0, 4000.0),
                accel: self.vec(-40.0, 40.0),
                cofg: self.vec(-2.0, 2.0),
                vel: self.vec(-30.0, 30.0),
            },
            m,
            dt: self.f(0.001, 0.05),
            answers,
            vectors,
            denom_hint,
            along_hint,
        }
    }
}

// --- the proof ------------------------------------------------------------------------------------

/// **The suspension ray**, over ten thousand generated wheels.
///
/// Every word of the wheel object is compared, and so is the ordered list of ground queries with the
/// point each was handed. The `dt` the ground step passes in is generated and random on both sides and
/// the port takes none at all: if the original read it, that would show here.
#[test]
fn the_suspension_ray_matches_the_original() {
    let Some((mut ee, curve, tape)) = harness() else { return };
    let mut g = Gen(0x5eed_51_u64);
    // Which exits were reached, so a case the generator stops producing cannot quietly go untested.
    let mut seen = [0u32; 8];
    for k in 0..10_000 {
        let s = g.state();
        round_trips(&mut ee, &s);
        let (was, calls) = original(&mut ee, &tape, &s, "wheel_suspension", WHEEL_ONLY);

        // The port, over the same state and the same world.
        let mut w = s.w;
        let mut at = 0usize;
        let answers = s.answers.clone();
        let mut ours = wheel::suspension(&mut w, &s.m, &curve, &mut |_| {
            let a = answers[at.min(answers.len() - 1)];
            at += 1;
            a
        });
        assert_eq!(std::mem::take(&mut ours), calls, "state {k}: the ground queries differ\n{s:#?}");

        reset(&mut ee, WHEEL_ONLY);
        poke(&mut ee, &State { w, ..s.clone() });
        same("the suspension ray", k, &was, &snapshot(&ee, WHEEL_ONLY), &s, WHEEL_ONLY);

        seen[(w.ray_steps as usize).min(6)] += 1;
        seen[7] += (w.ray_hit != 0) as u32;
    }
    for (i, n) in seen.iter().enumerate().take(6).skip(1) {
        assert!(*n > 0, "no case ran the ray exactly {i} time(s): {seen:?}");
    }
    assert!(seen[0] > 0, "no case skipped the ray altogether (a wheel with no world): {seen:?}");
    assert!(seen[7] > 0, "no case found ground under the wheel: {seen:?}");
}

/// **The wheel's own integration**, over ten thousand generated wheels.
///
/// Two of its three parts are callees and are hooked, so what is left in the object afterwards is the
/// steering hinge, and that is what the word-for-word comparison is of. All three of the dispatch's
/// cases are counted, and so is each end of the steering lock, because a clamp the generator stops
/// reaching is a clamp nothing checks.
#[test]
fn the_wheels_own_integration_matches_the_original() {
    let Some((mut ee, _curve, tape)) = harness() else { return };
    let mut g = Gen(0x5eed_16_u64);
    let mut seen = [0u32; 5];
    for k in 0..10_000 {
        let s = g.state();
        round_trips(&mut ee, &s);
        let (was, calls) = original(&mut ee, &tape, &s, "wheel_integrate", WHEEL_ONLY);

        let mut w = s.w;
        let ours = wheel::integrate(&mut w, s.dt);
        assert_eq!(ours, calls, "state {k}: the calls differ\n{s:#?}");

        reset(&mut ee, WHEEL_ONLY);
        poke(&mut ee, &State { w, ..s.clone() });
        same("the wheel's integration", k, &was, &snapshot(&ee, WHEEL_ONLY), &s, WHEEL_ONLY);

        seen[match (s.w.steered, s.w.balancing) {
            (false, _) => 0,
            (true, false) => 1,
            (true, true) => 2,
        }] += 1;
        if s.w.steered && s.w.balancing && w.steer_rate == F::ZERO {
            seen[3 + (w.steer_angle == s.w.min_steer_angle) as usize] += 1;
        }
    }
    assert!(seen.iter().all(|&n| n > 0), "a case of the steering went untested: {seen:?}");
}

/// **The tire's forces**, over ten thousand generated wheels and the chassis under them.
///
/// Every word of both objects is compared, and so is the ordered list of everything the step asked
/// of the world — the chassis' transform three times over, the contact velocity once or twice, and
/// either the steering hinge or nothing — with the vectors it handed each one. The answers come from
/// the generator, so the branch a lean or a force sends the step down is chosen rather than waited
/// for, and the counts at the end say every branch was taken.
#[test]
#[allow(clippy::float_arithmetic)] // the coverage count is host arithmetic; the proof is not
fn the_tires_forces_match_the_original() {
    let Some((mut ee, _curve, tape)) = harness() else { return };
    let mut g = Gen(0x5eed_f0_u64);
    let mut seen = [0u32; 14];
    for k in 0..10_000 {
        let s = g.state();
        round_trips(&mut ee, &s);
        let (was, calls) = original(&mut ee, &tape, &s, "wheel_tire_forces", WHEEL_AND_CHASSIS);

        // The port, over the same state and the same answers.
        let (mut w, mut c) = (s.w, s.c);
        let (vectors, mut at) = (s.vectors.clone(), 0usize);
        let ours = wheel::tire_forces(&mut w, &mut c, &s.m, s.dt, &mut |call| match call {
            Call::Unrotate(_) | Call::Rotate(_) | Call::Unpoint(_) => {
                at += 1;
                vectors[at - 1]
            }
            _ => Vf::default(),
        });
        assert_eq!(ours, calls, "state {k}: the calls differ\n{s:#?}");

        reset(&mut ee, WHEEL_AND_CHASSIS);
        poke(&mut ee, &State { w, c, ..s.clone() });
        same("the tire's forces", k, &was, &snapshot(&ee, WHEEL_AND_CHASSIS), &s, WHEEL_AND_CHASSIS);

        // Which way each branch went, counted from what the step asked the world and what it left
        // behind, so that a branch the generator stops reaching cannot go quietly untested.
        let asked = |f: &dyn Fn(&Call) -> bool| calls.iter().any(f);
        let velocities = calls.iter().filter(|c| **c == Call::ContactVelocity).count();
        if s.w.ray_hit == 0 && s.w.swept_contact == 0 {
            seen[0] += 1;
        } else if !w.contact {
            seen[1 + (s.denom_hint > 1e-06 && s.along_hint <= 0.0) as usize] += 1;
        }
        if w.contact {
            seen[3 + (velocities == 2) as usize + (velocities == 2 && asked(&|c| matches!(c, Call::Rotate(_)))) as usize] += 1;
            let closing = s.w.contact_velocity.x() * s.w.contact_normal.x()
                + s.w.contact_velocity.y() * s.w.contact_normal.y()
                + s.w.contact_velocity.z() * s.w.contact_normal.z();
            seen[6 + (closing.clamp(-s.w.max_damper_speed.0, s.w.max_damper_speed.0) > 0.0) as usize] += 1;
            seen[8 + asked(&|c| matches!(c, Call::Unpoint(_))) as usize] += 1;
            seen[10 + (w.spring_force == F::ZERO) as usize] += 1;
            // The lean the load scale is taken from, flattened and normalised as the step does it.
            let v = s.vectors[0];
            let len = (v.x() * v.x() + v.y() * v.y()).sqrt();
            let lean = if len == 0.0 { 0.0 } else { v.y() / len };
            seen[12] += (lean > 0.5 && lean < 0.64) as u32;
        }
        // The extension rate limit, caught where only it can have acted: no contact at all, and a
        // compression the tail nonetheless left standing.
        seen[13] += (!w.contact && w.compression != F::ZERO) as u32;
    }
    let names = ["no contact", "the plane parallel to the line", "the plane behind the wheel",
                 "a contact inside the travel", "bottomed out, no push", "bottomed out with the push",
                 "the damper closing", "the damper extending", "the force through the hinge",
                 "the force into the chassis", "a spring force", "the spring force clamped to zero",
                 "the load scale between its ends", "the extension rate limit"];
    for (n, name) in seen.iter().zip(names) {
        assert!(*n > 0, "nothing reached {name}: {seen:?}");
    }
}
