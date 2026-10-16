//! S19 — the chassis, proved against the original.
//!
//! The same shape as `tests/wheel.rs` one object over: the harness lays a chassis out in the
//! interpreter's RAM, writes a generated state into it, runs the original, then writes the *same*
//! state again, applies the port over the top and compares **every word** of the 0x330-byte object.
//! A field the original writes and the port does not model is a difference, not an omission nobody
//! notices.
//!
//! The chassis is laid out past anything the executable uses, and so is its transform. The transform
//! is never written here — the two calls that write it are the transform's own and are hooked — so
//! [`only_touched_the_regions`] is what says the original stayed inside the body.
//!
//! # The vtable
//!
//! The step dispatches through three virtuals on itself (the contact count cleared, the velocity
//! into the body's frame, the angular velocity into the world) and through a function pointer it
//! keeps at `+0x25c` (the integrator). All four are wired to the *real* addresses rather than to
//! stand-ins, because all four are ported — and `the_vtable_is_what_this_module_ports` checks that
//! the chassis' own vtable in the image still holds the five entries this module was written from.
//!
//! # The seams
//!
//! What the step calls and this module does not port is hooked, and each hook is the proof's
//! instrument twice over: it serves the answer the generator chose, so every branch below it can be
//! driven, and it records the vector it was handed — which is the port's own arithmetic, compared
//! against the call list the port returns.

use game_sim::chassis::{
    self, CLEAR, Call, Chassis, Contact, ContactCall, Deriv, Given, Mat3, WheelContact,
};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::RefCell;
use std::rc::Rc;

/// The chassis and the transform it holds, both past the end of anything the game uses. The length
/// is the object's own size, and the whole of it is compared.
const CHASSIS: u32 = 0x0130_0000;
const XFORM: u32 = 0x0131_0000;
const SIZE: u32 = 0x330;

type Regions = &'static [(&'static str, u32, u32)];
const BODY: Regions = &[("chassis", CHASSIS, SIZE)];

/// Scratch for the pieces that are proved on their own: the out-parameter the transform's rotate
/// pair writes, and the two arrays the integrator walks.
const OUT: u32 = 0x0132_0000;
const X: u32 = 0x0133_0000;
const DX: u32 = 0x0134_0000;
const ROTATED: Regions = &[("out", OUT, 0x10)];
const INTEGRATED: Regions = &[("x", X, 0x80), ("dx", DX, 0x100)];

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
fn put_3(ee: &mut Ee, a: u32, v: [F; 3]) {
    for (k, x) in v.into_iter().enumerate() {
        put_f(ee, a + 4 * k as u32, x);
    }
}
fn get_3(ee: &Ee, a: u32) -> [F; 3] {
    [0, 1, 2].map(|k| F(ee.f32_at(a + 4 * k)))
}

/// Zero every byte of the compared regions, so a byte the original writes and the port does not
/// cannot hide behind something left over from the case before.
fn reset(ee: &mut Ee, regions: Regions) {
    for &(_, base, len) in regions {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
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

/// Nothing the original writes may land outside the compared objects: a write to the transform, to a
/// global or past the end of the body would be state the port cannot have, and this is the test that
/// would say so. The stack frame is allowed — every answer a hooked callee gives is written into it.
fn only_touched_the_regions(ee: &Ee, regions: Regions) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        let inside = regions.iter().any(|&(_, base, len)| a >= base && b <= base + len)
            || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!(
                "the original wrote {a:#010x}..{b:#010x}, which is none of the compared objects or the stack"
            ));
        }
    }
    Ok(())
}

/// Compare two snapshots and say which field differs.
fn same<S: std::fmt::Debug>(name: &str, k: u32, was: &[u32], now: &[u32], s: &S, regions: Regions) {
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

// --- the body in memory ---------------------------------------------------------------------------

/// The pointers out of the chassis: its class, its transform and the integrator it calls through
/// itself. All three are the real ones — this module ports what they point at.
fn wire(ee: &mut Ee) {
    ee.set_u32(CHASSIS + 0x8, addr("chassis_vtable").unwrap());
    ee.set_u32(CHASSIS + 0x254, XFORM);
    ee.set_u32(CHASSIS + 0x25c, addr("integrate_ab2").unwrap());
    // The transform is already up to date, so the lazy rebuild the rotate pair begins with returns
    // at once and leaves the rows the generator wrote alone.
    ee.set_u32(XFORM + 0x110, 1);
}

/// Write a chassis into RAM, at the offsets the original keeps it at.
fn poke(ee: &mut Ee, c: &Chassis) {
    wire(ee);
    put_f(ee, CHASSIS + 0x34, c.speed);
    put_v(ee, CHASSIS + 0x60, c.cofg_world);
    put_v(ee, CHASSIS + 0x70, c.accel);
    put_v(ee, CHASSIS + 0x80, c.ang_accel);
    put_v(ee, CHASSIS + 0x90, c.euler);
    for (k, row) in c.inv_inertia.rows.iter().enumerate() {
        put_v(ee, CHASSIS + 0xf0 + 0x10 * k as u32, *row);
    }
    put_v(ee, CHASSIS + 0x130, c.force);
    put_v(ee, CHASSIS + 0x140, c.torque);
    put_v(ee, CHASSIS + 0x150, c.impulse);
    put_v(ee, CHASSIS + 0x160, c.correction);
    put_v(ee, CHASSIS + 0x190, c.cofg);
    put_v(ee, CHASSIS + 0x1a0, c.quat);
    ee.set_u32(CHASSIS + 0x1b4, c.awake as u32);
    ee.set_u32(CHASSIS + 0x1e0, c.contacts as u32);
    put_f(ee, CHASSIS + 0x214, c.inv_mass);
    put_f(ee, CHASSIS + 0x220, c.max_speed_sq);
    put_f(ee, CHASSIS + 0x224, c.max_ang_speed_sq);
    put_3(ee, CHASSIS + 0x230, c.ang_vel_max);
    put_3(ee, CHASSIS + 0x240, c.ang_vel_min);
    put_v(ee, CHASSIS + 0x260, c.vel);
    put_v(ee, CHASSIS + 0x270, c.local_vel);
    put_v(ee, CHASSIS + 0x280, c.world_ang_vel);
    put_v(ee, CHASSIS + 0x290, c.ang_vel);
    for k in 0..3 {
        ee.set_u32(CHASSIS + 0x2a0 + 4 * k as u32, c.free[k] as u32);
        ee.set_u32(CHASSIS + 0x2ac + 4 * k as u32, c.ang_free[k] as u32);
    }
    put_3(ee, CHASSIS + 0x2b8, c.pos_hist.dx);
    put_3(ee, CHASSIS + 0x2c4, c.pos_hist.prev);
    put_3(ee, CHASSIS + 0x2d0, c.vel_hist.dx);
    put_3(ee, CHASSIS + 0x2dc, c.vel_hist.prev);
    put_3(ee, CHASSIS + 0x310, c.ang_hist.dx);
    put_3(ee, CHASSIS + 0x31c, c.ang_hist.prev);
    for k in 0..4 {
        put_f(ee, CHASSIS + 0x2f0 + 4 * k as u32, c.quat_hist.dx[k]);
        put_f(ee, CHASSIS + 0x300 + 4 * k as u32, c.quat_hist.prev[k]);
    }
}

/// The chassis as the port sees it, read back out of RAM. Used to check that [`poke`] is the exact
/// inverse of this — a field one of them forgets is a field the proofs are not really testing, since
/// the comparison would see the same leftover word on both sides.
fn peek(ee: &Ee) -> Chassis {
    Chassis {
        speed: F(ee.f32_at(CHASSIS + 0x34)),
        cofg_world: get_v(ee, CHASSIS + 0x60),
        accel: get_v(ee, CHASSIS + 0x70),
        ang_accel: get_v(ee, CHASSIS + 0x80),
        euler: get_v(ee, CHASSIS + 0x90),
        inv_inertia: Mat3 {
            rows: [0, 1, 2].map(|k| get_v(ee, CHASSIS + 0xf0 + 0x10 * k)),
        },
        force: get_v(ee, CHASSIS + 0x130),
        torque: get_v(ee, CHASSIS + 0x140),
        impulse: get_v(ee, CHASSIS + 0x150),
        correction: get_v(ee, CHASSIS + 0x160),
        cofg: get_v(ee, CHASSIS + 0x190),
        quat: get_v(ee, CHASSIS + 0x1a0),
        awake: ee.i32_at(CHASSIS + 0x1b4),
        contacts: ee.i32_at(CHASSIS + 0x1e0),
        inv_mass: F(ee.f32_at(CHASSIS + 0x214)),
        max_speed_sq: F(ee.f32_at(CHASSIS + 0x220)),
        max_ang_speed_sq: F(ee.f32_at(CHASSIS + 0x224)),
        ang_vel_max: get_3(ee, CHASSIS + 0x230),
        ang_vel_min: get_3(ee, CHASSIS + 0x240),
        vel: get_v(ee, CHASSIS + 0x260),
        local_vel: get_v(ee, CHASSIS + 0x270),
        world_ang_vel: get_v(ee, CHASSIS + 0x280),
        ang_vel: get_v(ee, CHASSIS + 0x290),
        free: [0, 1, 2].map(|k| ee.i32_at(CHASSIS + 0x2a0 + 4 * k)),
        ang_free: [0, 1, 2].map(|k| ee.i32_at(CHASSIS + 0x2ac + 4 * k)),
        pos_hist: Deriv { dx: get_3(ee, CHASSIS + 0x2b8), prev: get_3(ee, CHASSIS + 0x2c4) },
        vel_hist: Deriv { dx: get_3(ee, CHASSIS + 0x2d0), prev: get_3(ee, CHASSIS + 0x2dc) },
        ang_hist: Deriv { dx: get_3(ee, CHASSIS + 0x310), prev: get_3(ee, CHASSIS + 0x31c) },
        quat_hist: Deriv {
            dx: [0, 1, 2, 3].map(|k| F(ee.f32_at(CHASSIS + 0x2f0 + 4 * k))),
            prev: [0, 1, 2, 3].map(|k| F(ee.f32_at(CHASSIS + 0x300 + 4 * k))),
        },
    }
}

fn round_trips(ee: &mut Ee, c: &Chassis) {
    reset(ee, BODY);
    poke(ee, c);
    assert_eq!(&peek(ee), c, "the harness cannot read back what it wrote");
}

// --- the seams ------------------------------------------------------------------------------------

/// What the step called, and what the generator wants the callees to answer. The hooks read their
/// answers out of here and write what they were asked into `calls`.
#[derive(Default)]
struct Tape {
    calls: Vec<Call>,
    answers: Option<State>,
    /// The transform's rotate is asked twice — the centre of gravity first, the angular velocity
    /// last — and each gets its own answer.
    rotates: usize,
}

/// One case: a body, a step length, the answers its callees give, and the three angles the measure's
/// callee gives.
#[derive(Clone, Debug)]
struct State {
    ch: Chassis,
    dt: F,
    g: Given,
    euler: Vf,
}

/// An interpreter with every callee this module does not port hooked. Each hook serves the
/// generator's answer and records the vector it was handed, which is the port's own arithmetic.
fn harness() -> Option<(Ee, Rc<RefCell<Tape>>)> {
    let mut ee = game_or_skip()?;
    wire(&mut ee);
    let tape = Rc::new(RefCell::new(Tape::default()));

    // The centre of gravity, through the transform: the body's own frame to the world.
    let t = tape.clone();
    ee.hooks.insert(
        addr("orientation_transform_point").unwrap(),
        Box::new(move |ee| {
            let (out, point) = (ee.r(4) as u32, ee.r(6) as u32);
            assert_eq!(ee.r(5) as u32, XFORM, "the point went through another transform");
            let mut t = t.borrow_mut();
            t.calls.push(Call::PointCofg(get_v(ee, point)));
            let a = t.answers.as_ref().unwrap().g.cofg_world;
            put_v(ee, out, a);
        }),
    );
    // The transform's rotation, as a quaternion. Its argument is the rotation part of the matrix,
    // three rows in at +0x80, which is the one call here that is handed a pointer into the object
    // rather than the object.
    let t = tape.clone();
    ee.hooks.insert(
        addr("mat4_to_quat").unwrap(),
        Box::new(move |ee| {
            let out = ee.r(4) as u32;
            assert_eq!(ee.r(5) as u32, XFORM + 0x80, "the matrix read is not the transform's rows");
            let mut t = t.borrow_mut();
            t.calls.push(Call::MatrixQuat);
            let a = t.answers.as_ref().unwrap().g.matrix_quat;
            put_v(ee, out, a);
        }),
    );
    // The quaternion's derivative for an angular velocity. Both of its arguments are read back out
    // of the chassis, so the port's claim about *which* quaternion and *which* velocity it hands
    // over — the ones as they stand at that point in the step, not as they came in — is what is
    // being compared.
    let t = tape.clone();
    ee.hooks.insert(
        addr("quat_deriv").unwrap(),
        Box::new(move |ee| {
            let (out, q, w) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
            let mut t = t.borrow_mut();
            t.calls.push(Call::QuatDeriv(get_v(ee, q), get_v(ee, w)));
            let a = t.answers.as_ref().unwrap().g.quat_deriv;
            put_v(ee, out, a);
        }),
    );
    // The correction's axis and angle made into a quaternion. The angle comes in f12.
    let t = tape.clone();
    ee.hooks.insert(
        addr("quat_from_axis_angle").unwrap(),
        Box::new(move |ee| {
            let (out, axis) = (ee.r(4) as u32, ee.r(5) as u32);
            let mut t = t.borrow_mut();
            t.calls.push(Call::CorrectionQuat(get_v(ee, axis), F(ee.f(12))));
            let a = t.answers.as_ref().unwrap().g.correction_quat;
            put_v(ee, out, a);
        }),
    );
    // The transform's rotate, twice: the centre of gravity on the way to the new origin, and then
    // the angular velocity into the world through the chassis' own +0xc0 virtual.
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_rotate").unwrap(),
        Box::new(move |ee| {
            let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
            assert_eq!(ee.r(5) as u32, XFORM, "the rotate went through another transform");
            let mut t = t.borrow_mut();
            let (call, a) = {
                let g = t.answers.as_ref().unwrap().g;
                match t.rotates {
                    0 => (Call::RotateCofg(get_v(ee, v)), g.rotated_cofg),
                    1 => (Call::Rotate(get_v(ee, v)), g.world_ang_vel),
                    n => panic!("the transform's rotate was asked {} times", n + 1),
                }
            };
            t.rotates += 1;
            t.calls.push(call);
            put_v(ee, out, a);
        }),
    );
    // The transform's unrotate, once, through the +0xb4 virtual: the velocity in the body's frame.
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_unrotate").unwrap(),
        Box::new(move |ee| {
            let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
            assert_eq!(ee.r(5) as u32, XFORM, "the unrotate went through another transform");
            let mut t = t.borrow_mut();
            t.calls.push(Call::Unrotate(get_v(ee, v)));
            let a = t.answers.as_ref().unwrap().g.local_vel;
            put_v(ee, out, a);
        }),
    );
    // The two with no answer: the orientation written back onto the transform, and the flag that
    // says the frames below it are stale. Both would write the transform, which is why it can be
    // left out of the compared regions and the write check kept strict.
    let t = tape.clone();
    ee.hooks.insert(
        addr("orientation_set").unwrap(),
        Box::new(move |ee| {
            let (q, pos) = (ee.r(5) as u32, ee.r(6) as u32);
            assert_eq!(ee.r(4) as u32, XFORM, "the orientation was set on something else");
            t.borrow_mut().calls.push(Call::SetTransform(get_v(ee, q), get_v(ee, pos)));
        }),
    );
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_touch").unwrap(),
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, XFORM, "another transform was touched");
            t.borrow_mut().calls.push(Call::TouchTransform);
        }),
    );
    // The measure's two: the orientation as three angles, and the base class' own per-frame update.
    let t = tape.clone();
    ee.hooks.insert(
        addr("quat_to_euler").unwrap(),
        Box::new(move |ee| {
            let (out, q) = (ee.r(4) as u32, ee.r(5) as u32);
            let mut t = t.borrow_mut();
            t.calls.push(Call::QuatToEuler(get_v(ee, q)));
            let a = t.answers.as_ref().unwrap().euler;
            put_v(ee, out, a);
        }),
    );
    let t = tape.clone();
    ee.hooks.insert(
        addr("chassis_base_update").unwrap(),
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, CHASSIS, "the base update was handed another body");
            t.borrow_mut().calls.push(Call::FrameUpdate(F(ee.f(12))));
        }),
    );
    Some((ee, tape))
}

/// Run one case through the original and hand back the words of the body afterwards and what it
/// called.
fn original(ee: &mut Ee, tape: &Rc<RefCell<Tape>>, s: &State, entry: &str) -> (Vec<u32>, Vec<Call>) {
    reset(ee, BODY);
    poke(ee, &s.ch);
    *tape.borrow_mut() = Tape { calls: Vec::new(), answers: Some(s.clone()), rotates: 0 };
    ee.call(addr(entry).unwrap(), &[CHASSIS as u64], &[s.dt.0], 200_000);
    only_touched_the_regions(ee, BODY).unwrap();
    (snapshot(ee, BODY), std::mem::take(&mut tape.borrow_mut().calls))
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
    fn three(&mut self, lo: f32, hi: f32) -> [F; 3] {
        [self.f(lo, hi), self.f(lo, hi), self.f(lo, hi)]
    }
    /// An orientation: a unit quaternion most of the time, occasionally one that is not unit at all
    /// and occasionally exactly nothing, so that what the reciprocal square root does with a zero
    /// length is proved rather than avoided.
    fn quat(&mut self) -> Vf {
        if self.one_in(61) {
            return Vf::new(0.0, 0.0, 0.0, 0.0);
        }
        let v = self.vec(-1.0, 1.0);
        if self.one_in(17) {
            return v;
        }
        let n = (v.x() * v.x() + v.y() * v.y() + v.z() * v.z() + v.w() * v.w()).sqrt().max(1e-6);
        Vf::new(v.x() / n, v.y() / n, v.z() / n, v.w() / n)
    }
    /// A derivative history: the slot the integrator reads and the slot it keeps the last one in.
    fn hist<const N: usize>(&mut self, lo: f32, hi: f32) -> Deriv<N> {
        Deriv { dx: [0; N].map(|_| self.f(lo, hi)), prev: [0; N].map(|_| self.f(lo, hi)) }
    }
    /// One case.
    fn state(&mut self) -> State {
        let ch = Chassis {
            speed: self.f(0.0, 40.0),
            cofg_world: Vf::new(
                self.f(-400.0, 400.0).0,
                self.f(-40.0, 40.0).0,
                self.f(-400.0, 400.0).0,
                1.0,
            ),
            accel: self.vec(-60.0, 60.0),
            ang_accel: self.vec(-20.0, 20.0),
            euler: self.vec(-4.0, 4.0),
            inv_inertia: Mat3::new(self.vec(-2.0, 2.0), self.vec(-2.0, 2.0), self.vec(-2.0, 2.0)),
            force: self.vec(-4000.0, 4000.0),
            torque: self.vec(-400.0, 400.0),
            impulse: self.vec(-2.0, 2.0),
            // Half the time nothing was asked for, which is the constant the last step left; of the
            // rest, a few are a direction with no length at all, so the divide by zero in the
            // correction's normalise is proved too.
            correction: if self.one_in(2) {
                CLEAR
            } else if self.one_in(8) {
                Vf::new(0.0, 0.0, 0.0, self.f(-1.0, 1.0).0)
            } else {
                self.vec(-0.4, 0.4)
            },
            cofg: self.vec(-1.0, 1.0),
            quat: self.quat(),
            awake: if self.one_in(9) { 0 } else { 1 + (self.bits() % 3) as i32 },
            contacts: (self.bits() % 5) as i32,
            inv_mass: self.f(0.002, 0.02),
            // A limit either far above anything the step can reach or far below it, so each clamp is
            // either plainly off or plainly on rather than near its edge.
            max_speed_sq: if self.one_in(4) { self.f(0.0, 1e-4) } else { self.f(1e6, 1e7) },
            max_ang_speed_sq: if self.one_in(4) { self.f(0.0, 1e-4) } else { self.f(1e4, 1e5) },
            // The per-axis window, on the scale the angular velocity comes out at, so both ends of
            // it are reached — and sometimes crossed, the minimum above the maximum.
            ang_vel_max: self.three(-2.0, 9.0),
            ang_vel_min: self.three(-9.0, 2.0),
            vel: self.vec(-30.0, 30.0),
            local_vel: self.vec(-30.0, 30.0),
            world_ang_vel: self.vec(-9.0, 9.0),
            ang_vel: self.vec(-9.0, 9.0),
            free: [0; 3].map(|_| if self.one_in(4) { 0 } else { 1 }),
            ang_free: [0; 3].map(|_| if self.one_in(4) { 0 } else { 1 }),
            pos_hist: self.hist(-30.0, 30.0),
            vel_hist: self.hist(-60.0, 60.0),
            ang_hist: self.hist(-20.0, 20.0),
            quat_hist: self.hist(-4.0, 4.0),
        };
        State {
            ch,
            dt: if self.one_in(5) { F(1.0 / 60.0) } else { self.f(0.004, 0.05) },
            g: Given {
                cofg_world: self.vec(-400.0, 400.0),
                matrix_quat: self.quat(),
                quat_deriv: self.vec(-4.0, 4.0),
                correction_quat: self.quat(),
                rotated_cofg: self.vec(-1.0, 1.0),
                local_vel: self.vec(-30.0, 30.0),
                world_ang_vel: self.vec(-9.0, 9.0),
            },
            euler: self.vec(-4.0, 4.0),
        }
    }
}

// --- the pieces, each on its own ------------------------------------------------------------------

/// The transform's two rotations. The unrotate is the one worth the cases: it builds the transpose
/// with five MMI instructions and the third column it builds is not the transpose's third column —
/// its `z` lane is the *first* row's `w`, because the register the pack left the pair in is the one
/// moved over as it stands. That is either what the original does or it is not, and ten thousand
/// rows settle it.
#[test]
fn the_transforms_rotate_and_unrotate() {
    let Some((mut ee, _)) = harness() else { return };
    const V: u32 = OUT + 0x40;
    let mut g = Gen(0x5019_0001);
    for k in 0..10_000u32 {
        let rows = [g.vec(-4.0, 4.0), g.vec(-4.0, 4.0), g.vec(-4.0, 4.0)];
        let v = if g.one_in(11) { Vf::new(0.0, 0.0, 0.0, 0.0) } else { g.vec(-40.0, 40.0) };
        let m = Mat3::new(rows[0], rows[1], rows[2]);
        for (name, port) in
            [("transform_rotate", chassis::rotate as fn(&Mat3, Vf) -> Vf), ("transform_unrotate", chassis::unrotate)]
        {
            // The rows where the transform keeps them, and the vector asked about.
            for (i, row) in rows.iter().enumerate() {
                put_v(&mut ee, XFORM + 0x80 + 0x10 * i as u32, *row);
            }
            put_v(&mut ee, V, v);
            reset(&mut ee, ROTATED);
            // The hooks are the harness', and these two are the functions under test: unhook them.
            let a = addr(name).unwrap();
            let hook = ee.hooks.remove(&a);
            ee.call(a, &[OUT as u64, XFORM as u64, V as u64], &[], 200_000);
            if let Some(h) = hook {
                ee.hooks.insert(a, h);
            }
            only_touched_the_regions(&ee, ROTATED).unwrap();
            let was = snapshot(&ee, ROTATED);
            reset(&mut ee, ROTATED);
            put_v(&mut ee, OUT, port(&m, v));
            same(name, k, &was, &snapshot(&ee, ROTATED), &(m, v), ROTATED);
        }
    }
}

/// The two-step integrator the chassis calls through the pointer at `+0x25c`, over every length the
/// chassis ever asks for and well past it: the plain loop below nine lanes and the eight-at-a-time
/// unrolled path above, which the chassis never reaches but which is the same function.
#[test]
fn the_two_step_integrator() {
    let Some((mut ee, _)) = harness() else { return };
    let mut g = Gen(0x5019_0002);
    for k in 0..2_000u32 {
        let n = (g.bits() % 17) as usize;
        let dt = if g.one_in(5) { F(1.0 / 60.0) } else { g.f(-0.05, 0.05) };
        let x: Vec<F> = (0..n).map(|_| g.f(-100.0, 100.0)).collect();
        let dx: Vec<F> = (0..2 * n).map(|_| g.f(-60.0, 60.0)).collect();
        reset(&mut ee, INTEGRATED);
        for (i, v) in x.iter().enumerate() {
            put_f(&mut ee, X + 4 * i as u32, *v);
        }
        for (i, v) in dx.iter().enumerate() {
            put_f(&mut ee, DX + 4 * i as u32, *v);
        }
        ee.call(addr("integrate_ab2").unwrap(), &[n as u64, X as u64, DX as u64], &[dt.0], 200_000);
        only_touched_the_regions(&ee, INTEGRATED).unwrap();
        let was = snapshot(&ee, INTEGRATED);

        let (mut px, mut pdx) = (x.clone(), dx.clone());
        let (d, p) = pdx.split_at_mut(n);
        chassis::advance(&mut px, d, p, dt);
        reset(&mut ee, INTEGRATED);
        for (i, v) in px.iter().enumerate() {
            put_f(&mut ee, X + 4 * i as u32, *v);
        }
        for (i, v) in pdx.iter().enumerate() {
            put_f(&mut ee, DX + 4 * i as u32, *v);
        }
        same("integrate_ab2", k, &was, &snapshot(&ee, INTEGRATED), &(n, dt, x, dx), INTEGRATED);
    }
}

/// The three words the ground step pins an axis with. Nothing else of the body may move.
#[test]
fn the_freedom_flags() {
    let Some((mut ee, _)) = harness() else { return };
    let mut g = Gen(0x5019_0003);
    for k in 0..2_000u32 {
        let s = g.state();
        let flags = [0, 1, 2].map(|_| if g.one_in(3) { 0 } else { g.bits() as i32 });
        round_trips(&mut ee, &s.ch);
        reset(&mut ee, BODY);
        poke(&mut ee, &s.ch);
        ee.call(
            addr("chassis_set_flags").unwrap(),
            &[CHASSIS as u64, flags[0] as u64, flags[1] as u64, flags[2] as u64],
            &[],
            200_000,
        );
        only_touched_the_regions(&ee, BODY).unwrap();
        let was = snapshot(&ee, BODY);

        let mut ch = s.ch;
        chassis::set_flags(&mut ch, flags[0], flags[1], flags[2]);
        reset(&mut ee, BODY);
        poke(&mut ee, &ch);
        same("chassis_set_flags", k, &was, &snapshot(&ee, BODY), &(s.ch.ang_free, flags), BODY);
    }
}

/// The five entries of the chassis' own vtable this module was written from. The two whole-body
/// proofs dispatch through the real table rather than through stand-ins, so if the table in the
/// image is not what it was read as, that is worth knowing here rather than as a confusing failure
/// three tests down.
#[test]
fn the_vtable_is_what_this_module_ports() {
    let Some((ee, _)) = harness() else { return };
    let vt = addr("chassis_vtable").unwrap();
    for (off, name) in [
        (0x38, "chassis_measure"),
        (0x3c, "chassis_integrate"),
        (0xa4, "chassis_clear_contacts"),
        (0xb4, "chassis_local_velocity"),
        (0xc0, "chassis_world_ang_vel"),
    ] {
        assert_eq!(ee.u32_at(vt + off), addr(name).unwrap(), "the vtable's +{off:#x} is not {name}");
    }
    // One word, and the port inlines it: the contact count, cleared.
    assert_eq!(
        (ee.u32_at(addr("chassis_clear_contacts").unwrap()), ee.u32_at(addr("chassis_clear_contacts").unwrap() + 4)),
        (0x03e0_0008, 0xac80_01e0),
        "the contact count's virtual is no longer `jr $ra; sw $zero, 0x1e0($a0)`"
    );
}

// --- the whole body -------------------------------------------------------------------------------

/// The per-frame measure: the orientation written out as three angles and the speed as a scalar, the
/// only two numbers of the body anything outside the physics reads. Every word of the body is
/// compared, so the quadword the angles arrive in — the fourth lane included — is proved as well as
/// the three the name promises.
#[test]
fn the_measure() {
    let Some((mut ee, tape)) = harness() else { return };
    let mut g = Gen(0x5019_0005);
    for k in 0..10_000u32 {
        let s = g.state();
        round_trips(&mut ee, &s.ch);
        let (was, calls) = original(&mut ee, &tape, &s, "chassis_measure");

        let mut ch = s.ch;
        let port = chassis::measure(&mut ch, s.dt, s.euler);
        assert_eq!(calls, port, "case {k}: the measure called something else\n{s:#?}");
        reset(&mut ee, BODY);
        poke(&mut ee, &ch);
        same("chassis_measure", k, &was, &snapshot(&ee, BODY), &s, BODY);
    }
}

/// The step itself: eight hundred bytes of body, through every branch of it, compared word for word
/// against the original and with the ordered list of what it called compared too. The counters at
/// the end are what says the cases reached the branches rather than ten thousand runs down the same
/// side of each one.
#[test]
#[allow(clippy::float_arithmetic)] // the coverage counters' own arithmetic, not the step's
fn the_integrate() {
    let Some((mut ee, tape)) = harness() else { return };
    let mut g = Gen(0x5019_0006);
    const BRANCHES: [&str; 12] = [
        "the body asleep",
        "the orientation difference negated",
        "the orientation difference kept",
        "a linear axis pinned",
        "an angular axis pinned",
        "the speed limit reached",
        "the angular speed limit reached",
        "an axis held at its maximum",
        "an axis held at its minimum",
        "a correction asked for",
        "no correction asked for",
        "a correction with no length at all",
    ];
    let mut seen = [0u32; 12];
    for k in 0..10_000u32 {
        let s = g.state();
        round_trips(&mut ee, &s.ch);
        let (was, calls) = original(&mut ee, &tape, &s, "chassis_integrate");

        let mut ch = s.ch;
        let port = chassis::integrate(&mut ch, s.dt, &s.g);
        assert_eq!(calls, port, "case {k}: the step called something else\n{s:#?}");
        reset(&mut ee, BODY);
        poke(&mut ee, &ch);
        same("chassis_integrate", k, &was, &snapshot(&ee, BODY), &s, BODY);

        // Which way each branch went, in host arithmetic over the inputs. These decide nothing — the
        // comparison above is the proof — they only count what the cases reached.
        let c = &s.ch;
        let len2 = |v: [f32; 3]| v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
        if c.awake == 0 {
            seen[0] += 1;
        } else {
            let dot = (0..4).map(|i| s.g.matrix_quat.0[i] * c.quat.0[i]).sum::<f32>();
            seen[if dot < 0.0 { 1 } else { 2 }] += 1;
            let h = 0.5 * s.dt.0;
            // The velocity as the step will have it when the limit is tested.
            let mut a = [0, 1, 2].map(|i| c.force.0[i] * c.inv_mass.0);
            let mut v = [0, 1, 2].map(|i| c.vel.0[i]);
            let mut pv = [0, 1, 2].map(|i| c.vel_hist.prev[i].0);
            for i in 0..3 {
                if c.free[i] == 0 {
                    seen[3] += 1;
                    (v[i], a[i], pv[i]) = (0.0, 0.0, 0.0);
                }
            }
            if len2([0, 1, 2].map(|i| v[i] + h * (3.0 * a[i] - pv[i]))) > c.max_speed_sq.0 {
                seen[5] += 1;
            }
            // And the angular velocity, through the inverse inertia tensor the same way.
            let mut aa = [0, 1, 2]
                .map(|i| (0..3).map(|j| c.inv_inertia.rows[j].0[i] * c.torque.0[j]).sum::<f32>());
            let mut w = [0, 1, 2].map(|i| c.ang_vel.0[i]);
            let mut pw = [0, 1, 2].map(|i| c.ang_hist.prev[i].0);
            for i in 0..3 {
                if c.ang_free[i] == 0 {
                    seen[4] += 1;
                    (w[i], aa[i], pw[i]) = (0.0, 0.0, 0.0);
                }
            }
            if len2([0, 1, 2].map(|i| w[i] + h * (3.0 * aa[i] - pw[i]))) > c.max_ang_speed_sq.0 {
                seen[6] += 1;
            }
            for i in 0..3 {
                seen[7] += u32::from(ch.ang_vel.0[i] == c.ang_vel_max[i].0);
                seen[8] += u32::from(ch.ang_vel.0[i] == c.ang_vel_min[i].0);
            }
            if c.correction.to_bits() == CLEAR.to_bits() {
                seen[10] += 1;
            } else {
                seen[9] += 1;
                seen[11] += u32::from(len2([0, 1, 2].map(|i| c.correction.0[i])) == 0.0);
            }
        }
    }
    for (n, what) in seen.iter().zip(BRANCHES) {
        assert!(*n > 0, "no case reached {what}");
    }
}

// --- the contact distributor ----------------------------------------------------------------------

/// The bike, the two wheels, and the chain of objects the distributor walks to find the frame's
/// contacts. All of it is laid out where the harness wants it, which is what makes an owner, a
/// vehicle or a listener element identifiable by its address alone.
const BIKE: u32 = 0x0140_0000;
const SEARCH: u32 = 0x0141_0000;
/// What the sweep is handed as its second argument, out of `bike+0x7bc`.
const ACTORS: u32 = 0x0141_8000;
/// `*(search+0x38)`, whose `+0xa8` is the virtual that empties the record.
const SVT: u32 = 0x0142_0000;
const RECORD: u32 = 0x0143_0000;
const SELECTS: u32 = 0x0144_0000;
const NORMALS: u32 = 0x0145_0000;
const POINTS: u32 = 0x0146_0000;
const NODES: u32 = 0x0147_0000;
/// The nodes, the bodies that own them, the listener lists those bodies hold, the elements of those
/// lists and the vehicles — one of each per contact, at a stride that makes the index readable back
/// out of any address.
const NODE: u32 = 0x0148_0000;
const OWNER: u32 = 0x0149_0000;
const HOLDER: u32 = 0x014a_0000;
const ELEMENT: u32 = 0x014b_0000;
const EVT: u32 = 0x014c_0000;
const VEHICLE: u32 = 0x014d_0000;
const FRONT: u32 = 0x014e_0000;
const REAR: u32 = 0x014f_0000;
const STRIDE: u32 = 0x1000;
/// Two addresses that are not the game's: the record's clear and a listener's notification are both
/// reached through a function pointer in data, so the harness can put its own address there and hook
/// that. Nothing is read from either one.
const CLEAR_RECORD: u32 = 0x0160_0000;
const NOTIFY: u32 = 0x0160_0100;
/// The bike's own `+0x7d4` is the only word of it the distributor writes, and the two wheels take
/// two quadwords and three words each. All three objects are compared whole.
const RIDE: Regions = &[("bike", BIKE, 0x7e0), ("front", FRONT, 0x200), ("rear", REAR, 0x200)];

/// One case of the distributor.
#[derive(Clone, Debug)]
struct Ride {
    look: bool,
    /// The whole byte at `search+0x116`: only bit 5 may matter.
    flags: u8,
    hits: Vec<Contact>,
    front: WheelContact,
    rear: WheelContact,
    rear_node: u32,
    dt: F,
}

fn put_wheel(ee: &mut Ee, base: u32, w: &WheelContact) {
    put_v(ee, base + 0x120, w.point);
    put_v(ee, base + 0x130, w.normal);
    ee.set_u32(base + 0x1f0, w.owner);
    ee.set_u32(base + 0x1f4, w.vehicle);
    ee.set_u32(base + 0xd4, w.touching as u32);
}

/// The bike and everything under it, as the distributor will read it.
fn put_ride(ee: &mut Ee, r: &Ride) {
    reset(ee, RIDE);
    ee.set_u32(BIKE + 0x7b0, FRONT);
    ee.set_u32(BIKE + 0x7b4, REAR);
    ee.set_u32(BIKE + 0x7bc, ACTORS);
    ee.set_u32(BIKE + 0x7c8, SEARCH);
    ee.set_u32(BIKE + 0x7d4, r.rear_node);
    put_wheel(ee, FRONT, &r.front);
    put_wheel(ee, REAR, &r.rear);
    ee.set_u32(SEARCH + 0x38, SVT);
    ee.set_u32(SVT + 0xa8, CLEAR_RECORD);
    ee.set_u32(SEARCH + 0x6c, RECORD);
    ee.write(SEARCH + 0x116, 1, u128::from(if r.look { r.flags | 0x20 } else { r.flags & !0x20 }));
    ee.set_u32(RECORD + 0x8c, r.hits.len() as u32);
    ee.set_u32(RECORD + 0x90, SELECTS);
    ee.set_u32(RECORD + 0x94, NORMALS);
    ee.set_u32(RECORD + 0x98, POINTS);
    ee.set_u32(RECORD + 0x9c, NODES);
    for (i, c) in r.hits.iter().enumerate() {
        let i = i as u32;
        put_f(ee, SELECTS + 0x20 * i + 8, c.select);
        put_v(ee, NORMALS + 0x10 * i, c.normal);
        put_v(ee, POINTS + 0x10 * i, c.point);
        ee.set_u32(NODES + 4 * i, c.node);
        ee.set_u32(c.node + 0x80, c.owner);
        ee.set_u32(c.node + 0x7c, c.vehicle);
        ee.set_u32(VEHICLE + STRIDE * i + 0x1b0, c.vehicle_kind as u32);
        ee.set_u32(OWNER + STRIDE * i + 0x1f8, HOLDER + STRIDE * i);
        ee.set_u32(HOLDER + STRIDE * i + 0xf4, c.listeners as u32);
        for j in 0..c.listeners as u32 {
            ee.set_u32(ELEMENT + STRIDE * i + 0x10 * j, EVT);
        }
    }
    ee.set_u32(EVT + 0xc, NOTIFY);
}

/// An interpreter with the three things under the distributor hooked: the record's clear, the sweep
/// that fills it, and the list getter whose answer the notification is dispatched through.
fn ride_harness() -> Option<(Ee, Rc<RefCell<Vec<ContactCall>>>)> {
    let mut ee = game_or_skip()?;
    let calls: Rc<RefCell<Vec<ContactCall>>> = Rc::new(RefCell::new(Vec::new()));
    let t = calls.clone();
    ee.hooks.insert(
        CLEAR_RECORD,
        Box::new(move |_| {
            t.borrow_mut().push(ContactCall::ClearRecord);
        }),
    );
    let t = calls.clone();
    ee.hooks.insert(
        addr("collide_against_search").unwrap(),
        Box::new(move |ee| {
            assert_eq!(
                (ee.r(4) as u32, ee.r(5) as u32),
                (SEARCH, ACTORS),
                "the sweep was handed something other than the search and the actor list"
            );
            t.borrow_mut().push(ContactCall::Collide);
        }),
    );
    // The list getter: index `j` of the owner's listener list. Which owner is readable out of the
    // list's own address, so the element handed back names both the owner and the index — and the
    // notification below is recorded from the element, which is what proves the dispatch.
    ee.hooks.insert(
        addr("list_element").unwrap(),
        Box::new(move |ee| {
            let (list, j) = (ee.r(4) as u32, ee.r(5) as u32);
            assert_eq!((list - 0xf4) % STRIDE, 0, "the listener list is not one of the harness'");
            let i = (list - 0xf4 - HOLDER) / STRIDE;
            ee.set(2, u64::from(ELEMENT + STRIDE * i + 0x10 * j));
        }),
    );
    let t = calls.clone();
    ee.hooks.insert(
        NOTIFY,
        Box::new(move |ee| {
            let e = ee.r(4) as u32;
            assert_eq!((ee.r(5), ee.r(6)), (0, 0), "a listener was notified with other arguments");
            let (i, j) = ((e - ELEMENT) / STRIDE, ((e - ELEMENT) % STRIDE) / 0x10);
            t.borrow_mut().push(ContactCall::Notify(OWNER + STRIDE * i, j as i32));
        }),
    );
    Some((ee, calls))
}

#[allow(clippy::float_arithmetic)] // the generator's own arithmetic
impl Gen {
    fn wheel(&mut self) -> WheelContact {
        WheelContact {
            point: self.vec(-200.0, 200.0),
            normal: self.vec(-1.0, 1.0),
            owner: if self.one_in(2) { 0 } else { 0xdead_0000 + self.bits() % 16 },
            vehicle: if self.one_in(2) { 0 } else { 0xbeef_0000 + self.bits() % 16 },
            touching: i32::from(!self.one_in(2)),
        }
    }
    fn ride(&mut self) -> Ride {
        let n = if self.one_in(7) { 0 } else { 1 + self.bits() % 5 };
        let hits = (0..n)
            .map(|i| {
                let owner = if self.one_in(3) { 0 } else { OWNER + STRIDE * i };
                Contact {
                    // Above zero is the front wheel's and anything else the rear's, so exactly zero
                    // is worth generating on its own: it goes to the rear.
                    select: if self.one_in(8) {
                        F(0.0)
                    } else if self.one_in(2) {
                        self.f(0.001, 4.0)
                    } else {
                        self.f(-4.0, -0.001)
                    },
                    normal: self.vec(-1.0, 1.0),
                    point: self.vec(-200.0, 200.0),
                    node: NODE + STRIDE * i,
                    owner,
                    vehicle: if self.one_in(3) { 0 } else { VEHICLE + STRIDE * i },
                    // Only a 2 counts as a vehicle; the wheel is told there is none otherwise.
                    vehicle_kind: if self.one_in(2) { 2 } else { (self.bits() % 5) as i32 },
                    listeners: if owner == 0 { (self.bits() % 3) as i32 } else { (self.bits() % 4) as i32 },
                }
            })
            .collect();
        Ride {
            look: !self.one_in(4),
            flags: self.bits() as u8,
            hits,
            front: self.wheel(),
            rear: self.wheel(),
            rear_node: 0x1234_0000 + self.bits() % 16,
            dt: self.f(0.004, 0.05),
        }
    }
}

/// The distributor: the frame's contacts onto the two wheels, the front by the sign of a distance
/// along the bike and the rear everything else, with the last rear node kept on the bike and every
/// owner's listeners told before either wheel is written.
#[test]
fn the_contact_distributor() {
    let Some((mut ee, calls)) = ride_harness() else { return };
    let mut g = Gen(0x5019_0007);
    const BRANCHES: [&str; 9] = [
        "the flag clear, so the record never walked",
        "no contacts at all",
        "a contact on the front wheel",
        "a contact on the rear wheel",
        "a contact exactly on the boundary",
        "a contact whose surface has no owner",
        "an owner with listeners to tell",
        "a vehicle of the kind that counts",
        "a vehicle of a kind that does not",
    ];
    let mut seen = [0u32; 9];
    for k in 0..10_000u32 {
        let r = g.ride();
        put_ride(&mut ee, &r);
        calls.borrow_mut().clear();
        ee.call(addr("bike_wheel_contacts").unwrap(), &[BIKE as u64], &[r.dt.0], 200_000);
        only_touched_the_regions(&ee, RIDE).unwrap();
        let was = snapshot(&ee, RIDE);
        let original = std::mem::take(&mut *calls.borrow_mut());

        let mut port = r.clone();
        let cs = r.hits.clone();
        let made = chassis::wheel_contacts(
            &mut port.front,
            &mut port.rear,
            &mut port.rear_node,
            r.look,
            &cs,
            r.dt,
        );
        assert_eq!(original, made, "case {k}: the distributor called something else\n{r:#?}");
        put_ride(&mut ee, &port);
        same("bike_wheel_contacts", k, &was, &snapshot(&ee, RIDE), &r, RIDE);

        seen[0] += u32::from(!r.look);
        seen[1] += u32::from(r.hits.is_empty());
        if r.look {
            for c in &r.hits {
                seen[2] += u32::from(c.select.0 > 0.0);
                seen[3] += u32::from(c.select.0 <= 0.0);
                seen[4] += u32::from(c.select.0 == 0.0);
                seen[5] += u32::from(c.owner == 0);
                seen[6] += u32::from(c.owner != 0 && c.listeners > 0);
                seen[7] += u32::from(c.vehicle != 0 && c.vehicle_kind == 2);
                seen[8] += u32::from(c.vehicle != 0 && c.vehicle_kind != 2);
            }
        }
    }
    for (n, what) in seen.iter().zip(BRANCHES) {
        assert!(*n > 0, "no case reached {what}");
    }
}
