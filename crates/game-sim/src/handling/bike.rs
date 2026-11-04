//! **SLUS-21104's bike listeners** (F31b): the physics world's PreProcess, Force and PostProcess on
//! MX vs. ATV's two-wheeled bike, ported as they read there and proved against its own code and
//! against C15's rides (`tests/mxvatv_bike_ride.rs`).
//!
//! The quad is the bike with four wheels ([`crate::quad`]), and the bike's words sit where the
//! quad's do, so the bike's listeners run on the quad's types: [`Quad`] and [`Body`] for what they
//! read and keep, [`Moto`] for the steps they call ([`crate::quad::Steps`] has those ported). The
//! bike's front wheel is the quad's left front ([`FRONT`]) and its rear the left rear ([`REAR`]):
//! the two the quad pairs as the bike's own wheel pair is paired. The quad's right-hand wheels stay
//! untouched.
//!
//! F31b1: the PreProcess ([`ground_head`]).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::seed::{RECORD, WHEELS, Words};
use crate::quad::{Body, Call, Moto, Params, Quad};
use ps2_float::{Acc, F, Vf};

/// The bike's two wheels, in the quad's slots.
pub const FRONT: usize = crate::quad::LEFT_FRONT;
pub const REAR: usize = crate::quad::LEFT_REAR;

/// The stick counts as held at full lock from here (either way).
const FULL_LOCK: f32 = 0.987;
/// The donut needs the stick deflected past this.
const DONUT_DEFLECTION: f32 = 0.9;
/// The springs' speed map, stopped to moving (ft/s).
const SPRING_STOPPED: f32 = 14.6666;
const SPRING_MOVING: f32 = 29.3332;
/// The rodata quadword the peak-rates vector and the body's per-frame quadword are reset to.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// The original's inlined map and clamp (`(x − a) / (b − a)`, `adda`/`madd` onto `out0..out1`, then
/// `max`/`min`). Callers test `a == b` themselves.
fn map_clamp(x: F, a: F, b: F, out0: F, out1: F) -> F {
    crate::air::map_clamp(x, a, b, out0, out1)
}

/// **The bike's PreProcess** (the ground head), once a frame before the substeps.
///
/// Skipped with no vehicle state or in states 3 and 4 ([`Quad::active`]). The input limiter first;
/// off the ground the front brake is let go. Then the steering and the body's turn locks
/// (`axes`, the body's [`crate::atv_body::Body::angular_axes`], set through its virtual):
///
/// - balancing (the two balance words equal): the front wheel held up; unless the vehicle is
///   `held`, all three turns free, and nothing else;
/// - otherwise the front let go, and wrecked ([`Quad::control_off`]: the wreck reason is set) its
///   steering command zeroed and, unless `held`, all three turns free;
/// - otherwise the steering command into the front and [`Quad::steer_command`]: the stick times
///   the lock times the speed fade, with the full-lock option the full-lock angle ramped in over its
///   time while the stick is hard over and the fade on the rest of the lock only. Then the turns:
///   in the air all three free; on the ground the yaw locked, or with the vehicle `held` all three.
///
/// The donut on the rear (both wheels down, under the donut speed, the stick past 0.9 and the
/// throttle open: the quad's ramps on the stick angle), each wheel's spring rate off the speed, the
/// peak-rates vector and the body's per-frame quadword reset, and the gearbox (handed the rear's
/// one-over-radius), the clutch and the head and tail.
///
/// Where the quad's head writes a pair, the bike's writes its one wheel; the quad's touches no turn
/// lock.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn ground_head(p: &Params, q: &mut Quad, body: &mut Body, held: bool, axes: &mut [bool; 3], dt: F, m: &mut impl Moto) {
    if !q.active {
        return;
    }
    m.call(Call::InputLimiter(dt), q, body);
    if !q.on_ground {
        q.front_brake = F::ZERO;
    }
    if q.balancing {
        q.wheels[FRONT].balancing = true;
        if !held {
            *axes = [true; 3];
        }
    } else {
        q.wheels[FRONT].balancing = false;
        if q.control_off {
            q.wheels[FRONT].steer_command = F::ZERO;
            if !held {
                *axes = [true; 3];
            }
        } else {
            let fade = if p.steer_speed_1 == p.steer_speed_2 {
                p.steer_scale_1
            } else {
                map_clamp(q.speed(), p.steer_speed_1, p.steer_speed_2, p.steer_scale_1, p.steer_scale_2)
            };
            let lock = q.wheels[FRONT].max_steer_angle;
            let command = if !p.full_lock {
                lock * (q.lean * fade)
            } else {
                q.full_lock_timer = if q.lean.abs() < F(FULL_LOCK) { F::ZERO } else { q.full_lock_timer + dt };
                let ramp = F::ONE.min(q.full_lock_timer / p.full_lock_time);
                Acc::mula(q.lean, p.full_lock_angle * ramp).madd(q.lean * fade, lock - p.full_lock_angle)
            };
            q.wheels[FRONT].steer_command = command;
            q.steer_command = command;
            *axes = if !q.on_ground {
                [true; 3]
            } else if !held {
                [true, true, false]
            } else {
                [false; 3]
            };
        }
    }

    q.wheels[REAR].donut = if !q.all_down
        || !(q.speed_len < p.max_donut_speed)
        || q.deflection <= F(DONUT_DEFLECTION)
        || q.gas == F::ZERO
    {
        F::ZERO
    } else {
        crate::quad::donut(p, q.stick_angle)
    };

    let (stopped, moving) = (F(SPRING_STOPPED), F(SPRING_MOVING));
    q.wheels[FRONT].spring = map_clamp(q.speed_len, stopped, moving, p.front_spring.0, p.front_spring.1);
    q.wheels[REAR].spring = map_clamp(q.speed_len, stopped, moving, p.rear_spring.0, p.rear_spring.1);

    q.attitude = NOTHING;
    body.reset_each_frame = NOTHING;
    let inv_radius = q.wheels[REAR].inv_radius;
    m.call(Call::AutoShift { dt, inv_radius }, q, body);
    m.call(Call::Clutch(dt), q, body);
    m.call(Call::HeadTail, q, body);
}

// --- the bike's words (F31b1) ------------------------------------------------------------------------

/// The vehicle state (`+0xbc4`, its kind at `+0x28`), the wreck reason (`+0xc80`), the two balance
/// words (`+0x1524`, `+0x152c`) and the hold mode (`+0xddc`), bike-relative: what [`read_quad`]'s
/// guards are made of.
const STATE: u32 = 0xbc4;
const WRECK_REASON: u32 = 0xc80;
const BALANCE: [u32; 2] = [0x1524, 0x152c];
const HELD: u32 = 0xddc;
/// The physics world (`+0xe8`), whose gravity is at its `+0x40`.
const WORLD: u32 = 0xe8;

/// The keys [`ground_head`] and the quad's controllers read, off the bike at `bike` (the `.phy`'s, as
/// the game left them in the bike).
pub fn read_params(m: &impl Words, bike: u32) -> Params {
    let f = |o: u32| m.f(bike + o);
    Params {
        max_pitch_command: f(0x139c),
        pitch_gain: f(0x13a0),
        pitch_rate_gain: f(0x13a4),
        inv_wheelie_top_speed: f(0x13a8),
        max_stoppie_pitch_command: f(0x13cc),
        stoppie_pitch_gain: f(0x13d0),
        stoppie_yaw_rate_scale: f(0x13d4),
        stoppie_top_speed: f(0x13d8),
        steer_speed_1: f(0x1300),
        steer_scale_1: f(0x1304),
        steer_speed_2: f(0x1308),
        steer_scale_2: f(0x130c),
        full_lock: m.b(bike + 0x12f0),
        full_lock_angle: f(0x12f4),
        full_lock_time: f(0x12f8),
        max_donut_speed: f(0x122c),
        donut_angle: f(0x1230),
        donut_angle_back: f(0x1234),
        front_spring: (f(0x137c), f(0x1380)),
        rear_spring: (f(0x1384), f(0x1388)),
    }
}

/// The vehicle's hold mode (`+0xddc`): set, the PreProcess leaves the body's turn locks as they are
/// where it would free them, and locks all three on the ground where it would lock the yaw.
pub fn read_held(m: &impl Words, bike: u32) -> bool {
    m.b(bike + HELD)
}

/// A bike word [`read_quad`] reads and [`quad_words`] writes: where it is and which field.
enum Word {
    B(u32, fn(&mut Quad) -> &mut bool),
    F(u32, fn(&mut Quad) -> &mut F),
    V(u32, fn(&mut Quad) -> &mut Vf),
}

/// The bike's own words, at the quad's offsets (`tests/atv_quad.rs`' map).
const QUAD_WORDS: [Word; 39] = [
    Word::B(0x324, |q| &mut q.on_ground),
    Word::B(0x1270, |q| &mut q.all_down),
    Word::B(0x1274, |q| &mut q.front_down),
    Word::B(0x1278, |q| &mut q.rear_down),
    Word::V(0xc30, |q| &mut q.contact_vel),
    Word::F(0x310, |q| &mut q.brake),
    Word::F(0x314, |q| &mut q.gas),
    Word::F(0x318, |q| &mut q.gas_limited),
    Word::V(0x360, |q| &mut q.euler),
    Word::V(0x340, |q| &mut q.world_ang_vel),
    Word::F(0x12cc, |q| &mut q.front_brake),
    Word::F(0x12d0, |q| &mut q.rear_brake),
    Word::F(0x12d4, |q| &mut q.clutch_in),
    Word::F(0x12e8, |q| &mut q.lean),
    Word::F(0x12ec, |q| &mut q.lean_fore_aft),
    Word::F(0x13ac, |q| &mut q.pitch_integral),
    Word::B(0x13b0, |q| &mut q.stoppie),
    Word::B(0xdd8, |q| &mut q.push),
    Word::F(0x123c, |q| &mut q.push_target),
    Word::F(0x1240, |q| &mut q.push_gain),
    Word::F(0x12c4, |q| &mut q.push_gate),
    Word::F(0x32c, |q| &mut q.speed_len),
    Word::F(0x12c8, |q| &mut q.steer_command),
    Word::F(0x12fc, |q| &mut q.full_lock_timer),
    Word::F(0x12b8, |q| &mut q.stick_angle),
    Word::F(0x12bc, |q| &mut q.deflection),
    Word::V(0x1250, |q| &mut q.attitude),
    Word::V(0x350, |q| &mut q.local_vel),
    Word::V(0xbe0, |q| &mut q.heading),
    Word::V(0xbf0, |q| &mut q.up),
    Word::B(0xde4, |q| &mut q.was_airborne),
    Word::B(0xde0, |q| &mut q.airborne),
    Word::F(0x1190, |q| &mut q.hang_time),
    Word::F(0x330, |q| &mut q.crossup),
    Word::B(0x1198, |q| &mut q.flipping),
    Word::B(0x11d4, |q| &mut q.spinning),
    Word::F(0x1188, |q| &mut q.authority),
    Word::F(0x118c, |q| &mut q.authority_time),
    Word::V(0xbd0, |q| &mut q.node_pos),
];

/// Each wheel's words, wheel-relative, in the quad's wheel.
enum WheelWord {
    B(u32, fn(&mut crate::quad::Wheel) -> &mut bool),
    F(u32, fn(&mut crate::quad::Wheel) -> &mut F),
    V(u32, fn(&mut crate::quad::Wheel) -> &mut Vf),
}

const WHEEL_WORDS: [WheelWord; 11] = [
    WheelWord::B(0x2c, |w| &mut w.steered),
    WheelWord::B(0xb4, |w| &mut w.rear_pair),
    WheelWord::B(0xdc, |w| &mut w.touching),
    WheelWord::V(0x130, |w| &mut w.normal),
    WheelWord::V(0x190, |w| &mut w.contact_vel),
    WheelWord::B(0x30, |w| &mut w.balancing),
    WheelWord::F(0x70, |w| &mut w.max_steer_angle),
    WheelWord::F(0x8ac, |w| &mut w.steer_command),
    WheelWord::F(0x38, |w| &mut w.donut),
    WheelWord::F(0x48, |w| &mut w.spring),
    WheelWord::F(0x838, |w| &mut w.inv_radius),
];

/// The bike at `bike` as the quad's steps see it: its wheels in [`FRONT`] and [`REAR`], the
/// guards (a vehicle state outside 3 and 4, the two balance words equal, a wreck reason set) worked
/// out, the world's gravity and the collision record's tip-over flag read through their pointers,
/// and the kept node position (`+0xc40`).
pub fn read_quad(m: &impl Words, bike: u32) -> Quad {
    let mut q = Quad::default();
    for w in &QUAD_WORDS {
        match *w {
            Word::B(o, f) => *f(&mut q) = m.b(bike + o),
            Word::F(o, f) => *f(&mut q) = m.f(bike + o),
            Word::V(o, f) => *f(&mut q) = m.v(bike + o),
        }
    }
    for (slot, o) in [(FRONT, WHEELS[0]), (REAR, WHEELS[1])] {
        let at = m.word(bike + o);
        let w = &mut q.wheels[slot];
        for x in &WHEEL_WORDS {
            match *x {
                WheelWord::B(o, f) => *f(w) = m.b(at + o),
                WheelWord::F(o, f) => *f(w) = m.f(at + o),
                WheelWord::V(o, f) => *f(w) = m.v(at + o),
            }
        }
    }
    let state = m.word(bike + STATE);
    q.active = state != 0 && m.word(state + 0x28).wrapping_sub(3) >= 2;
    q.balancing = m.word(bike + BALANCE[0]) == m.word(bike + BALANCE[1]);
    q.control_off = m.word(bike + WRECK_REASON) != 0;
    q.gravity = m.v(m.word(bike + WORLD) + 0x40);
    q.tipped = m.b(m.word(bike + RECORD) + 0x24);
    q.node_pos_kept = m.v(bike + 0xc40);
    q
}

fn put_bool(out: &mut Vec<(u32, u32)>, at: u32, b: bool) {
    out.push((at, u32::from(b)));
}

fn put_f(out: &mut Vec<(u32, u32)>, at: u32, x: F) {
    out.push((at, x.0.to_bits()));
}

fn put_v(out: &mut Vec<(u32, u32)>, at: u32, v: Vf) {
    for (k, w) in v.to_bits().into_iter().enumerate() {
        out.push((at + 4 * k as u32, w));
    }
}

/// [`read_quad`]'s inverse for the words the bike keeps: the bike's own words, both wheels' (at
/// `wheels`, front then rear), the record's tip-over flag (at `record`) and the kept node position.
/// The guards, read from words the listeners do not write, and the world's gravity are not given.
/// A flag goes back as 0 or 1.
pub fn quad_words(q: &Quad, bike: u32, wheels: [u32; 2], record: u32) -> Vec<(u32, u32)> {
    let mut q = *q;
    let mut out = Vec::new();
    for w in &QUAD_WORDS {
        match *w {
            Word::B(o, f) => put_bool(&mut out, bike + o, *f(&mut q)),
            Word::F(o, f) => put_f(&mut out, bike + o, *f(&mut q)),
            Word::V(o, f) => put_v(&mut out, bike + o, *f(&mut q)),
        }
    }
    for (slot, at) in [(FRONT, wheels[0]), (REAR, wheels[1])] {
        let w = &mut q.wheels[slot];
        for x in &WHEEL_WORDS {
            match *x {
                WheelWord::B(o, f) => put_bool(&mut out, at + o, *f(w)),
                WheelWord::F(o, f) => put_f(&mut out, at + o, *f(w)),
                WheelWord::V(o, f) => put_v(&mut out, at + o, *f(w)),
            }
        }
    }
    put_bool(&mut out, record + 0x24, q.tipped);
    put_v(&mut out, bike + 0xc40, q.node_pos_kept);
    out
}
