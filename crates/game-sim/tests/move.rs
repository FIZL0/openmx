//! S3 — movement on the ground, proved against the original.
//!
//! Six proofs. Four for the pieces of the ground step that are functions of their own in the original
//! — the clutch's launch ramp, the gearbox, the rev limiter and the per-substep wheel clear — and then
//! two for the step itself: one over generated states, and one replay of a scripted 240-tick ride.
//!
//! They all work the way `tests/air.rs` does, because it is the same object: the harness lays the
//! bike, its chassis, its two wheels, its engine and its collision record out in the interpreter's
//! RAM, writes a generated state into it, runs the original, and then writes the *same* state again,
//! applies the port, writes the port's answer over the top and compares every word of the whole
//! graph. Comparing the region rather than a list of fields is the point: a field the original writes
//! and the port does not model shows up as a difference.
//!
//! # The twenty-one calls
//!
//! The ground step is mostly a driver: it calls the suspension, the tires, the engine, the roll and
//! pitch control and a dozen more, and S3 ports none of those (the journal says why for each). So
//! every one of them is hooked, which does two things. It makes the step's own arithmetic comparable
//! — with the callees doing nothing, what is left in the object graph is what the step itself wrote —
//! and it turns the oracle into a tracer: each hook records the address it was reached at and the
//! `dt` it was handed, and the result is compared with the ordered list [`mv::step`] returns. That is
//! how the update order gets proved even where the callee does not.

use game_sim::r#move::{self as mv, Bike, Call, Chassis, Engine, Fresh, NOTHING, Params, Ride, Side, Step, Wheel};
use ps2_float::{F, Vf};
use ps2_oracle::{Ee, addr, game_or_skip};
use std::cell::RefCell;
use std::rc::Rc;

/// The object graph, laid out in RAM past anything the executable uses. The lengths are each object's
/// highest offset this step touches, rounded up — the whole of each is compared.
const OBJ: u32 = 0x0120_0000;
const CHASSIS: u32 = 0x0121_0000;
const FRONT: u32 = 0x0122_0000;
const REAR: u32 = 0x0123_0000;
const ENGINE: u32 = 0x0124_0000;
const COLL: u32 = 0x0125_0000;
const REGIONS: [(&str, u32, u32); 6] = [
    ("bike", OBJ, 0xa80),
    ("chassis", CHASSIS, 0x2c0),
    ("front wheel", FRONT, 0x8c0),
    ("rear wheel", REAR, 0x8c0),
    ("engine", ENGINE, 0xc0),
    ("collision", COLL, 0x40),
];

/// Everything the graph points at that is only ever read: the two vtables, the bike's scene node, the
/// gear ratio table, the chassis' orientation (which only the hooked transforms see) and the search
/// the last two calls are handed. None of these is compared, and the write check below would catch it
/// if the original touched one.
const CH_VTABLE: u32 = 0x0126_0000;
const COLL_VTABLE: u32 = 0x0126_1000;
const NODE: u32 = 0x0126_2000;
const RATIOS: u32 = 0x0126_3000;
const ORIENTATION: u32 = 0x0126_4000;
const SEARCH: u32 = 0x0126_5000;
const ACTORS: u32 = 0x0126_6000;
/// Where the two vtable slots point. Nothing is there: a hook catches the call, which is the only way
/// to see a virtual call happen at all.
const INTEGRATE: u32 = 0x0127_0000;
const COLLISION_BEGIN: u32 = 0x0127_1000;

/// One generated frame: the bike's tuning, the state the step owns, and the four objects it reaches.
#[derive(Clone, Debug)]
struct State {
    p: Params,
    r: Ride,
    ch: Chassis,
    front: Wheel,
    rear: Wheel,
    eng: Engine,
    st: Step,
}

fn put_f(ee: &mut Ee, a: u32, v: F) {
    ee.set_f32(a, v.0);
}
fn put_b(ee: &mut Ee, a: u32, v: bool) {
    ee.set_u32(a, v as u32);
}
fn put_v(ee: &mut Ee, a: u32, v: Vf) {
    for (k, x) in [v.x(), v.y(), v.z(), v.w()].into_iter().enumerate() {
        ee.set_f32(a + 4 * k as u32, x);
    }
}
fn get_v(ee: &Ee, a: u32) -> Vf {
    Vf::new(ee.f32_at(a), ee.f32_at(a + 4), ee.f32_at(a + 8), ee.f32_at(a + 12))
}

/// Zero every byte of every region, so a byte the original writes and the port does not cannot hide
/// behind something left over from the state before.
fn reset(ee: &mut Ee) {
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            ee.set_u32(base + o, 0);
        }
    }
}

/// The pointers between the pieces, and everything outside the compared regions. Once per machine.
fn wire(ee: &mut Ee) {
    ee.set_u32(OBJ + 0x18, NODE);
    ee.set_u32(OBJ + 0x7a4, CHASSIS);
    ee.set_u32(OBJ + 0x7b0, FRONT);
    ee.set_u32(OBJ + 0x7b4, REAR);
    ee.set_u32(OBJ + 0x7b8, ENGINE);
    ee.set_u32(OBJ + 0x7bc, ACTORS);
    ee.set_u32(OBJ + 0x7c4, SEARCH);
    ee.set_u32(OBJ + 0x7d0, COLL);
    ee.set_u32(CHASSIS + 0x8, CH_VTABLE);
    ee.set_u32(CH_VTABLE + 0x3c, INTEGRATE);
    ee.set_u32(COLL + 0x8, COLL_VTABLE);
    ee.set_u32(COLL_VTABLE + 0x80, COLLISION_BEGIN);
    ee.set_u32(ENGINE + 0x7c, RATIOS);
    ee.set_u32(ENGINE + 0x9c, REAR);
}

/// Write a state into the object graph: every field the step reads or writes, at the offset the
/// original keeps it at. The pointers [`wire`] laid down are rewritten here too, because [`reset`]
/// clears the regions they live in.
fn poke(ee: &mut Ee, s: &State) {
    wire(ee);
    let (p, r, ch, eng) = (&s.p, &s.r, &s.ch, &s.eng);

    put_v(ee, OBJ + 0x460, p.cofg);
    put_v(ee, OBJ + 0x480, p.balance_cofg);
    put_f(ee, OBJ + 0x698, p.cofg_travel_forward);
    put_f(ee, OBJ + 0x69c, p.cofg_travel_back);
    put_f(ee, OBJ + 0x6a4, p.brake_cofg_travel_forward);
    put_f(ee, OBJ + 0x6a0, p.clutch_cofg_travel_back);
    put_f(ee, OBJ + 0x678, p.steer_speed_1);
    put_f(ee, OBJ + 0x67c, p.steer_scale_1);
    put_f(ee, OBJ + 0x680, p.steer_speed_2);
    put_f(ee, OBJ + 0x684, p.steer_scale_2);
    put_f(ee, OBJ + 0x534, p.max_donut_speed);
    put_f(ee, OBJ + 0x538, p.donut_angle);
    put_f(ee, OBJ + 0x53c, p.donut_angle_back);
    put_f(ee, OBJ + 0x6f4, p.front_spring_when_stopped);
    put_f(ee, OBJ + 0x6f8, p.front_spring);
    put_f(ee, OBJ + 0x6fc, p.rear_spring_when_stopped);
    put_f(ee, OBJ + 0x700, p.rear_spring);
    put_f(ee, OBJ + 0x6c8, p.clutch_torque_scale_time);
    put_f(ee, OBJ + 0x6d0, p.max_clutch_torque_scale_speed);
    put_f(ee, OBJ + 0x6d4, p.max_clutch_torque_scale);
    put_f(ee, OBJ + 0x740, p.up_shift_rpm);
    put_f(ee, OBJ + 0x744, p.down_shift_rpm);
    put_f(ee, OBJ + 0x544, p.reverse_speed);
    put_f(ee, OBJ + 0x548, p.reverse_gain);
    put_f(ee, OBJ + 0x6a8, p.drag);
    put_v(ee, OBJ + 0x450, p.gravity);

    put_v(ee, OBJ + 0x550, r.g_peak);
    put_f(ee, OBJ + 0x6c0, r.clutch_timer);
    put_f(ee, OBJ + 0x6c4, r.clutch_scale_start);
    put_f(ee, OBJ + 0x6cc, r.clutch_cofg_weight);
    put_f(ee, OBJ + 0x65c, r.clutch_in);
    put_b(ee, COLL + 0x30, r.collision_near);
    for (a, v) in [0xa6cu32, 0xa64, 0xa4c, 0xa60].into_iter().zip(r.no_hander_scratch) {
        put_f(ee, OBJ + a, v);
    }

    put_f(ee, CHASSIS + 0x34, ch.speed);
    put_v(ee, CHASSIS + 0x130, ch.force);
    put_v(ee, CHASSIS + 0x170, ch.accel);
    put_v(ee, CHASSIS + 0x180, ch.ang_accel);
    put_v(ee, CHASSIS + 0x190, ch.cofg);
    ee.set_u32(CHASSIS + 0x254, if ch.has_transform { ORIENTATION } else { 0 });
    put_v(ee, CHASSIS + 0x260, ch.vel);
    ee.set_u32(CHASSIS + 0x2ac, ch.flags.0 as u32);
    ee.set_u32(CHASSIS + 0x2b0, ch.flags.1 as u32);
    ee.set_u32(CHASSIS + 0x2b4, ch.flags.2 as u32);

    for (base, w) in [(FRONT, &s.front), (REAR, &s.rear)] {
        put_b(ee, base + 0x3c, w.balancing);
        put_f(ee, base + 0x44, w.donut_scale);
        put_f(ee, base + 0x54, w.spring);
        put_f(ee, base + 0x7c, w.max_steer_angle);
        put_f(ee, base + 0x8ac, w.steer_command);
        put_f(ee, base + 0x838, w.inv_radius);
        put_f(ee, base + 0x1d4, w.spin);
        put_b(ee, base + 0xdc, w.contact);
        put_b(ee, base + 0xc4, w.other_tire_down);
        for (a, v) in [0xd0u32, 0xd4, 0x1f0, 0x1f4, 0xf0, 0xf8, 0x1a0].into_iter().zip(w.fresh.scalars) {
            put_f(ee, base + a, v);
        }
        for (a, v) in [0x170u32, 0x1b0, 0x1c0].into_iter().zip(w.fresh.vectors) {
            put_v(ee, base + a, v);
        }
    }

    ee.set_u32(ENGINE + 0x68, eng.gear as u32);
    ee.set_u32(ENGINE + 0x78, eng.gears as u32);
    put_f(ee, ENGINE + 0x40, eng.final_drive);
    for (k, v) in eng.gear_ratios.into_iter().enumerate() {
        put_f(ee, RATIOS + 4 * k as u32, v);
    }
    put_f(ee, ENGINE + 0x80, eng.ratio);
    put_f(ee, ENGINE + 0x84, eng.inv_ratio);
    put_f(ee, ENGINE + 0x88, eng.clutch_torque_scale);
    put_b(ee, ENGINE + 0x64, eng.clutch_locked);
    put_f(ee, ENGINE + 0xb4, eng.rev_limit);

    poke_step(ee, &s.st);
}

/// The frame's inputs on their own, which is what a replay tick rewrites.
fn poke_step(ee: &mut Ee, st: &Step) {
    put_b(ee, OBJ + 0x574, st.on_ground);
    put_b(ee, OBJ + 0x578, st.both_wheels);
    put_f(ee, OBJ + 0x438, st.speed);
    put_f(ee, OBJ + 0x62c, st.stick_y);
    put_f(ee, OBJ + 0x634, st.stick_angle);
    put_f(ee, OBJ + 0x638, st.deflection);
    put_f(ee, OBJ + 0x670, st.lean);
    put_f(ee, OBJ + 0x63c, st.gas);
    put_f(ee, OBJ + 0x640, st.brake);
    put_f(ee, OBJ + 0x644, st.clutch);
    put_f(ee, OBJ + 0x658, st.gas_rate_limited);
    put_f(ee, OBJ + 0x648, st.reverse);
    put_b(ee, OBJ + 0xa48, st.balancing);
    put_f(ee, NODE + 0x94, st.node_distance);
}

/// Every word of every region, in order.
fn snap(ee: &Ee) -> Vec<u32> {
    let mut out = Vec::new();
    for (_, base, len) in REGIONS {
        for o in (0..len).step_by(4) {
            out.push(ee.u32_at(base + o));
        }
    }
    out
}

/// Which field a differing word is, by name of its region and offset in it.
fn where_is(i: usize) -> String {
    let mut i = i as u32 * 4;
    for (name, _, len) in REGIONS {
        if i < len {
            return format!("{name} + {i:#x}");
        }
        i -= len;
    }
    "past the end".into()
}

/// Nothing the original writes may land outside the object graph: a write to a global would be state
/// the port cannot have, and this is the test that would say so. The stack frame is allowed, and so
/// are the two out-parameters the hooked transforms fill in, which live in it.
fn only_touched_the_regions(ee: &Ee) -> Result<(), String> {
    let frame = (ee.stack - 0x2000, ee.stack + 0x40);
    for (a, b) in ee.written_ranges() {
        let inside = REGIONS.iter().any(|&(_, base, len)| a >= base && b <= base + len)
            || (a >= frame.0 && b <= frame.1);
        if !inside {
            return Err(format!("the original wrote {a:#010x}..{b:#010x}, which is not the bike or the stack"));
        }
    }
    Ok(())
}

/// Compare two snapshots and say which field differs, in the original's terms.
fn same(name: &str, k: u32, was: &[u32], now: &[u32], s: &State) {
    if was == now {
        return;
    }
    let i = was.iter().zip(now).position(|(a, b)| a != b).unwrap();
    let (a, b) = (was[i], now[i]);
    panic!(
        "{name}, state {k}: {} is {a:#010x} ({}) in the original and {b:#010x} ({}) in the port\n\
         and {} word(s) differ in all\n{s:#?}",
        where_is(i),
        f32::from_bits(a),
        f32::from_bits(b),
        was.iter().zip(now).filter(|(a, b)| a != b).count(),
    );
}

// --- the tracer -----------------------------------------------------------------------------------

/// What the hooks record, and what the two vectors the hooked transforms hand back are. One of these
/// per machine, shared with every hook.
#[derive(Default)]
struct Tape {
    calls: Vec<Call>,
    /// What `transform_unrotate` writes into its out-parameter, and what `transform_rotate` does.
    /// Both belong to the chassis' own transform, so the harness chooses them and the port takes the
    /// same two vectors as given.
    local_accel: Vf,
    reverse_force: Vf,
}

/// A machine with the object graph wired up and every callee of the ground step hooked.
///
/// The hooks are the proof's instrument as much as its scaffolding: each one records the call and
/// (for the two transforms) hands back the vector the harness chose, so that what is left in the
/// object graph afterwards is exactly what the step itself wrote.
fn harness() -> Option<(Ee, Rc<RefCell<Tape>>)> {
    let mut ee = game_or_skip()?;
    wire(&mut ee);
    let tape = Rc::new(RefCell::new(Tape::default()));

    // The plain `(dt, bike)` and `(dt, wheel)` callees: record the call and do nothing.
    let plain: [(&str, fn(&Ee) -> Call); 9] = [
        ("bike_stand_upright", |ee| Call::StandUpright(F(ee.f(12)))),
        ("bike_balance", |ee| Call::Balance(F(ee.f(12)))),
        ("bike_roll_pitch", |ee| Call::RollPitch(F(ee.f(12)))),
        ("bike_preload", |ee| Call::Preload(F(ee.f(12)))),
        ("bike_backflip", |ee| Call::Backflip(F(ee.f(12)))),
        ("engine_update", |ee| Call::EngineUpdate(F(ee.f(12)))),
        ("bike_airborne_control", |ee| Call::AirborneControl(F(ee.f(12)))),
        ("bike_wheel_contacts", |ee| Call::Contacts(F(ee.f(12)))),
        ("bike_no_hander", |ee| Call::NoHander(F(ee.f(12)))),
    ];
    for (name, make) in plain {
        let t = tape.clone();
        ee.hooks.insert(addr(name).unwrap(), Box::new(move |ee| t.borrow_mut().calls.push(make(ee))));
    }
    // The three per-wheel callees, which say which wheel by their argument.
    let per_wheel: [(&str, fn(Side, F) -> Call); 3] = [
        ("wheel_suspension", Call::Suspension),
        ("wheel_tire_forces", Call::TireForces),
        ("wheel_integrate", Call::WheelIntegrate),
    ];
    for (name, make) in per_wheel {
        let t = tape.clone();
        ee.hooks.insert(
            addr(name).unwrap(),
            Box::new(move |ee| {
                let side = if ee.r(4) as u32 == FRONT { Side::Front } else { Side::Rear };
                t.borrow_mut().calls.push(make(side, F(ee.f(12))));
            }),
        );
    }
    // The two the chassis' transform owns. Each records the vector it was handed — which is the
    // port's own arithmetic, so it is compared — and writes back the one the harness chose.
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_rotate").unwrap(),
        Box::new(move |ee| {
            let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
            let local = get_v(ee, v);
            let force = t.borrow().reverse_force;
            t.borrow_mut().calls.push(Call::Rotate(local));
            put_v(ee, out, force);
        }),
    );
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_unrotate").unwrap(),
        Box::new(move |ee| {
            let (out, v) = (ee.r(4) as u32, ee.r(6) as u32);
            let given = get_v(ee, v);
            let accel = t.borrow().local_accel;
            t.borrow_mut().calls.push(Call::Unrotate(given));
            put_v(ee, out, accel);
        }),
    );
    // The two virtual calls, and the two halves of the nearby-actor search.
    let t = tape.clone();
    ee.hooks.insert(COLLISION_BEGIN, Box::new(move |ee| {
        t.borrow_mut().calls.push(Call::CollisionBegin(ee.r(5) != 0));
    }));
    let t = tape.clone();
    ee.hooks.insert(INTEGRATE, Box::new(move |ee| {
        t.borrow_mut().calls.push(Call::Integrate(F(ee.f(12))));
    }));
    let t = tape.clone();
    ee.hooks.insert(addr("actor_search_reset").unwrap(), Box::new(move |_| {
        t.borrow_mut().calls.push(Call::SearchReset);
    }));
    let t = tape.clone();
    ee.hooks.insert(addr("actor_search_gather").unwrap(), Box::new(move |ee| {
        assert_eq!(ee.r(6), 3, "the search radius is not 3");
        t.borrow_mut().calls.push(Call::SearchGather);
    }));
    Some((ee, tape))
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
    fn b(&mut self) -> bool {
        self.bits() & 0x10000 != 0
    }
    fn one_in(&mut self, n: u32) -> bool {
        self.bits() % n == 0
    }
    /// Often exactly zero: a zero tuning value is a branch in four places here.
    fn maybe_zero(&mut self, lo: f32, hi: f32) -> F {
        if self.one_in(3) { F::ZERO } else { self.f(lo, hi) }
    }
    /// A control: zero, one, or somewhere between — the three cases the clutch and the throttle are
    /// tested for by equality.
    fn control(&mut self) -> F {
        match self.bits() % 4 {
            0 => F::ZERO,
            1 => F::ONE,
            _ => self.f(0.0, 1.0),
        }
    }
    fn vec(&mut self, lo: f32, hi: f32) -> Vf {
        Vf::new(self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0, self.f(lo, hi).0)
    }
    /// A velocity, occasionally exactly nothing so that what the normalise idiom does with a length
    /// of zero is covered too.
    fn vel(&mut self) -> Vf {
        if self.one_in(32) {
            NOTHING
        } else {
            Vf::new(self.f(-30.0, 30.0).0, self.f(-12.0, 12.0).0, self.f(-30.0, 30.0).0, 1.0)
        }
    }
    fn wheel(&mut self) -> Wheel {
        Wheel {
            balancing: self.b(),
            donut_scale: self.f(0.0, 1.0),
            spring: self.f(0.0, 4000.0),
            max_steer_angle: self.f(0.0, 0.8),
            steer_command: self.f(-0.8, 0.8),
            inv_radius: self.f(1.5, 4.0),
            spin: self.f(-20.0, 400.0),
            contact: self.b(),
            other_tire_down: self.b(),
            fresh: Fresh {
                scalars: std::array::from_fn(|_| self.f(-1000.0, 1000.0)),
                vectors: std::array::from_fn(|_| self.vec(-1000.0, 1000.0)),
            },
        }
    }

    fn state(&mut self) -> State {
        // The donut's two angles as the loader leaves them: the file's one angle in degrees, negated
        // into radians, and -pi minus that. Sometimes exactly on a quadrant boundary, which is where
        // the ramps collapse and the original takes the other branch.
        let donut_angle = match self.bits() % 8 {
            0 => F::ZERO,
            1 => F(-1.5707964),
            _ => self.f(-1.55, -0.05),
        };
        let donut_angle_back = match self.bits() % 8 {
            0 => F(-3.1415927),
            1 => F(-1.5707964),
            _ => F(-3.1415927) - donut_angle,
        };
        // The steering fade's two speeds, equal often enough to cover the branch that skips it.
        let steer_speed_1 = self.f(0.0, 10.0);
        let steer_speed_2 = if self.one_in(6) { steer_speed_1 } else { self.f(10.0, 40.0) };
        let p = Params {
            cofg: self.vec(-1.0, 1.0),
            balance_cofg: self.vec(-1.0, 1.0),
            cofg_travel_forward: self.f(0.0, 0.5),
            cofg_travel_back: self.f(0.0, 0.5),
            brake_cofg_travel_forward: self.f(-0.2, 0.5),
            clutch_cofg_travel_back: self.f(-0.2, 0.5),
            steer_speed_1,
            steer_scale_1: self.f(0.0, 1.5),
            steer_speed_2,
            steer_scale_2: self.f(0.0, 1.5),
            max_donut_speed: self.f(0.0, 45.0),
            donut_angle,
            donut_angle_back,
            front_spring_when_stopped: self.f(0.0, 4000.0),
            front_spring: self.f(0.0, 4000.0),
            rear_spring_when_stopped: self.f(0.0, 4000.0),
            rear_spring: self.f(0.0, 4000.0),
            clutch_torque_scale_time: self.maybe_zero(0.0, 1.0),
            max_clutch_torque_scale_speed: self.maybe_zero(0.0, 20.0),
            max_clutch_torque_scale: self.f(0.5, 4.0),
            up_shift_rpm: self.f(2000.0, 9000.0),
            down_shift_rpm: self.f(1000.0, 6000.0),
            reverse_speed: self.f(-6.0, 0.0),
            reverse_gain: self.f(0.0, 600.0),
            drag: self.f(0.0, 4.0),
            gravity: self.vec(-200.0, 200.0),
        };
        let gears = 1 + (self.bits() % 6) as i32;
        let eng = Engine {
            gear: 1 + (self.bits() % 7) as i32,
            gears,
            final_drive: self.f(1.0, 4.0),
            gear_ratios: std::array::from_fn(|_| self.f(0.5, 3.5)),
            ratio: self.f(0.5, 12.0),
            inv_ratio: self.f(0.08, 2.0),
            clutch_torque_scale: self.f(1.0, 4.0),
            clutch_locked: self.b(),
            rev_limit: self.f(3000.0, 12000.0),
        };
        let r = Ride {
            g_peak: self.vec(-40.0, 40.0),
            clutch_timer: self.maybe_zero(0.0, 1.2),
            clutch_scale_start: self.f(0.5, 4.0),
            clutch_cofg_weight: self.f(0.0, 1.0),
            clutch_in: self.f(0.0, 1.0),
            collision_near: self.b(),
            no_hander_scratch: std::array::from_fn(|_| self.f(-10.0, 10.0)),
        };
        let ch = Chassis {
            speed: self.f(0.0, 40.0),
            force: self.vec(-2000.0, 2000.0),
            accel: self.vec(-40.0, 40.0),
            ang_accel: self.vec(-40.0, 40.0),
            cofg: self.vec(-1.0, 1.0),
            has_transform: !self.one_in(4),
            vel: self.vel(),
            flags: (0, 0, 0),
        };
        let st = Step {
            // Mostly a 60 fps frame, which is one substep; the rest spread over the counts up to five.
            dt: if self.one_in(3) { self.f(0.0, 0.085) } else { F(1.0 / 60.0) },
            on_ground: self.b(),
            both_wheels: !self.one_in(3),
            speed: self.f(0.0, 40.0),
            stick_y: self.f(-1.1, 1.1),
            stick_angle: self.f(-3.2, 3.2),
            // Mostly hard over, because a deflection past 0.9 is the gate on the donut and a donut
            // is two of the four branches of [`mv::donut_roll`].
            deflection: if self.one_in(3) { self.f(0.0, 1.0) } else { self.f(0.9, 1.05) },
            lean: self.f(-1.1, 1.1),
            gas: self.control(),
            brake: self.control(),
            clutch: self.control(),
            gas_rate_limited: self.control(),
            reverse: if self.b() { F::ZERO } else { F::ONE },
            balancing: self.one_in(4),
            local_accel: self.vec(-40.0, 40.0),
            reverse_force: self.vec(-600.0, 600.0),
            node_distance: self.f(0.0, 0.5),
        };
        let (front, rear) = (self.wheel(), self.wheel());
        State { p, r, ch, front, rear, eng, st }
    }
}

/// A proof is only worth the branches its states reach. Each proof tallies the path every state took,
/// and this is the assertion that none of those paths was reached only a handful of times.
fn covered(name: &str, tally: &std::collections::BTreeMap<&str, u32>, paths: &[&str], least: u32) {
    if tally.is_empty() {
        return; // no game, nothing proved
    }
    for path in paths {
        let n = tally.get(path).copied().unwrap_or(0);
        assert!(n >= least, "{name}: only {n} state(s) went down the {path:?} path; the tally is {tally:?}");
    }
}

// --- the four pieces ------------------------------------------------------------------------------

/// The clutch's launch ramp, against `bike_clutch`.
#[test]
fn the_clutch_is_the_originals() {
    let Some((mut ee, _)) = harness() else { return };
    let mut g = Gen(0x5133);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        *tally
            .entry(if s.st.clutch == F::ONE && s.st.gas == F::ONE {
                "rearmed"
            } else if s.st.clutch == F::ZERO && s.r.clutch_timer <= s.p.clutch_torque_scale_time {
                if s.r.clutch_timer == F::ZERO { "ramp starts" } else { "ramp runs" }
            } else {
                "nothing"
            })
            .or_insert(0) += 1;
        reset(&mut ee);
        poke(&mut ee, &s);
        ee.call(addr("bike_clutch").unwrap(), &[OBJ as u64], &[s.st.dt.0], 4_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("the clutch, state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);

        let mut t = s.clone();
        mv::clutch(&t.p, &mut t.r, &mut t.ch, &mut t.eng, &t.st);
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the clutch", k, &was, &snap(&ee), &s);
    }
    covered("the clutch", &tally, &["rearmed", "ramp starts", "ramp runs", "nothing"], 200);
}

/// The gearbox, against `bike_set_gear` — including the two gears out of range, which it refuses.
#[test]
fn the_gearbox_is_the_originals() {
    let Some((mut ee, _)) = harness() else { return };
    let mut g = Gen(0x9e21);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let s = g.state();
        let want = (g.bits() % 9) as i32 - 1;
        *tally
            .entry(if want <= 0 {
                "below first"
            } else if want > s.eng.gears {
                "above top"
            } else {
                "in gear"
            })
            .or_insert(0) += 1;
        reset(&mut ee);
        poke(&mut ee, &s);
        ee.call(addr("bike_set_gear").unwrap(), &[ENGINE as u64, want as u64], &[], 4_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("the gearbox, state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);

        let mut t = s.clone();
        mv::set_gear(&mut t.eng, want);
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the gearbox", k, &was, &snap(&ee), &s);
    }
    covered("the gearbox", &tally, &["below first", "above top", "in gear"], 500);
}

/// The rev limiter, against `engine_rev_limit`.
#[test]
fn the_rev_limiter_is_the_originals() {
    let Some((mut ee, _)) = harness() else { return };
    let mut g = Gen(0x1cc7);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        // Half the states are put near the limit, because an unbiased spin is almost never over it.
        if g.b() {
            s.rear.spin = s.eng.rev_limit / s.eng.ratio * F(0.1047198) * g.f(0.9, 1.1);
        }
        let over = F(9.549296) * s.rear.spin * s.eng.ratio > s.eng.rev_limit;
        *tally
            .entry(match (s.eng.clutch_locked, over) {
                (false, _) => "slipping",
                (true, true) => "over the limit",
                (true, false) => "under the limit",
            })
            .or_insert(0) += 1;
        reset(&mut ee);
        poke(&mut ee, &s);
        ee.call(addr("engine_rev_limit").unwrap(), &[ENGINE as u64], &[], 4_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("the rev limiter, state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);

        let mut t = s.clone();
        mv::engine_post(&t.eng, &mut t.rear);
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the rev limiter", k, &was, &snap(&ee), &s);
    }
    covered("the rev limiter", &tally, &["slipping", "over the limit", "under the limit"], 500);
}

/// The per-substep wheel clear, against `wheel_clear`. Both wheels, because the port writes the same
/// eleven fields on each and a transposed offset would show up on one and not the other.
#[test]
fn the_wheel_clear_is_the_originals() {
    let Some((mut ee, _)) = harness() else { return };
    let mut g = Gen(0x4a0d);
    for k in 0..2_000 {
        let s = g.state();
        reset(&mut ee);
        poke(&mut ee, &s);
        ee.call(addr("wheel_clear").unwrap(), &[FRONT as u64], &[], 4_000);
        ee.call(addr("wheel_clear").unwrap(), &[REAR as u64], &[], 4_000);
        let was = snap(&ee);

        let mut t = s.clone();
        mv::wheel_clear(&mut t.front);
        mv::wheel_clear(&mut t.rear);
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the wheel clear", k, &was, &snap(&ee), &s);
    }
}

// --- the whole step -------------------------------------------------------------------------------

/// Apply the port to a state, and give back the calls it says it made.
fn apply(t: &mut State) -> Vec<Call> {
    let (p, st) = (t.p, t.st);
    let mut b = Bike {
        ride: &mut t.r,
        chassis: &mut t.ch,
        front: &mut t.front,
        rear: &mut t.rear,
        engine: &mut t.eng,
    };
    mv::step(&p, &mut b, &st)
}

/// **The ground step**, against `bike_ground_step`, over 2000 generated frames: the whole object graph
/// word for word, and the ordered list of everything it called.
#[test]
fn the_ground_step_is_the_originals() {
    let Some((mut ee, tape)) = harness() else { return };
    let mut g = Gen(0x3c5b);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..2_000 {
        let s = g.state();
        let (n, _) = mv::substeps(s.st.dt);
        *tally.entry(if s.st.balancing { "balancing" } else { "riding" }).or_insert(0) += 1;
        *tally.entry(if n > 1 { "several substeps" } else { "one substep" }).or_insert(0) += 1;
        *tally
            .entry(if s.rear.contact && s.st.reverse != F::ZERO {
                if s.ch.has_transform { "reverse push" } else { "reverse, no transform" }
            } else {
                "no reverse"
            })
            .or_insert(0) += 1;
        *tally.entry(if mv::donut_roll(&s.p, &s.ch, &s.st) == F::ZERO { "no donut" } else { "donut" }).or_insert(0) += 1;

        reset(&mut ee);
        poke(&mut ee, &s);
        {
            let mut t = tape.borrow_mut();
            t.calls.clear();
            t.local_accel = s.st.local_accel;
            t.reverse_force = s.st.reverse_force;
        }
        ee.call(addr("bike_ground_step").unwrap(), &[OBJ as u64], &[s.st.dt.0], 400_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("the ground step, state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);
        let theirs = tape.borrow().calls.clone();

        let mut t = s.clone();
        let ours = apply(&mut t);
        if ours != theirs {
            let i = ours.iter().zip(&theirs).position(|(a, b)| a != b).unwrap_or(ours.len().min(theirs.len()));
            panic!(
                "the ground step, state {k}: call {i} of {} is {:?} in the port and call {i} of {} is \
                 {:?} in the original\n{s:#?}",
                ours.len(),
                ours.get(i),
                theirs.len(),
                theirs.get(i),
            );
        }
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the ground step", k, &was, &snap(&ee), &s);
    }
    covered(
        "the ground step",
        &tally,
        &["balancing", "riding", "one substep", "several substeps", "reverse push", "no reverse", "donut", "no donut"],
        100,
    );
}

// --- the replay fixture ---------------------------------------------------------------------------

/// A scripted ride: 240 frames of inputs, as a ride that pulls away, shifts up, brakes, leans into a
/// donut, dumps the clutch and reverses would deliver them.
///
/// The point of a fixture after the generated states is that it *carries state*: the original's bike
/// is stepped 240 times in the one piece of memory, so the clutch ramp, the gearbox, the moved centre
/// of gravity and the g-load peaks of frame 117 are whatever frames 1 to 116 made them. A per-frame
/// state that happened to be reachable is not the same test as a state that was actually reached.
fn tape_at(i: u32, speed: F) -> Step {
    #[allow(clippy::float_arithmetic)] // the fixture's own script, not the simulation's arithmetic
    let t = i as f32 / 60.0;
    let phase = i / 40;
    Step {
        dt: F(1.0 / 60.0),
        on_ground: phase != 3,
        both_wheels: phase != 3 && phase != 4,
        speed,
        stick_y: F(match phase {
            1 => -0.8,
            4 => 0.9,
            _ => 0.0,
        }),
        stick_angle: F(if phase == 4 { -2.4 } else { -0.3 }),
        deflection: F(if phase == 4 { 1.0 } else { 0.2 }),
        lean: F(match phase {
            2 => 0.75,
            4 => -0.5,
            _ => 0.05,
        }),
        gas: F(match phase {
            0 | 1 | 4 => 1.0,
            2 => 0.45,
            _ => 0.0,
        }),
        brake: F(if phase == 3 { 1.0 } else { 0.0 }),
        clutch: F(match phase {
            0 => 1.0,
            1 => 0.0,
            _ => 0.0,
        }),
        gas_rate_limited: F(if phase == 3 || phase == 5 { 0.0 } else { 0.8 }),
        reverse: F(if phase == 5 { 1.0 } else { 0.0 }),
        balancing: phase == 4,
        // Two vectors that belong to the chassis' transform: made to wander so that a frame's g-load
        // peaks are not the frame before's.
        #[allow(clippy::float_arithmetic)]
        local_accel: Vf::new(12.0 * (t * 2.1).sin(), 30.0 * (t * 1.3).cos(), 18.0 * (t * 0.7).sin(), 1.0),
        #[allow(clippy::float_arithmetic)]
        reverse_force: Vf::new(0.0, 0.0, 120.0 * (t * 0.5).cos(), 1.0),
        node_distance: F(if phase == 3 { 0.1 } else { 0.4 }),
    }
}

/// The 240-frame replay: the original stepped 240 times in one piece of memory against the port
/// stepped 240 times over its own state, compared in full after every frame.
#[test]
fn a_scripted_ride_replays_the_same() {
    let Some((mut ee, tape)) = harness() else { return };
    // One plausible bike, not a generated one: a fixture is about the sequence, not the coverage.
    let mut g = Gen(0x7777);
    let start = State {
        p: Params {
            cofg: Vf::new(0.0, 0.42, 0.03, 1.0),
            balance_cofg: Vf::new(0.0, 0.5, -0.2, 1.0),
            cofg_travel_forward: F(0.18),
            cofg_travel_back: F(0.24),
            brake_cofg_travel_forward: F(0.12),
            clutch_cofg_travel_back: F(0.3),
            steer_speed_1: F(4.0),
            steer_scale_1: F(1.0),
            steer_speed_2: F(26.0),
            steer_scale_2: F(0.35),
            max_donut_speed: F(12.0),
            donut_angle: F(-0.5235988),
            donut_angle_back: F(-3.1415927) - F(-0.5235988),
            front_spring_when_stopped: F(1600.0),
            front_spring: F(2400.0),
            rear_spring_when_stopped: F(1900.0),
            rear_spring: F(2800.0),
            clutch_torque_scale_time: F(0.4),
            max_clutch_torque_scale_speed: F(9.0),
            max_clutch_torque_scale: F(2.5),
            up_shift_rpm: F(8200.0),
            down_shift_rpm: F(4200.0),
            reverse_speed: F(-3.0),
            reverse_gain: F(220.0),
            drag: F(0.42),
            gravity: Vf::new(0.0, -7084.0, 0.0, 1.0),
        },
        r: Ride::default(),
        ch: Chassis {
            speed: F::ZERO,
            force: NOTHING,
            accel: NOTHING,
            ang_accel: NOTHING,
            cofg: Vf::new(0.0, 0.42, 0.03, 1.0),
            has_transform: true,
            vel: Vf::new(0.0, 0.0, 0.0, 1.0),
            flags: (0, 0, 0),
        },
        front: Wheel { max_steer_angle: F(0.52), inv_radius: F(2.8), contact: true, ..g.wheel() },
        rear: Wheel { max_steer_angle: F::ZERO, inv_radius: F(2.6), contact: true, ..g.wheel() },
        eng: Engine {
            gear: 1,
            gears: 5,
            final_drive: F(3.2),
            gear_ratios: [F(2.6), F(1.9), F(1.5), F(1.2), F(1.0), F::ZERO, F::ZERO, F::ZERO],
            ratio: F(3.2) * F(2.6),
            inv_ratio: F::ONE / (F(3.2) * F(2.6)),
            clutch_torque_scale: F::ONE,
            clutch_locked: true,
            rev_limit: F(11000.0),
        },
        st: tape_at(0, F::ZERO),
    };

    // The speed the fixture rides at: the chassis would compute this from the forces, and the chassis
    // is not this task's — so the fixture scripts it, the same way for both sides.
    #[allow(clippy::float_arithmetic)]
    let speed_at = |i: u32| F((i as f32 / 60.0 * 3.4).min(28.0));

    // Pass one: the original, 240 frames in the one piece of memory.
    reset(&mut ee);
    poke(&mut ee, &start);
    let mut theirs = Vec::new();
    for i in 0..240 {
        let st = tape_at(i, speed_at(i));
        poke_step(&mut ee, &st);
        {
            let mut t = tape.borrow_mut();
            t.calls.clear();
            t.local_accel = st.local_accel;
            t.reverse_force = st.reverse_force;
        }
        ee.call(addr("bike_ground_step").unwrap(), &[OBJ as u64], &[st.dt.0], 400_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("frame {i}: {e}"));
        theirs.push((snap(&ee), tape.borrow().calls.clone()));
    }

    // Pass two: the port, 240 frames over its own state, compared with each of theirs in turn.
    let mut t = start.clone();
    for (i, (was, their_calls)) in theirs.into_iter().enumerate() {
        t.st = tape_at(i as u32, speed_at(i as u32));
        let ours = apply(&mut t);
        assert_eq!(ours, their_calls, "frame {i}: the calls differ");
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the replay", i as u32, &was, &snap(&ee), &t);
    }
}
