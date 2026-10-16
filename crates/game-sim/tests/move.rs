//! S3 and S15 — movement on the ground, proved against the original.
//!
//! Seven proofs. Five for the pieces of the ground step that are functions of their own in the
//! original — the clutch's launch ramp, the gearbox, the rev limiter, the per-substep wheel clear and
//! the engine's own update — and then two for the step itself: one over generated states, and one
//! replay of a scripted 240-tick ride.
//!
//! They all work the way `tests/air.rs` does, because it is the same object: the harness lays the
//! bike, its chassis, its two wheels, its engine and its collision record out in the interpreter's
//! RAM, writes a generated state into it, runs the original, and then writes the *same* state again,
//! applies the port, writes the port's answer over the top and compares every word of the whole
//! graph. Comparing the region rather than a list of fields is the point: a field the original writes
//! and the port does not model shows up as a difference.
//!
//! # The twenty-one calls, less the engine
//!
//! The ground step is mostly a driver: it calls the suspension, the tires, the engine, the roll and
//! pitch control and a dozen more, and S3 ported none of those (the journal says why for each). So
//! every one of them is hooked, which does two things. It makes the step's own arithmetic comparable
//! — with the callees doing nothing, what is left in the object graph is what the step itself wrote —
//! and it turns the oracle into a tracer: each hook records the address it was reached at and the
//! `dt` it was handed, and the result is compared with the ordered list [`mv::step`] returns. That is
//! how the update order gets proved even where the callee does not.
//!
//! One of the twenty-one is no longer hooked: `engine_update`, which S15 ported. It runs for real on
//! both sides of the two step proofs, so the engine's revs, its torque and the driven wheel's torque
//! accumulator are part of the compared graph there, and they carry from substep to substep and from
//! frame to frame the way the game's do.

use game_sim::air::AcosTable;
use game_sim::camera::{CURVE_ENTRIES, CurveTable};
use game_sim::r#move::{
    self as mv, Bike, Call, Chassis, Engine, Frame, Fresh, NOTHING, Outside, Params, Ride, Side,
    Step, Tables, Wheel,
};
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
/// The engine's power curve: the RPM of each point and the torque at it, two heap arrays in the
/// original. `engine_set_power_curve` fills them at load time and nothing writes them afterwards.
const CURVE_RPM: u32 = 0x0126_7000;
const CURVE_TORQUE: u32 = 0x0126_8000;
/// Where the two vtable slots point. Nothing is there: a hook catches the call, which is the only way
/// to see a virtual call happen at all.
const INTEGRATE: u32 = 0x0127_0000;
const COLLISION_BEGIN: u32 = 0x0127_1000;
/// The engine's two start-up tables, which the no-hander's rotation and both roll angles read: the
/// 6001-entry `{cos, sin}` curve and the 30001-entry arc cosine. Far enough apart for both.
const CURVE: u32 = 0x0130_0000;
const ACOS: u32 = 0x0140_0000;

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
    /// The forward axis of each of the two frames the controllers ask about — the `z` row of the
    /// frame's world matrix, which is the word `orientation_axes` copies out. The harness keeps them
    /// where that function reads them, so the hook hands back the same quadword the real one would
    /// and the port's [`Outside`] can hand back the same again.
    node_forward: Vf,
    chassis_forward: Vf,
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
    // The two floats the engine reads through a pointer, which the bike's own setup aims at its
    // rate-limited throttle and clutch (`0x002535c4`), and the power curve's two arrays — the second
    // of which the cursor at `+0x8c` holds a third pointer to.
    ee.set_u32(ENGINE + 0x30, OBJ + 0x658);
    ee.set_u32(ENGINE + 0x34, OBJ + 0x65c);
    ee.set_u32(ENGINE + 0x70, CURVE_RPM);
    ee.set_u32(ENGINE + 0x74, CURVE_TORQUE);
    ee.set_u32(ENGINE + 0x98, CURVE_RPM);
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
    put_f(ee, OBJ + 0x540, p.donut_roll_angle_scale);
    put_f(ee, OBJ + 0x6d8, p.max_roll_command);
    put_f(ee, OBJ + 0x6e0, p.max_roll_speed);
    put_f(ee, OBJ + 0x6e8, p.roll_gain);
    put_f(ee, OBJ + 0x6ec, p.min_stand_upright_speed);
    put_f(ee, OBJ + 0x6f0, p.max_stand_upright_speed);
    put_f(ee, OBJ + 0x704, p.roll_control_gain);
    put_f(ee, OBJ + 0x708, p.roll_control_rate_gain);
    put_f(ee, OBJ + 0x70c, p.roll_control_max_cg);
    put_f(ee, OBJ + 0x710, p.max_roll_rate_disturbance);
    put_f(ee, OBJ + 0x714, p.max_pitch_command);
    put_f(ee, OBJ + 0x718, p.pitch_gain);
    put_f(ee, OBJ + 0x71c, p.pitch_rate_gain);
    put_f(ee, OBJ + 0x720, p.inv_wheelie_top_speed);
    put_f(ee, OBJ + 0x730, p.max_stoppie_pitch_command);
    put_f(ee, OBJ + 0x734, p.stoppie_pitch_gain);
    put_f(ee, OBJ + 0x738, p.stoppie_yaw_rate_scale);
    put_f(ee, OBJ + 0x73c, p.stoppie_top_speed);
    put_f(ee, OBJ + 0xa50, p.balance_inertia_rate);
    put_f(ee, OBJ + 0xa54, p.min_balance_inertia);
    put_f(ee, OBJ + 0xa58, p.max_balance_inertia);
    put_f(ee, OBJ + 0xa5c, p.input_inertia);
    put_f(ee, OBJ + 0xa74, p.max_balance_point);
    put_f(ee, OBJ + 0xa68, p.balance_noise_amplitude);
    put_f(ee, OBJ + 0xa70, p.balance_noise_time_scale);

    put_v(ee, OBJ + 0x550, r.g_peak);
    put_f(ee, OBJ + 0x6c0, r.clutch_timer);
    put_f(ee, OBJ + 0x6c4, r.clutch_scale_start);
    put_f(ee, OBJ + 0x6cc, r.clutch_cofg_weight);
    put_f(ee, OBJ + 0x65c, r.clutch_in);
    put_b(ee, COLL + 0x30, r.collision_near);
    put_f(ee, OBJ + 0xa4c, r.balance_inertia);
    put_f(ee, OBJ + 0xa60, r.balance_command);
    put_f(ee, OBJ + 0xa64, r.balance_point);
    put_f(ee, OBJ + 0xa6c, r.balance_noise_time);
    put_f(ee, OBJ + 0x724, r.pitch_integral);
    put_b(ee, OBJ + 0x728, r.stoppie);

    put_f(ee, CHASSIS + 0x34, ch.speed);
    put_v(ee, CHASSIS + 0x130, ch.force);
    put_v(ee, CHASSIS + 0x170, ch.accel);
    put_v(ee, CHASSIS + 0x180, ch.ang_accel);
    put_v(ee, CHASSIS + 0x190, ch.cofg);
    put_v(ee, CHASSIS + 0x90, ch.euler);
    put_v(ee, CHASSIS + 0x140, ch.torque);
    put_v(ee, CHASSIS + 0x280, ch.world_ang_vel);
    put_v(ee, CHASSIS + 0x290, ch.ang_vel);
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
        put_f(ee, base + 0x210, w.drive_torque);
        put_f(ee, base + 0x214, w.radius);
        put_b(ee, base + 0xe0, w.spin_from_torque);
        put_f(ee, base + 0x858, w.contact_force_fwd);
        put_v(ee, base + 0x130, w.contact_normal);
        put_b(ee, base + 0x38, w.steered);
        for (a, v) in [0xd0u32, 0xd4, 0x1f0, 0x1f4, 0xf0, 0xf8, 0x1a0].into_iter().zip(w.fresh.scalars) {
            ee.set_u32(base + a, v);
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
    put_f(ee, ENGINE + 0x38, eng.idle_throttle);
    put_f(ee, ENGINE + 0x3c, eng.engine_braking);
    put_f(ee, ENGINE + 0x48, eng.inv_inertia);
    put_f(ee, ENGINE + 0x50, eng.clutch_static);
    put_f(ee, ENGINE + 0x54, eng.clutch_kinetic);
    put_f(ee, ENGINE + 0x60, eng.min_rpm);
    put_f(ee, ENGINE + 0xa0, eng.torque);
    put_f(ee, ENGINE + 0xa4, eng.wheel_torque);
    put_f(ee, ENGINE + 0xa8, eng.rpm);
    put_f(ee, ENGINE + 0xac, eng.road_rpm);
    put_f(ee, ENGINE + 0xbc, eng.torque_scale);
    // The cursor, and the count the loader keeps twice — once for the curve's own arrays at `+0x6c`
    // and once inside the cursor at `+0x8c`. The port keeps one, because they cannot disagree.
    ee.set_u32(ENGINE + 0x6c, eng.curve.points);
    ee.set_u32(ENGINE + 0x8c, eng.curve.points);
    ee.set_u32(ENGINE + 0x90, eng.curve.index);
    put_f(ee, ENGINE + 0x94, eng.curve.frac());
    for k in 0..eng.curve_rpm.len() as u32 {
        put_f(ee, CURVE_RPM + 4 * k, eng.curve_rpm[k as usize]);
        put_f(ee, CURVE_TORQUE + 4 * k, eng.curve_torque[k as usize]);
    }

    // Outside the compared regions: the two frames' forward axes, where `orientation_axes` reads
    // them from. Both of the harness' sides answer from the same quadword.
    put_v(ee, NODE + 0xa0, s.node_forward);
    put_v(ee, ORIENTATION + 0xa0, s.chassis_forward);

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
    put_f(ee, OBJ + 0x628, st.stick_x);
    put_f(ee, OBJ + 0x650, st.front_brake);
    put_f(ee, OBJ + 0x654, st.rear_brake);
    put_f(ee, OBJ + 0x674, st.lean_fore_aft);
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

/// The same for the ordered list of transform calls: where the two lists part company, and what each
/// side asked for there. A controller that skipped a call, made one against the wrong frame, or
/// handed over a vector a word different fails here rather than in the object graph — which is the
/// only way to see it at all for the no-hander, whose whole answer goes out through one of them.
fn same_out(name: &str, k: u32, ours: &[Out], theirs: &[Out], s: &State) {
    if ours == theirs {
        return;
    }
    let i = ours.iter().zip(theirs).position(|(a, b)| a != b).unwrap_or(ours.len().min(theirs.len()));
    panic!(
        "{name}, state {k}: transform call {i} of {} is {:?} in the port and {i} of {} is {:?} in \
         the original\n{s:#?}",
        ours.len(),
        ours.get(i),
        theirs.len(),
        theirs.get(i),
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
    /// Every [`Outside`] call the four controllers made, in order. The port's own [`Outside`] records
    /// the same list, and the two are compared.
    out: Vec<Out>,
}

/// One call into the engine's transform code. Four of them, and the original makes all four through
/// functions this crate does not port — so each side records what it asked for, and the two lists are
/// compared the same way the [`Call`] list is.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Out {
    Unrotate(Frame, Vf),
    Forward(Frame),
    Noise(F),
    SetAxes(Vf, Vf),
}

/// Which of the two frames a pointer is. Anything else is a bug in the port or in this harness, and
/// there is no third answer to fall back on.
fn frame_of(a: u32) -> Frame {
    match a {
        NODE => Frame::Node,
        // A null frame is the chassis without a transform: the one the low-speed upright controller
        // asks about without checking.
        ORIENTATION | 0 => Frame::Chassis,
        _ => panic!("the transform code was handed {a:#010x}, which is neither frame"),
    }
}

/// A stand-in for `transform_unrotate`: a different exact rotation for each of the two frames, so
/// that a port which skipped the call, or made it against the other frame, comes out with a different
/// number rather than the same one.
///
/// Both are signed lane permutations — orthonormal, and exact in `f32` — so a unit normal stays a unit
/// normal and the roll angle the controllers measure from it is a real angle. `w` comes through as the
/// original's does.
fn fake_unrotate(frame: Frame, v: Vf) -> Vf {
    let (x, y, z, w) = (v.x(), v.y(), v.z(), v.w());
    match frame {
        // A quarter turn about `z`: `x` and `y` swap roles, which is what the roll angle reads.
        Frame::Node => Vf::new(-y, x, z, w),
        Frame::Chassis => Vf::new(z, -x, y, w),
    }
}

/// A stand-in for the engine's smooth noise, the same table `tests/trick.rs` uses: both sides get the
/// identical single from the identical argument without either of them doing any arithmetic. It
/// reaches `+-1`, which is what pushes the balance command onto its own clamp.
fn fake_noise(x: f32) -> f32 {
    const WAVE: [f32; 8] = [0.0, 1.0, -1.0, 0.5, -0.25, 0.937_5, -0.812_5, 0.125];
    WAVE[(x.to_bits() >> 3) as usize & 7]
}

/// What a frame answers when it is asked for its forward axis: the `z` row of its world matrix, which
/// is what the original copies out, with `w` of zero.
fn forward_at(ee: &Ee, frame: u32) -> Vf {
    Vf::new(ee.f32_at(frame + 0xa0), ee.f32_at(frame + 0xa4), ee.f32_at(frame + 0xa8), 0.0)
}

/// The port's side of the four transform calls: the same stand-ins the hooks use, and the same
/// ordered record.
struct Stands<'a> {
    node_forward: Vf,
    chassis_forward: Vf,
    has_transform: bool,
    out: &'a mut Vec<Out>,
}

impl Outside for Stands<'_> {
    fn unrotate(&mut self, frame: Frame, v: Vf) -> Vf {
        self.out.push(Out::Unrotate(frame, v));
        fake_unrotate(frame, v)
    }
    fn forward(&mut self, frame: Frame) -> Vf {
        self.out.push(Out::Forward(frame));
        let v = match frame {
            Frame::Node => self.node_forward,
            // The one call the original makes with a frame that can be null — the low-speed upright
            // controller does not check. `orientation_axes` compares its two frame arguments, finds
            // them both zero, and answers with the default forward axis.
            Frame::Chassis if !self.has_transform => Vf::new(0.0, 0.0, 1.0, 0.0),
            Frame::Chassis => self.chassis_forward,
        };
        Vf::new(v.x(), v.y(), v.z(), 0.0)
    }
    fn noise(&mut self, t: F) -> F {
        self.out.push(Out::Noise(t));
        F(fake_noise(t.0))
    }
    fn set_axes(&mut self, forward: Vf, up: Vf) {
        self.out.push(Out::SetAxes(forward, up));
    }
}

/// A machine with the object graph wired up and every callee of the ground step hooked, with the two
/// start-up tables the four controllers read built in it.
///
/// The hooks are the proof's instrument as much as its scaffolding: each one records the call and
/// (for the transforms) hands back the vector the harness chose, so that what is left in the object
/// graph afterwards is exactly what the step itself wrote.
///
/// The `{cos, sin}` curve is built by the *game's* own builder, through a hooked `mem_alloc` that
/// hands it [`CURVE`], and the port is given those 6000 entries — the global has to be filled in
/// before any of this runs, because the no-hander's rotation reads it through that pointer. The arc
/// cosine goes the other way: [`AcosTable::build`] is already proved equal to the game's builder
/// (`tests/air.rs`), and writing the port's copy into the original's table is what proves both roll
/// angles read it at the same index.
fn harness() -> Option<(Ee, Rc<RefCell<Tape>>, AcosTable, CurveTable)> {
    let mut ee = game_or_skip()?;
    wire(&mut ee);

    ee.hooks.insert(addr("mem_alloc").unwrap(), Box::new(|ee| ee.set(2, CURVE as u64)));
    ee.call(addr("curve_table_build").unwrap(), &[], &[], 2_000_000_000);
    ee.hooks.remove(&addr("mem_alloc").unwrap());
    assert_eq!(ee.v0(), 1, "the game's curve table builder reported failure");
    assert_eq!(ee.u32_at(addr("curve_table_ptr").unwrap()), CURVE, "the table went somewhere else");
    let entries: Vec<[f32; 2]> = (0..CURVE_ENTRIES as u32)
        .map(|i| [ee.f32_at(CURVE + 8 * i), ee.f32_at(CURVE + 8 * i + 4)])
        .collect();
    let curve = CurveTable::from_entries(&entries);
    let acos = AcosTable::build();
    for (i, e) in acos.entries().iter().enumerate() {
        ee.set_f32(ACOS + 4 * i as u32, *e);
    }
    ee.set_u32(addr("acos_table_ptr").unwrap(), ACOS);
    ee.set_f32(addr("acos_table_scale").unwrap(), 14999.5);

    let tape = Rc::new(RefCell::new(Tape::default()));

    // The plain `(dt, bike)` and `(dt, wheel)` callees: record the call and do nothing. The four
    // controllers are no longer among them — S17 ported them, so they run for real on both sides.
    let plain: [(&str, fn(&Ee) -> Call); 4] = [
        ("bike_preload", |ee| Call::Preload(F(ee.f(12)))),
        ("bike_backflip", |ee| Call::Backflip(F(ee.f(12)))),
        ("bike_airborne_control", |ee| Call::AirborneControl(F(ee.f(12)))),
        ("bike_wheel_contacts", |ee| Call::Contacts(F(ee.f(12)))),
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
    // `transform_unrotate` is made from two places now: the step itself, which brings the chassis'
    // measured acceleration back into the bike's frame, and two of the four controllers, which bring
    // a contact normal in. The vector tells them apart — the step's is the chassis' own field, and a
    // controller's is a stack slot — and they are answered differently: the step's with the vector
    // the harness chose, a controller's with an exact rotation the port repeats.
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_unrotate").unwrap(),
        Box::new(move |ee| {
            let (out, frame, v) = (ee.r(4) as u32, ee.r(5) as u32, ee.r(6) as u32);
            let given = get_v(ee, v);
            if v == CHASSIS + 0x170 {
                let accel = t.borrow().local_accel;
                t.borrow_mut().calls.push(Call::Unrotate(given));
                put_v(ee, out, accel);
            } else {
                let frame = frame_of(frame);
                t.borrow_mut().out.push(Out::Unrotate(frame, given));
                put_v(ee, out, fake_unrotate(frame, given));
            }
        }),
    );
    // A frame's forward axis. The real function copies the `z` row of the frame's world matrix out
    // with a `w` of zero, which is what this does — and answers the identity's `z` for the null frame
    // the low-speed upright controller hands it without checking.
    let t = tape.clone();
    ee.hooks.insert(
        addr("orientation_axes").unwrap(),
        Box::new(move |ee| {
            let (frame, relative, out, up) = (ee.r(4) as u32, ee.r(5), ee.r(6) as u32, ee.r(7));
            assert_eq!(relative, 0, "the axes were asked for relative to another frame");
            assert_eq!(up, 0, "the up axis was asked for as well");
            t.borrow_mut().out.push(Out::Forward(frame_of(frame)));
            let v = if frame == 0 { Vf::new(0.0, 0.0, 1.0, 0.0) } else { forward_at(ee, frame) };
            put_v(ee, out, v);
        }),
    );
    // The engine's shared smooth noise, which only the balance controller asks for.
    let t = tape.clone();
    ee.hooks.insert(
        addr("smooth_noise").unwrap(),
        Box::new(move |ee| {
            let x = ee.f(12);
            t.borrow_mut().out.push(Out::Noise(F(x)));
            ee.set_f(0, fake_noise(x));
        }),
    );
    // The no-hander's last act: the bike's node, pointed by two axes. Nothing is written, because
    // what the real one writes is the node's world matrix, which is outside the compared graph — the
    // two axes it was handed are the port's own arithmetic, and those are what is compared.
    let t = tape.clone();
    ee.hooks.insert(
        addr("transform_set_axes").unwrap(),
        Box::new(move |ee| {
            assert_eq!(ee.r(4) as u32, NODE, "the no-hander pointed something other than the node");
            assert_eq!(ee.r(5), 0, "the axes were given relative to another frame");
            assert_eq!((ee.r(8), ee.r(9), ee.r(10)), (1, 1, 1), "the three sync flags are not all 1");
            let (forward, up) = (get_v(ee, ee.r(6) as u32), get_v(ee, ee.r(7) as u32));
            t.borrow_mut().out.push(Out::SetAxes(forward, up));
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
    Some((ee, tape, acos, curve))
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
    /// A contact normal. Mostly a real one — near enough a unit vector that the roll angle read off
    /// it is a real angle — and sometimes one of the four shapes the angle's own arithmetic turns on:
    /// straight up and straight down (where the flattened `xy` is nothing at all), a `y` of exactly
    /// zero (where the two clamps meet their ends), and nothing whatsoever.
    fn normal(&mut self) -> Vf {
        match self.bits() % 16 {
            0 => Vf::new(0.0, 1.0, 0.0, 0.0),
            1 => Vf::new(0.0, -1.0, 0.0, 0.0),
            2 => NOTHING,
            3 => Vf::new(self.f(-1.0, 1.0).0, 0.0, self.f(-1.0, 1.0).0, 0.0),
            _ => {
                let (x, y, z) = (self.f(-0.7, 0.7), self.f(-1.0, 1.0), self.f(-0.7, 0.7));
                let n = (x.0 * x.0 + y.0 * y.0 + z.0 * z.0).sqrt();
                let n = if n == 0.0 { 1.0 } else { n };
                Vf::new(x.0 / n, y.0 / n, z.0 / n, 0.0)
            }
        }
    }
    /// A word the original reads with `lw` and tests with `beqz`: the suspension's two "did it find
    /// anything" flags. Zero often, because that is a branch in the no-hander three times over, and
    /// otherwise the game's own `1` — a word, not a float, which is the whole reason [`Fresh`] keeps
    /// these seven as words.
    fn flag_word(&mut self) -> u32 {
        if self.one_in(3) { 0 } else { 1 }
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
    /// A power curve as the loader leaves one: a few points, strictly rising in RPM so that the
    /// seek's divide can never be by zero, with a torque at each. The slots past the last point are
    /// zero on both sides — the original's array is exactly that long, so nothing may read them.
    fn curve(&mut self) -> (u32, [F; mv::MAX_RPM_POINTS], [F; mv::MAX_RPM_POINTS]) {
        let points = 1 + self.bits() % 8;
        let (mut rpm, mut torque) = ([F::ZERO; mv::MAX_RPM_POINTS], [F::ZERO; mv::MAX_RPM_POINTS]);
        let mut at = self.f(400.0, 1500.0);
        for k in 0..points as usize {
            rpm[k] = at;
            torque[k] = self.f(8.0, 95.0);
            at = at + self.f(400.0, 1600.0);
        }
        (points, rpm, torque)
    }

    fn wheel(&mut self) -> Wheel {
        Wheel {
            balancing: self.b(),
            // Exactly zero one time in three: no donut is the no-hander's other branch, and the
            // step overwrites the back wheel's copy of this anyway.
            donut_scale: if self.one_in(3) { F::ZERO } else { self.f(0.0, 1.0) },
            spring: self.f(0.0, 4000.0),
            max_steer_angle: self.f(0.0, 0.8),
            steer_command: self.f(-0.8, 0.8),
            inv_radius: self.f(1.5, 4.0),
            spin: self.f(-20.0, 400.0),
            contact: self.b(),
            other_tire_down: self.b(),
            drive_torque: self.f(-400.0, 400.0),
            radius: self.f(0.25, 0.7),
            spin_from_torque: self.b(),
            contact_force_fwd: self.f(-900.0, 900.0),
            contact_normal: self.normal(),
            steered: self.b(),
            fresh: Fresh {
                // The first two are the suspension's `lw`-and-`beqz` flags, which the no-hander
                // reads; the other five are floats nothing here looks at.
                scalars: std::array::from_fn(|i| match i {
                    0 | 1 => self.flag_word(),
                    _ => self.f(-1000.0, 1000.0).to_bits(),
                }),
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
        // The no-hander's speed fade, and the balance controller's inertia limits: the same again.
        let min_stand_upright_speed = self.f(0.0, 12.0);
        let max_stand_upright_speed =
            if self.one_in(6) { min_stand_upright_speed } else { self.f(12.0, 45.0) };
        let min_balance_inertia = self.f(0.0, 1.0);
        let max_balance_inertia =
            if self.one_in(6) { min_balance_inertia } else { self.f(1.0, 4.0) };
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
            // S17's twenty-five, in offset order. The two pairs of limits are equal one time in six,
            // which is the branch each of the two speed fades takes when its range is empty.
            donut_roll_angle_scale: self.f(0.5, 3.0),
            max_roll_command: self.f(0.0, 0.8),
            max_roll_speed: self.maybe_zero(0.0, 40.0),
            roll_gain: self.f(0.0, 12.0),
            min_stand_upright_speed,
            max_stand_upright_speed,
            roll_control_gain: self.f(0.0, 40.0),
            // Mostly the size the clamp on the centre of gravity is meant for — a tenth of a metre
            // for a few degrees of roll — and sometimes big enough to peg it, which is the clamp.
            roll_control_rate_gain: if self.one_in(3) { self.f(0.0, 2.0) } else { self.f(0.0, 0.02) },
            roll_control_max_cg: self.f(0.0, 0.6),
            max_roll_rate_disturbance: self.f(0.0, 4.0),
            max_pitch_command: self.f(0.0, 0.9),
            pitch_gain: self.f(0.0, 60.0),
            pitch_rate_gain: self.f(0.0, 200.0),
            inv_wheelie_top_speed: self.f(0.02, 0.2),
            max_stoppie_pitch_command: self.f(0.0, 0.9),
            stoppie_pitch_gain: self.f(0.0, 60.0),
            stoppie_yaw_rate_scale: self.f(0.0, 4.0),
            stoppie_top_speed: self.maybe_zero(0.0, 40.0),
            balance_inertia_rate: self.f(0.0, 8.0),
            min_balance_inertia,
            max_balance_inertia,
            input_inertia: self.f(0.0, 5.0),
            max_balance_point: self.f(0.0, 1.5),
            balance_noise_amplitude: self.maybe_zero(0.0, 2.0),
            balance_noise_time_scale: self.f(0.0, 4.0),
        };
        let gears = 1 + (self.bits() % 6) as i32;
        let (points, curve_rpm, curve_torque) = self.curve();
        let eng = Engine {
            gear: 1 + (self.bits() % 7) as i32,
            gears,
            final_drive: self.f(1.0, 4.0),
            gear_ratios: std::array::from_fn(|_| self.f(0.5, 3.5)),
            ratio: self.f(0.5, 12.0),
            inv_ratio: self.f(0.08, 2.0),
            clutch_torque_scale: self.f(1.0, 4.0),
            clutch_locked: self.b(),
            // The loader's own invariant: the limit is the last point of the curve, so the engine is
            // never asked for a torque past the end of its own table.
            rev_limit: curve_rpm[points as usize - 1],
            idle_throttle: self.f(0.0, 0.25),
            // `EngineBraking` is a coefficient on the square of the revs, so it is this small: at
            // 8000 rpm this range costs the engine between 6 and 130 lb-ft.
            engine_braking: self.f(1.0e-7, 2.0e-6),
            inv_inertia: self.f(0.2, 8.0),
            clutch_static: self.f(0.0, 400.0),
            clutch_kinetic: self.f(0.0, 400.0),
            min_rpm: self.f(700.0, 1600.0),
            curve_rpm,
            curve_torque,
            // Sometimes parked past the end, which is the seek's own out-of-range path.
            curve: mv::Curve::at(points, self.bits() % (points + 1), self.f(0.0, 1.0)),
            torque: self.f(-200.0, 600.0),
            wheel_torque: self.f(-2000.0, 6000.0),
            // Occasionally below the curve and occasionally past it, because those are the two ends
            // the cursor's seek parks at.
            rpm: if self.one_in(6) { self.f(0.0, 600.0) } else { self.f(600.0, 13000.0) },
            road_rpm: self.f(-500.0, 13000.0),
            // 1.0 in the game: the engine's constructor sets it and no setter was found. Varied here
            // anyway, so that the port is proved to read it rather than to assume it.
            torque_scale: self.f(0.5, 1.5),
        };
        let r = Ride {
            g_peak: self.vec(-40.0, 40.0),
            clutch_timer: self.maybe_zero(0.0, 1.2),
            clutch_scale_start: self.f(0.5, 4.0),
            clutch_cofg_weight: self.f(0.0, 1.0),
            // A control, not a number: the engine's clutch tests it against 1 and against 0 exactly.
            clutch_in: self.control(),
            collision_near: self.b(),
            // The balance controller's four accumulators and the pitch controller's one, as a frame
            // part way into a wheelie or a balance would have left them, plus the stoppie's latch.
            balance_inertia: self.f(0.0, 4.0),
            balance_command: self.f(-1.0, 1.0),
            balance_point: self.f(-1.5, 1.5),
            balance_noise_time: self.f(0.0, 30.0),
            pitch_integral: self.f(-400.0, 400.0),
            stoppie: self.b(),
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
            euler: self.vec(-3.2, 3.2),
            torque: self.vec(-2000.0, 2000.0),
            world_ang_vel: self.vec(-12.0, 12.0),
            ang_vel: self.vec(-12.0, 12.0),
        };
        // The front wheel's own rate-limited brake; the rear's is generated beside it below.
        let front_brake = self.control();
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
            front_brake,
            reverse: if self.b() { F::ZERO } else { F::ONE },
            balancing: self.one_in(4),
            local_accel: self.vec(-40.0, 40.0),
            reverse_force: self.vec(-600.0, 600.0),
            node_distance: self.f(0.0, 0.5),
            stick_x: self.f(-1.1, 1.1),
            // The rear wheel's brake, a word of its own: the wheelie's veto reads this one and the
            // stoppie's gate the front's, so they differ half the time here and a port that read the
            // wrong one of the two fails. Equal the other half, because both halves of the
            // controller also have to be reached with one brake lever pulling both wheels.
            rear_brake: if self.b() { self.control() } else { front_brake },
            // The gates the two pitch controllers test are 0.5 forward and -1.0 back, with -0.1 the
            // stoppie's way out, so each of the three is landed on exactly often enough to prove the
            // compare is the original's and not one either side of it.
            lean_fore_aft: match self.bits() % 8 {
                0 => F(0.5),
                1 => F(-1.0),
                2 => F(-0.1),
                3 | 4 => self.f(-1.1, -0.9),
                _ => self.f(-1.1, 1.1),
            },
        };
        let (front, rear) = (self.wheel(), self.wheel());
        // The two frames' forward axes. One time in nine the chassis' is the front wheel's contact
        // normal, which is the one way the low-speed upright controller's flattened forward comes out
        // with no length at all.
        let chassis_forward =
            if self.one_in(9) { front.contact_normal } else { self.vec(-1.0, 1.0) };
        State {
            p,
            r,
            ch,
            front,
            rear,
            eng,
            st,
            node_forward: self.vec(-1.0, 1.0),
            chassis_forward,
        }
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
    let Some((mut ee, _, _, _)) = harness() else { return };
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
    let Some((mut ee, _, _, _)) = harness() else { return };
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
    let Some((mut ee, _, _, _)) = harness() else { return };
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
    let Some((mut ee, _, _, _)) = harness() else { return };
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

/// **The engine**, against `engine_update`, over 10,000 generated states: the power curve's cursor
/// and the torque it reads, the clutch's lock-up and break-away, the torque that reaches the driven
/// wheel's accumulator, and where the revs go next.
///
/// This is the one proof where the generator has to aim. The clutch locks up when the engine's revs
/// *cross* the revs the road is asking for, and a pair of unrelated random numbers almost never
/// straddles a third — so half the states are built the other way round: the road's revs are placed a
/// step either side of where the wheel now puts them, and the engine's revs are placed somewhere
/// along (or just past) that step. Both directions of the crossing and the near miss come out of the
/// same three draws, which is what the tally checks.
#[test]
fn the_engine_is_the_originals() {
    let Some((mut ee, _, _, _)) = harness() else { return };
    let mut g = Gen(0x6e13);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        if g.b() {
            // Where this substep's wheel speed puts the road's revs, the step the last substep took
            // to get there, and the engine somewhere along that step — past the end of it one time
            // in six, which is the near miss that does not lock up.
            let now = F(9.549296) * s.rear.spin * s.eng.ratio;
            let step = g.f(-400.0, 400.0);
            s.eng.road_rpm = now + step;
            s.eng.rpm = now + step * g.f(0.0, 1.2);
        }
        let points = s.eng.curve.points as usize;
        *tally
            .entry(if s.eng.rpm < s.eng.curve_rpm[0] {
                "below the curve"
            } else if !(s.eng.rpm < s.eng.curve_rpm[points - 1]) {
                "past the curve"
            } else {
                "on the curve"
            })
            .or_insert(0) += 1;
        *tally.entry(if s.rear.spin_from_torque { "load from the tire" } else { "load from the clutch" }).or_insert(0) += 1;

        reset(&mut ee);
        poke(&mut ee, &s);
        ee.call(addr("engine_update").unwrap(), &[ENGINE as u64], &[s.st.dt.0], 4_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("the engine, state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);

        let mut t = s.clone();
        mv::engine_update(&mut t.eng, &mut t.rear, t.st.gas_rate_limited, t.r.clutch_in, t.st.dt);
        *tally
            .entry(match (s.eng.clutch_locked, t.eng.clutch_locked) {
                (true, true) => "stayed locked",
                (true, false) => "broke away",
                (false, true) => "locked up",
                (false, false) => "stayed slipping",
            })
            .or_insert(0) += 1;
        *tally
            .entry(if t.eng.rpm == s.eng.min_rpm {
                "at the idle floor"
            } else if t.eng.rpm == s.eng.rev_limit {
                "at the rev limit"
            } else {
                "revving freely"
            })
            .or_insert(0) += 1;
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the engine", k, &was, &snap(&ee), &s);
    }
    covered(
        "the engine",
        &tally,
        &[
            "below the curve",
            "on the curve",
            "past the curve",
            "load from the tire",
            "load from the clutch",
            "stayed locked",
            "broke away",
            "locked up",
            "stayed slipping",
            "at the idle floor",
            "at the rev limit",
            "revving freely",
        ],
        100,
    );
}

// --- the whole step -------------------------------------------------------------------------------

/// Apply the port to a state, and give back the calls it says it made and the transform calls it
/// made on the way.
fn apply(t: &mut State, tb: &Tables<'_>, out: &mut Vec<Out>) -> Vec<Call> {
    let (p, st) = (t.p, t.st);
    let mut o = Stands {
        node_forward: t.node_forward,
        chassis_forward: t.chassis_forward,
        has_transform: t.ch.has_transform,
        out,
    };
    let mut b = Bike {
        ride: &mut t.r,
        chassis: &mut t.ch,
        front: &mut t.front,
        rear: &mut t.rear,
        engine: &mut t.eng,
    };
    mv::step(&p, &mut b, &st, tb, &mut o)
}

/// **The ground step**, against `bike_ground_step`, over 2000 generated frames: the whole object graph
/// word for word, and the ordered list of everything it called.
#[test]
fn the_ground_step_is_the_originals() {
    let Some((mut ee, tape, acos, curve)) = harness() else { return };
    let tables = Tables { acos: &acos, curve: &curve };
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
        // The four controllers, as far as the step decides whether they do anything at all. Their own
        // branches are tallied by their own proofs below.
        *tally
            .entry(if !s.st.both_wheels || s.st.balancing || s.st.gas != F::ZERO {
                "no upright hold"
            } else {
                "upright hold"
            })
            .or_insert(0) += 1;
        *tally.entry(if s.st.on_ground { "pitch control" } else { "airborne, no pitch" }).or_insert(0) += 1;
        *tally.entry(if s.st.balancing && s.front.contact { "balance" } else { "no balance" }).or_insert(0) += 1;

        reset(&mut ee);
        poke(&mut ee, &s);
        {
            let mut t = tape.borrow_mut();
            t.calls.clear();
            t.out.clear();
            t.local_accel = s.st.local_accel;
            t.reverse_force = s.st.reverse_force;
        }
        ee.call(addr("bike_ground_step").unwrap(), &[OBJ as u64], &[s.st.dt.0], 400_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("the ground step, state {k}: {e}\n{s:#?}"));
        let was = snap(&ee);
        let (theirs, theirs_out) = {
            let t = tape.borrow();
            (t.calls.clone(), t.out.clone())
        };

        let mut t = s.clone();
        let mut ours_out = Vec::new();
        let ours = apply(&mut t, &tables, &mut ours_out);
        same_out("the ground step", k, &ours_out, &theirs_out, &s);
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
        &[
            "balancing", "riding", "one substep", "several substeps", "reverse push", "no reverse",
            "donut", "no donut", "upright hold", "no upright hold", "pitch control",
            "airborne, no pitch", "balance", "no balance",
        ],
        100,
    );
}

// --- the four controllers -------------------------------------------------------------------------

/// Hand a state to the port and let one of its functions at it: the same five objects the original
/// reaches through its one pointer, and the [`Outside`] the harness' hooks stand in for.
fn with_port(
    t: &mut State,
    out: &mut Vec<Out>,
    f: impl FnOnce(&Params, &mut Bike<'_>, &Step, &mut dyn Outside),
) {
    let (p, st) = (t.p, t.st);
    let mut o = Stands {
        node_forward: t.node_forward,
        chassis_forward: t.chassis_forward,
        has_transform: t.ch.has_transform,
        out,
    };
    let mut b = Bike {
        ride: &mut t.r,
        chassis: &mut t.ch,
        front: &mut t.front,
        rear: &mut t.rear,
        engine: &mut t.eng,
    };
    f(&p, &mut b, &st, &mut o);
}

/// One controller over one state, both sides: run the original on the graph in RAM, run the port on
/// its own copy of the same state, and compare every word of the graph and the ordered list of
/// transform calls.
///
/// Gives the port's state back, so a proof can tally the branch a state *actually took* from what
/// came out of it rather than from a second copy of the controller's own conditions.
fn one_controller(
    ee: &mut Ee,
    tape: &Rc<RefCell<Tape>>,
    name: &'static str,
    func: &str,
    k: u32,
    s: &State,
    dt: F,
    port: impl FnOnce(&mut State, &mut Vec<Out>),
) -> State {
    reset(ee);
    poke(ee, s);
    tape.borrow_mut().out.clear();
    ee.call(addr(func).unwrap(), &[OBJ as u64], &[dt.0], 200_000);
    only_touched_the_regions(ee).unwrap_or_else(|e| panic!("{name}, state {k}: {e}\n{s:#?}"));
    let was = snap(ee);
    let theirs = tape.borrow().out.clone();

    let mut t = s.clone();
    let mut ours = Vec::new();
    port(&mut t, &mut ours);
    same_out(name, k, &ours, &theirs, s);
    reset(ee);
    poke(ee, &t);
    same(name, k, &was, &snap(ee), s);
    t
}

/// **The low-speed upright controller**, against `bike_stand_upright`, over 10,000 generated states.
///
/// Three states in four are aimed at the four conditions the controller sits behind — both wheels
/// down, not balancing, the throttle shut and under ten units of speed — because a state that does
/// not get through them proves only that the port gives up where the original does.
///
/// The branch that cannot be aimed at from a tuning value is the forward axis flattening to nothing,
/// and it needs the bike pointed exactly along the ground's normal with a normal that is exactly a
/// unit vector — anything else leaves a residue of the order of an ulp, which is not zero. One state
/// in six is built that way, and the mutation that proves the branch is reached is in the journal.
#[test]
fn the_upright_hold_is_the_originals() {
    let Some((mut ee, tape, _, _)) = harness() else { return };
    let mut g = Gen(0x1b0d);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        if !g.one_in(4) {
            s.st.both_wheels = true;
            s.st.balancing = false;
            s.st.gas = F::ZERO;
            // Either side of the ten the guard tests, and sometimes backwards.
            s.st.speed = if g.one_in(8) { g.f(-14.0, 0.0) } else { g.f(0.0, 12.0) };
        }
        let parallel = g.one_in(6);
        if parallel {
            let n = match g.bits() % 3 {
                0 => Vf::new(1.0, 0.0, 0.0, 0.0),
                1 => Vf::new(0.0, 1.0, 0.0, 0.0),
                _ => Vf::new(0.0, 0.0, 1.0, 0.0),
            };
            s.front.contact_normal = n;
            s.chassis_forward = n;
            s.ch.has_transform = true;
        }
        let held =
            s.st.both_wheels && !s.st.balancing && s.st.gas == F::ZERO && s.st.speed.abs() < F(10.0);
        *tally.entry(if held { "held" } else { "skipped" }).or_insert(0) += 1;
        if held {
            *tally.entry(if s.st.brake == F::ZERO { "rolling" } else { "braked" }).or_insert(0) += 1;
            *tally.entry(if s.ch.has_transform { "a heading" } else { "no transform" }).or_insert(0) += 1;
            *tally.entry(if parallel { "flat against the ground" } else { "a forward to use" }).or_insert(0) += 1;
        }
        let (_, h) = mv::substeps(s.st.dt);
        one_controller(&mut ee, &tape, "the upright hold", "bike_stand_upright", k, &s, h, |t, out| {
            with_port(t, out, |p, b, st, o| mv::stand_upright(p, b, st, o));
        });
    }
    covered(
        "the upright hold",
        &tally,
        &[
            "held", "skipped", "rolling", "braked", "a heading", "no transform",
            "flat against the ground", "a forward to use",
        ],
        100,
    );
}

/// **Balancing on one wheel**, against `bike_balance`, over 10,000 generated states.
///
/// The four accumulators are generated part-way along rather than from zero, so each of the three
/// clamps is met from both sides, and the tally is read off what came *out* of the port — the inertia
/// sitting on one of its two limits, the command on `+-1`, the balance point on `+-MaxBalancePoint`
/// and the centre of gravity on `RollControlMaxCG` — which is the branch the state actually took and
/// not a second copy of the controller's conditions.
#[test]
fn the_balance_is_the_originals() {
    let Some((mut ee, tape, acos, curve)) = harness() else { return };
    let tables = Tables { acos: &acos, curve: &curve };
    let mut g = Gen(0x2c1e);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        if !g.one_in(4) {
            s.front.contact = true;
        }
        *tally.entry(if s.front.contact { "balanced" } else { "no contact" }).or_insert(0) += 1;
        if s.front.contact {
            let n = s.front.contact_normal;
            *tally
                .entry(match () {
                    _ if F(n.y()) > F::ZERO => "rolled one way",
                    _ if F(n.y()) < F::ZERO => "rolled the other",
                    _ => "on edge",
                })
                .or_insert(0) += 1;
            *tally
                .entry(if s.p.balance_noise_amplitude == F::ZERO { "no noise" } else { "noise" })
                .or_insert(0) += 1;
        }
        let (_, h) = mv::substeps(s.st.dt);
        let t = one_controller(&mut ee, &tape, "the balance", "bike_balance", k, &s, h, |t, out| {
            with_port(t, out, |p, b, st, o| mv::balance(p, b, st, &tables, o, h));
        });
        if !s.front.contact {
            continue;
        }
        let (p, r) = (&s.p, &t.r);
        *tally
            .entry(match () {
                _ if r.balance_inertia == p.min_balance_inertia => "inertia at the floor",
                _ if r.balance_inertia == p.max_balance_inertia => "inertia at the ceiling",
                _ => "inertia between",
            })
            .or_insert(0) += 1;
        *tally
            .entry(if r.balance_command.abs() == F::ONE { "command clamped" } else { "command free" })
            .or_insert(0) += 1;
        *tally
            .entry(if r.balance_point.abs() == p.max_balance_point { "point clamped" } else { "point free" })
            .or_insert(0) += 1;
        *tally
            .entry(if F(t.ch.cofg.x()).abs() == p.roll_control_max_cg { "weight clamped" } else { "weight free" })
            .or_insert(0) += 1;
    }
    covered(
        "the balance",
        &tally,
        &[
            "balanced", "no contact", "rolled one way", "rolled the other", "noise", "no noise",
            "inertia at the floor", "inertia at the ceiling", "inertia between", "command clamped",
            "command free", "point clamped", "point free", "weight clamped", "weight free",
        ],
        100,
    );
}

/// **The wheelie and the stoppie**, against `bike_roll_pitch`, over 10,000 generated states.
///
/// Both halves run on the same state, so the aim is at their two sets of conditions at once: the
/// generator already lands the stick on each of the three gates it is tested against exactly, and the
/// throttle, the brake and the speed are nudged into range three states in four. The stoppie's latch
/// is generated already set as often as not, which is the only way to reach the half that runs
/// without starting.
#[test]
fn the_pitch_control_is_the_originals() {
    let Some((mut ee, tape, _, _)) = harness() else { return };
    let mut g = Gen(0x3d2f);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        if !g.one_in(4) {
            s.st.on_ground = true;
            s.st.speed = g.f(4.0, 20.0);
            s.rear.contact = g.b();
            s.front.contact = g.b();
            s.r.clutch_in = if g.one_in(3) { g.control() } else { F::ZERO };
            s.st.gas_rate_limited = if g.one_in(3) { F::ZERO } else { g.f(0.1, 1.0) };
            s.st.front_brake = if g.one_in(3) { F::ZERO } else { g.f(0.1, 1.0) };
            s.st.rear_brake = if g.b() { g.control() } else { s.st.front_brake };
        }
        let lean = s.st.lean_fore_aft;
        let wheelie = s.rear.contact
            && s.st.gas_rate_limited > F::ZERO
            && s.st.speed > F(5.0)
            && s.st.rear_brake == F::ZERO
            && s.r.clutch_in == F::ZERO
            && lean >= F(0.5);
        let start = s.front.contact
            && !s.r.stoppie
            && s.st.front_brake > F::ZERO
            && s.st.speed > F(15.0)
            && lean <= F(-1.0);
        *tally.entry(if s.st.on_ground { "grounded" } else { "airborne" }).or_insert(0) += 1;
        if s.st.on_ground {
            *tally.entry(if wheelie { "wheelie" } else { "no wheelie" }).or_insert(0) += 1;
            *tally.entry(if start { "stoppie starts" } else { "no stoppie start" }).or_insert(0) += 1;
            *tally
                .entry(if s.r.stoppie || start { "stoppie runs" } else { "no stoppie" })
                .or_insert(0) += 1;
            if wheelie || s.r.stoppie || start {
                *tally
                    .entry(if s.ch.has_transform { "a yaw torque" } else { "no transform" })
                    .or_insert(0) += 1;
            }
        }
        let (_, h) = mv::substeps(s.st.dt);
        let t = one_controller(&mut ee, &tape, "the pitch control", "bike_roll_pitch", k, &s, h, |t, out| {
            with_port(t, out, |p, b, st, o| mv::roll_pitch(p, b, st, o, h));
        });
        if s.st.on_ground && (s.r.stoppie || start) {
            *tally
                .entry(if t.r.stoppie { "stoppie held" } else { "stoppie let go" })
                .or_insert(0) += 1;
            *tally
                .entry(if s.p.stoppie_top_speed == F::ZERO { "no speed fade" } else { "a speed fade" })
                .or_insert(0) += 1;
        }
    }
    covered(
        "the pitch control",
        &tally,
        &[
            "grounded", "airborne", "wheelie", "no wheelie", "stoppie starts", "no stoppie start",
            "stoppie runs", "no stoppie", "a yaw torque", "no transform", "stoppie held",
            "stoppie let go", "a speed fade", "no speed fade",
        ],
        100,
    );
}

/// **The no-hander's roll**, against `bike_no_hander`, over 10,000 generated states.
///
/// The one function here whose whole answer leaves through a call the port does not make itself, so
/// what the `Out` list carries is the proof: the frame the ground normal was brought into, the
/// argument the noise was walked to — there is none here — and the two axes the node was pointed by.
/// Not one word of the compared graph moves.
///
/// The aim is at the three ways a wheel counts as properly down (a suspension ray that found
/// something, a collision sweep whose normal is more than a little off level, and neither) and at the
/// two ends of the speed fade, backwards as well as forwards.
#[test]
fn the_no_hander_is_the_originals() {
    let Some((mut ee, tape, acos, curve)) = harness() else { return };
    let tables = Tables { acos: &acos, curve: &curve };
    let mut g = Gen(0x4e30);
    let mut tally = std::collections::BTreeMap::new();
    for k in 0..10_000 {
        let mut s = g.state();
        if !g.one_in(4) {
            s.front.contact = g.b();
            s.rear.contact = true;
        }
        // Either side of the fade's two speeds, and backwards one time in four.
        if !g.one_in(3) {
            let v = g.f(0.0, 1.3) * s.p.max_stand_upright_speed;
            s.st.speed = if g.one_in(4) { F::ZERO - v } else { v };
        }
        let down = |w: &Wheel| {
            w.contact
                && (w.fresh.scalars[0] != 0
                    || (w.fresh.scalars[1] != 0 && F(w.contact_normal.y()) > F(0.17)))
        };
        *tally
            .entry(match () {
                _ if down(&s.front) => "the front wheel's ground",
                _ if down(&s.rear) => "the back wheel's ground",
                _ => "no ground",
            })
            .or_insert(0) += 1;
        let v = s.st.speed.abs();
        *tally
            .entry(match () {
                _ if s.p.min_stand_upright_speed == s.p.max_stand_upright_speed => "no fade",
                _ if v <= s.p.min_stand_upright_speed => "level",
                _ if v >= s.p.max_stand_upright_speed => "with the ground",
                _ => "between",
            })
            .or_insert(0) += 1;
        *tally.entry(if s.st.speed < F::ZERO { "backwards" } else { "forwards" }).or_insert(0) += 1;
        *tally
            .entry(if s.rear.donut_scale > F::ZERO { "a donut" } else { "no donut" })
            .or_insert(0) += 1;
        *tally
            .entry(if s.p.max_roll_speed == F::ZERO { "no roll command" } else { "a roll command" })
            .or_insert(0) += 1;
        let (_, h) = mv::substeps(s.st.dt);
        one_controller(&mut ee, &tape, "the no-hander", "bike_no_hander", k, &s, h, |t, out| {
            with_port(t, out, |p, b, st, o| mv::no_hander(p, b, st, &tables, o, h));
        });
    }
    covered(
        "the no-hander",
        &tally,
        &[
            "the front wheel's ground", "the back wheel's ground", "no ground", "no fade", "level",
            "with the ground", "between", "backwards", "forwards", "a donut", "no donut",
            "a roll command", "no roll command",
        ],
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
        // The rider's weight fore and aft: a wheelie out of the second phase and through the balance,
        // and the stick hard back under the brakes at the end, which is a stoppie.
        lean_fore_aft: F(match phase {
            2 => 0.8,
            4 => 0.6,
            5 => -1.05,
            _ => 0.0,
        }),
        // Sideways on the seat, which is what the balance controller's command integrates.
        #[allow(clippy::float_arithmetic)]
        stick_x: F(if phase == 4 { 0.6 * (t * 1.7).sin() } else { 0.0 }),
        // The two wheels' brakes as the rate limiter leaves them: one lever, both wheels, held
        // through the jump and again at the end.
        front_brake: F(if phase == 3 || phase == 5 { 1.0 } else { 0.0 }),
        rear_brake: F(if phase == 3 || phase == 5 { 1.0 } else { 0.0 }),
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
    let Some((mut ee, tape, acos, curve)) = harness() else { return };
    let tables = Tables { acos: &acos, curve: &curve };
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
            donut_roll_angle_scale: F(1.5),
            max_roll_command: F(0.5),
            max_roll_speed: F(14.0),
            roll_gain: F(6.0),
            // The loader's two defaults, in the units it leaves them in: 10 and 30 times 1.46666.
            min_stand_upright_speed: F(14.6666),
            max_stand_upright_speed: F(44.0),
            roll_control_gain: F(12.0),
            roll_control_rate_gain: F(0.05),
            roll_control_max_cg: F(0.25),
            max_roll_rate_disturbance: F(1.5),
            max_pitch_command: F(0.6),
            pitch_gain: F(30.0),
            pitch_rate_gain: F(90.0),
            inv_wheelie_top_speed: F::ONE / F(22.0),
            max_stoppie_pitch_command: F(0.45),
            stoppie_pitch_gain: F(25.0),
            stoppie_yaw_rate_scale: F(1.2),
            stoppie_top_speed: F(24.0),
            balance_inertia_rate: F(1.6),
            min_balance_inertia: F(0.4),
            max_balance_inertia: F(2.2),
            input_inertia: F(1.8),
            max_balance_point: F::ONE,
            balance_noise_amplitude: F(0.35),
            balance_noise_time_scale: F(0.8),
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
            // The attitude the two pitch controllers work against, and the rates they damp with. The
            // chassis would turn these over as it integrated, and the chassis is not this task's — so
            // the fixture sets a bike a few degrees nose-up and slightly out of shape and leaves it,
            // which is enough for the integral to wind up and the accumulators to drift.
            euler: Vf::new(0.11, 0.0, -0.06, 1.0),
            torque: NOTHING,
            world_ang_vel: Vf::new(0.2, -0.1, 0.35, 1.0),
            ang_vel: Vf::new(0.2, -0.1, 0.35, 1.0),
        },
        front: Wheel {
            max_steer_angle: F(0.52),
            inv_radius: F(2.8),
            radius: F(0.357),
            contact: true,
            spin_from_torque: false,
            drive_torque: F::ZERO,
            steered: true,
            // A little off camber, and the suspension's ray saying it found it.
            contact_normal: Vf::new(0.12, 0.98, 0.16, 0.0),
            ..g.wheel()
        },
        rear: Wheel {
            max_steer_angle: F::ZERO,
            inv_radius: F(2.6),
            radius: F(0.385),
            contact: true,
            spin_from_torque: false,
            drive_torque: F::ZERO,
            steered: false,
            contact_normal: Vf::new(0.08, 0.99, 0.11, 0.0),
            ..g.wheel()
        },
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
            idle_throttle: F(0.08),
            engine_braking: F(6.0e-7),
            inv_inertia: F(2.4),
            clutch_static: F(260.0),
            clutch_kinetic: F(180.0),
            min_rpm: F(1100.0),
            // Five points, the last at the rev limit: a 450 four-stroke's shape, in torque.
            curve_rpm: {
                let mut a = [F::ZERO; mv::MAX_RPM_POINTS];
                for (k, v) in [2000.0, 4500.0, 6500.0, 8500.0, 11000.0].into_iter().enumerate() {
                    a[k] = F(v);
                }
                a
            },
            curve_torque: {
                let mut a = [F::ZERO; mv::MAX_RPM_POINTS];
                for (k, v) in [22.0, 31.0, 34.0, 30.0, 21.0].into_iter().enumerate() {
                    a[k] = F(v);
                }
                a
            },
            curve: mv::Curve::new(5),
            torque: F::ZERO,
            wheel_torque: F::ZERO,
            rpm: F(1100.0),
            road_rpm: F::ZERO,
            torque_scale: F::ONE,
        },
        st: tape_at(0, F::ZERO),
        node_forward: Vf::new(0.0, 0.1, 0.995, 0.0),
        chassis_forward: Vf::new(0.0, 0.1, 0.995, 0.0),
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
            t.out.clear();
            t.local_accel = st.local_accel;
            t.reverse_force = st.reverse_force;
        }
        ee.call(addr("bike_ground_step").unwrap(), &[OBJ as u64], &[st.dt.0], 400_000);
        only_touched_the_regions(&ee).unwrap_or_else(|e| panic!("frame {i}: {e}"));
        let t = tape.borrow();
        theirs.push((snap(&ee), t.calls.clone(), t.out.clone()));
    }

    // Pass two: the port, 240 frames over its own state, compared with each of theirs in turn.
    let mut t = start.clone();
    for (i, (was, their_calls, their_out)) in theirs.into_iter().enumerate() {
        t.st = tape_at(i as u32, speed_at(i as u32));
        let mut ours_out = Vec::new();
        let ours = apply(&mut t, &tables, &mut ours_out);
        same_out("the replay", i as u32, &ours_out, &their_out, &t);
        assert_eq!(ours, their_calls, "frame {i}: the calls differ");
        reset(&mut ee);
        poke(&mut ee, &t);
        same("the replay", i as u32, &was, &snap(&ee), &t);
    }
}
