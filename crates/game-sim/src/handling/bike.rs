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
//! F31b1: the PreProcess ([`ground_head`]). F31b2: the Force body ([`ground_step`]), with the rider's
//! controllers ([`crate::handling::step`]'s, MD63b) on the words the quad's types do not carry
//! ([`Rider`]). F31b3: the PostProcess ([`air_state`]), with the bike's no-hander ([`no_hander`],
//! MX Unleashed's [`crate::r#move::no_hander`]) and the vehicle post ([`vehicle_post`]).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::seed::{RECORD, WHEELS, Words};
use crate::quad::{Body, Call, Moto, Params, Quad};
use crate::r#move as mv;
use ps2_float::{Acc, Dest, F, Vf};

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

// --- F31b2: the Force body ------------------------------------------------------------------------

/// What the bike's rider controllers read and keep that the quad's types do not carry: the balance's
/// keys and its four accumulators, the stick's raw `x` (before the input limiter) and the body's
/// centre of gravity, whose `x` the balance writes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rider {
    pub balance_inertia_rate: F,
    pub min_balance_inertia: F,
    pub max_balance_inertia: F,
    pub input_inertia: F,
    pub balance_noise_amplitude: F,
    pub balance_noise_time_scale: F,
    pub max_balance_point: F,
    pub max_roll_rate_disturbance: F,
    pub roll_control_gain: F,
    pub roll_control_rate_gain: F,
    pub roll_control_max_cg: F,
    pub balance_inertia: F,
    pub balance_command: F,
    pub balance_point: F,
    pub balance_noise_time: F,
    pub stick_x: F,
    pub cofg: Vf,
}

/// Whoever runs the bike's Force body: the quad's steps ([`Moto`]), and the two things only the
/// bike's own controllers ask for — the [`Rider`] words, kept with the rest of the bike's, and the
/// engine's smooth noise the balance walks along (a call out, as the original's is; the port's
/// [`crate::ride::noise`] is a stand-in for MX Unleashed's, not this).
pub trait Rides: Moto {
    fn rider(&mut self) -> &mut Rider;
    fn noise(&mut self, t: F) -> F;
}

/// The engine's sine/cosine curve and the arc cosine table the balance reads, built once.
fn tables() -> mv::Tables<'static> {
    static ACOS: std::sync::OnceLock<crate::air::AcosTable> = std::sync::OnceLock::new();
    static CURVE: std::sync::OnceLock<crate::camera::CurveTable> = std::sync::OnceLock::new();
    mv::Tables { acos: ACOS.get_or_init(crate::air::AcosTable::build), curve: CURVE.get_or_init(crate::camera::CurveTable::build) }
}

/// The controllers' outside: the frames come off the body's quaternion ([`crate::handling::step`]
/// answers them), so only the noise is asked, of the bike's [`Rides`].
struct Noise<'a, M>(&'a mut M);

impl<M: Rides> mv::Outside for Noise<'_, M> {
    fn unrotate(&mut self, _: mv::Frame, _: Vf) -> Vf {
        unreachable!("the bike's controllers rotate by the body's quaternion")
    }
    fn forward(&mut self, _: mv::Frame) -> Vf {
        unreachable!("the bike's controllers take the forward axis from the body's quaternion")
    }
    fn noise(&mut self, t: F) -> F {
        self.0.noise(t)
    }
    fn set_axes(&mut self, _: Vf, _: Vf) {
        unreachable!("the bike's Force body does not point the node")
    }
}

/// **The bike's rider controllers**, the Force body's first three with control on: the stand-upright
/// (it guards itself: both wheels down, not balancing, off the throttle, under 10 ft/s), the balance
/// when the two balance words agree, and the wheelie and stoppie. They are
/// [`crate::handling::step`]'s ports (MD63b), run on the bike's words: the front wheel's contact,
/// normal and steering, the rear's contact and normal, the signed speed, the levers, the body's
/// force, moment, local angular velocity, quaternion and centre of gravity.
fn controllers(p: &Params, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Rides) {
    let r = *m.rider();
    let keys = mv::Params {
        gravity: q.gravity,
        balance_inertia_rate: r.balance_inertia_rate,
        min_balance_inertia: r.min_balance_inertia,
        max_balance_inertia: r.max_balance_inertia,
        input_inertia: r.input_inertia,
        balance_noise_amplitude: r.balance_noise_amplitude,
        balance_noise_time_scale: r.balance_noise_time_scale,
        max_balance_point: r.max_balance_point,
        max_roll_rate_disturbance: r.max_roll_rate_disturbance,
        roll_control_gain: r.roll_control_gain,
        roll_control_rate_gain: r.roll_control_rate_gain,
        roll_control_max_cg: r.roll_control_max_cg,
        max_pitch_command: p.max_pitch_command,
        pitch_gain: p.pitch_gain,
        pitch_rate_gain: p.pitch_rate_gain,
        inv_wheelie_top_speed: p.inv_wheelie_top_speed,
        max_stoppie_pitch_command: p.max_stoppie_pitch_command,
        stoppie_pitch_gain: p.stoppie_pitch_gain,
        stoppie_yaw_rate_scale: p.stoppie_yaw_rate_scale,
        stoppie_top_speed: p.stoppie_top_speed,
        ..Default::default()
    };
    let mut ride = mv::Ride {
        balance_inertia: r.balance_inertia,
        balance_command: r.balance_command,
        balance_point: r.balance_point,
        balance_noise_time: r.balance_noise_time,
        pitch_integral: q.pitch_integral,
        stoppie: q.stoppie,
        clutch_in: q.clutch_in,
        ..Default::default()
    };
    let mut chassis = mv::Chassis {
        force: body.force,
        torque: body.moment,
        cofg: r.cofg,
        euler: q.euler,
        world_ang_vel: q.world_ang_vel,
        ang_vel: body.ang_vel,
        ..Default::default()
    };
    let wheel = |w: &crate::quad::Wheel| mv::Wheel { contact: w.touching, contact_normal: w.normal, steered: w.steered, ..Default::default() };
    let (mut front, mut rear) = (wheel(&q.wheels[FRONT]), wheel(&q.wheels[REAR]));
    let mut engine = mv::Engine::default();
    let s = mv::Step {
        dt,
        on_ground: q.on_ground,
        both_wheels: q.all_down,
        balancing: q.balancing,
        gas: q.gas,
        brake: q.brake,
        speed: q.speed(),
        stick_x: r.stick_x,
        lean: q.lean,
        lean_fore_aft: q.lean_fore_aft,
        gas_rate_limited: q.gas_limited,
        front_brake: q.front_brake,
        rear_brake: q.rear_brake,
        ..Default::default()
    };
    let orientation = body.orientation;
    {
        let mut b = mv::Bike { ride: &mut ride, chassis: &mut chassis, front: &mut front, rear: &mut rear, engine: &mut engine };
        crate::handling::step::stand_upright(&keys, &mut b, &s, &mut Noise(m), orientation);
        if q.balancing {
            crate::handling::step::balance(&keys, &mut b, &s, &tables(), &mut Noise(m), orientation, dt);
        }
        crate::handling::step::roll_pitch(&keys, &mut b, &s, orientation, dt);
    }
    (body.force, body.moment) = (chassis.force, chassis.torque);
    (q.pitch_integral, q.stoppie) = (ride.pitch_integral, ride.stoppie);
    q.wheels[FRONT].steered = front.steered;
    let r = m.rider();
    (r.balance_inertia, r.balance_command, r.balance_point, r.balance_noise_time) =
        (ride.balance_inertia, ride.balance_command, ride.balance_point, ride.balance_noise_time);
    r.cofg = chassis.cofg;
}

/// **The bike's ground push**: with the rear wheel touching (the wheel's own flag, where the quad
/// reads its rear-down) and the push's gate open, the bike's push — with its flag set, a force
/// `gain·(target − speed)` along the bike's own `z`, taken into the world.
fn ground_push(q: &Quad, body: &mut Body) {
    if !q.wheels[REAR].touching || q.push_gate == F::ZERO || !q.push {
        return;
    }
    let z = q.push_gain * (q.push_target - q.speed());
    body.add_local_force(NOTHING.merge(Dest::Z, Vf::new(0.0, 0.0, z.0, 0.0)));
}

/// **The bike's Force body** (its ground substep), once a substep.
///
/// Skipped as the PreProcess is ([`Quad::active`]). The vehicle's gravity first. With control on
/// (no wreck reason, [`Quad::control_off`]): the rider's controllers ([`controllers`]: the
/// stand-upright, the balance while the balance words agree, the wheelie and stoppie), the preload,
/// backflip, air spin, engine, air drag and airborne control, then the ground push. Then, control or
/// not: both wheels cleared, the contacts spread, both suspensions, both tire forces, the rear's
/// pair flag (set with the front touching and the rear not, cleared otherwise), both wheels
/// integrated, the rev limiter and the peak rates. Every round goes front, then rear.
///
/// The quad's ([`crate::quad::ground_step`]) runs four wheels and its own stand-upright and
/// roll/pitch, no balance, and gates its push on its rear-down flag.
pub fn ground_step(p: &Params, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Rides) {
    if !q.active {
        return;
    }
    m.call(Call::Gravity, q, body);
    if !q.control_off {
        controllers(p, q, body, dt, m);
        for c in [Call::Preload(dt), Call::Backflip(dt), Call::SpinAir(dt), Call::EngineUpdate(dt), Call::AirDrag(dt), Call::AirborneControl(dt)] {
            m.call(c, q, body);
        }
        ground_push(q, body);
    }
    for w in [FRONT, REAR] {
        m.call(Call::WheelClear(w), q, body);
    }
    m.call(Call::Contacts, q, body);
    for w in [FRONT, REAR] {
        m.call(Call::Suspension(w, dt), q, body);
    }
    for w in [FRONT, REAR] {
        m.call(Call::TireForces(w, dt), q, body);
    }
    q.wheels[REAR].rear_pair = q.wheels[FRONT].touching && !q.wheels[REAR].touching;
    for w in [FRONT, REAR] {
        m.call(Call::WheelIntegrate(w, dt), q, body);
    }
    m.call(Call::RevLimit, q, body);
    m.call(Call::PeakRates, q, body);
}

/// The [`Rider`] words, bike-relative (the balance's keys and accumulators, the raw stick `x`), and
/// the centre of gravity, body-relative.
const RIDER_KEYS: [u32; 11] = [0x14fc, 0x1500, 0x1504, 0x1508, 0x1514, 0x151c, 0x1520, 0x1398, 0x138c, 0x1390, 0x1394];
const RIDER_STATE: [u32; 4] = [0x14f8, 0x150c, 0x1510, 0x1518];
const STICK_X: u32 = 0x12a8;
const COFG: u32 = 0x1a0;

fn rider_fields(r: &mut Rider) -> [&mut F; 16] {
    let Rider {
        balance_inertia_rate, min_balance_inertia, max_balance_inertia, input_inertia, balance_noise_amplitude,
        balance_noise_time_scale, max_balance_point, max_roll_rate_disturbance, roll_control_gain,
        roll_control_rate_gain, roll_control_max_cg, balance_inertia, balance_command, balance_point,
        balance_noise_time, stick_x, ..
    } = r;
    [
        balance_inertia_rate, min_balance_inertia, max_balance_inertia, input_inertia, balance_noise_amplitude,
        balance_noise_time_scale, max_balance_point, max_roll_rate_disturbance, roll_control_gain,
        roll_control_rate_gain, roll_control_max_cg, balance_inertia, balance_command, balance_point,
        balance_noise_time, stick_x,
    ]
}

fn rider_offsets() -> impl Iterator<Item = u32> {
    RIDER_KEYS.into_iter().chain(RIDER_STATE).chain([STICK_X])
}

/// The bike at `bike`'s [`Rider`], its body at `body`.
pub fn read_rider(m: &impl Words, bike: u32, body: u32) -> Rider {
    let mut r = Rider { cofg: m.v(body + COFG), ..Default::default() };
    for (f, o) in rider_fields(&mut r).into_iter().zip(rider_offsets()) {
        *f = m.f(bike + o);
    }
    r
}

/// [`read_rider`]'s inverse.
pub fn rider_words(r: &Rider, bike: u32, body: u32) -> Vec<(u32, u32)> {
    let mut r = *r;
    let mut out = Vec::new();
    let cofg = r.cofg;
    for (f, o) in rider_fields(&mut r).into_iter().zip(rider_offsets()) {
        put_f(&mut out, bike + o, *f);
    }
    put_v(&mut out, body + COFG, cofg);
    out
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

// --- F31b3: the PostProcess ---------------------------------------------------------------------------

/// What the bike's PostProcess reads and keeps that [`Quad`] does not carry: the no-hander's keys and
/// the two words each wheel's ground test reads, and the vehicle's own words the vehicle post and the
/// world test read and the put out of the world clears.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Post {
    /// `MinStandUprightSpeed`/`MaxStandUprightSpeed` (`+0x1374`, `+0x1378`), `MaxRollCommand`
    /// (`+0x1360`), `MaxRollSpeed` (`+0x1368`), `RollGain` (`+0x1370`), `DonutRollAngleScale`
    /// (`+0x1238`).
    pub min_stand_upright_speed: F,
    pub max_stand_upright_speed: F,
    pub max_roll_command: F,
    pub max_roll_speed: F,
    pub roll_gain: F,
    pub donut_roll_angle_scale: F,
    /// Each wheel's suspension ray hit (`+0xd0`) and swept contact (`+0xd4`), front then rear: read
    /// as words, set when not zero.
    pub ray_hit: [bool; 2],
    pub swept: [bool; 2],
    /// The rear wheel's donut (`+0x38`), as the no-hander reads it.
    pub donut: F,
    /// The word holding the vehicle's flag byte (`+0x45`, its second byte): bit 0 the world's box,
    /// bit 6 its floor, bit 1 the node's bounds asked for, bit 2 cleared by the vehicle post.
    pub flag_word: u32,
    /// The world's bounds (through `+0xe8`), the in-world test's.
    pub bounds: crate::atv_moto::Bounds,
    /// The body is on (its word `+0`): without it the vehicle post does nothing.
    pub body_on: bool,
    /// The wreck reason (`+0xc80`) and the two words the put out of the world zeroes with it
    /// (`+0x868`, `+0xc1c`; not traced further).
    pub wreck_reason: u32,
    pub cleared: [u32; 2],
    /// The bike's post id (`+0xd20`), handed to the put out of the world's message.
    pub post_id: u32,
}

impl Post {
    /// The vehicle's flag byte.
    pub fn flags(&self) -> u8 {
        (self.flag_word >> 8) as u8
    }
}

/// The bike's own words [`Post`] holds, bike-relative.
const NO_HANDS: [u32; 6] = [0x1374, 0x1378, 0x1360, 0x1368, 0x1370, 0x1238];
const FLAG_WORD: u32 = 0x44;
const CLEARED: [u32; 2] = [0x868, 0xc1c];
const POST_ID: u32 = 0xd20;
/// The world's floor and the pointer to its box (world-relative), the box's corners (box-relative).
const FLOOR: u32 = 0x98;
const AREA: u32 = 0xa8;
const AREA_CORNERS: [u32; 2] = [0x30, 0x40];

/// The bike at `bike`'s [`Post`], its body at `body`.
pub fn read_post(m: &impl Words, bike: u32, body: u32) -> Post {
    let f = |o: u32| m.f(bike + o);
    let wheel = |k: usize| m.word(bike + WHEELS[k]);
    let world = m.word(bike + WORLD);
    let area = m.word(world + AREA);
    Post {
        min_stand_upright_speed: f(NO_HANDS[0]),
        max_stand_upright_speed: f(NO_HANDS[1]),
        max_roll_command: f(NO_HANDS[2]),
        max_roll_speed: f(NO_HANDS[3]),
        roll_gain: f(NO_HANDS[4]),
        donut_roll_angle_scale: f(NO_HANDS[5]),
        ray_hit: [0, 1].map(|k| m.word(wheel(k) + 0xd0) != 0),
        swept: [0, 1].map(|k| m.word(wheel(k) + 0xd4) != 0),
        donut: m.f(wheel(1) + 0x38),
        flag_word: m.word(bike + FLAG_WORD),
        bounds: crate::atv_moto::Bounds {
            floor: m.f(world + FLOOR),
            area: (area != 0).then(|| (m.v(area + AREA_CORNERS[0]), m.v(area + AREA_CORNERS[1]))),
        },
        body_on: m.word(body) != 0,
        wreck_reason: m.word(bike + WRECK_REASON),
        cleared: CLEARED.map(|o| m.word(bike + o)),
        post_id: m.word(bike + POST_ID),
    }
}

/// [`read_post`]'s inverse for the words the PostProcess writes: the flag word, the wreck reason and
/// the two cleared with it. (The rest it only reads.)
pub fn post_words(p: &Post, bike: u32) -> Vec<(u32, u32)> {
    let mut out = vec![(bike + FLAG_WORD, p.flag_word), (bike + WRECK_REASON, p.wreck_reason)];
    out.extend(CLEARED.into_iter().zip(p.cleared).map(|(o, w)| (bike + o, w)));
    out
}

/// What the bike's PostProcess hands out beyond the quad's steps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Out {
    /// The vehicle post's node placed (`atv_node_orientation`: the bike's node, 0, the body's
    /// quaternion, the origin).
    Place { orientation: Vf, at: Vf },
    /// The bike out of the world: the world's message queue handed category 8, message 13, with the
    /// bike's post id.
    OutOfWorld(u32),
    /// The bike's put-back (its vt `+0x3a8`), handed `place` and `keep`.
    PutBack { place: bool, keep: bool },
}

/// Whoever runs the bike's PostProcess: the bike's [`Rides`] (the [`Rider`], whose balance state the
/// PostProcess clears and whose centre of gravity the origin is taken off), the [`Post`] words, and
/// the calls [`Out`]. The quad's steps it shares go through [`Moto`]: the body's reset and the
/// respawn, the spatial update, the node's bounds, the collision sync and the node's position.
pub trait Posts: Rides {
    fn post(&mut self) -> &mut Post;
    fn out(&mut self, c: Out, q: &mut Quad, body: &mut Body);
}

/// The no-hander's outside: the frames off the body's quaternion (`q* v q` on VU0 and
/// [`crate::math::quat_axes`]'s forward axis), and the node's axes handed back as the body's new
/// quaternion — the frame built round the forward axis ([`crate::math::basis`] at `(0, 0, 0, 1)`,
/// orthogonalised, the forward kept) and turned into a quaternion ([`crate::math::mat4_to_quat`]).
struct Hands<'a>(&'a mut Body);

impl mv::Outside for Hands<'_> {
    fn unrotate(&mut self, _: mv::Frame, v: Vf) -> Vf {
        self.0.to_local(v)
    }
    fn forward(&mut self, _: mv::Frame) -> Vf {
        let (f, _) = crate::math::quat_axes(self.0.orientation);
        Vf::new(f[0].0, f[1].0, f[2].0, 0.0)
    }
    fn noise(&mut self, _: F) -> F {
        unreachable!("the no-hander makes no noise")
    }
    fn set_axes(&mut self, forward: Vf, up: Vf) {
        let m = crate::math::basis(Vf::new(0.0, 0.0, 0.0, 1.0), forward, up, true, true);
        self.0.orientation = crate::math::mat4_to_quat(&m);
    }
}

/// **The bike's no-hander**: MX Unleashed's ([`crate::r#move::no_hander`]) clause for clause — the
/// ground normal of the first wheel with real ground under it (front, then rear; loaded and either the
/// ray's hit or a swept contact above 0.17 up), faded from upright by the signed speed between the two
/// stand-upright speeds, the bike's roll off it, the command off the speed (and the rear's donut) times
/// the lean, the roll moved towards it at the roll gain, the faded normal turned about the forward axis
/// — with the frames off the body's quaternion and the result written back as the body's quaternion
/// ([`Hands`]), where MX Unleashed asks and sets its node.
///
/// The original adds the angle onto `0` (`adda.s 0, angle; madd.s`), which makes a `-0` angle `+0`;
/// [`crate::r#move::no_hander`] adds onto the angle. The two differ only when the step is a `-0` too,
/// which takes a product that underflows: not folded here (see the journal).
pub fn no_hander(p: &Post, q: &Quad, body: &mut Body, dt: F) {
    let keys = mv::Params {
        min_stand_upright_speed: p.min_stand_upright_speed,
        max_stand_upright_speed: p.max_stand_upright_speed,
        max_roll_command: p.max_roll_command,
        max_roll_speed: p.max_roll_speed,
        roll_gain: p.roll_gain,
        donut_roll_angle_scale: p.donut_roll_angle_scale,
        ..Default::default()
    };
    let wheel = |slot: usize, k: usize| {
        let w = &q.wheels[slot];
        let mut x = mv::Wheel { contact: w.touching, contact_normal: w.normal, ..Default::default() };
        x.fresh.scalars[0] = u32::from(p.ray_hit[k]);
        x.fresh.scalars[1] = u32::from(p.swept[k]);
        x
    };
    let (mut front, mut rear) = (wheel(FRONT, 0), wheel(REAR, 1));
    rear.donut_scale = p.donut;
    let (mut ride, mut chassis, mut engine) = (mv::Ride::default(), mv::Chassis::default(), mv::Engine::default());
    let mut b = mv::Bike { ride: &mut ride, chassis: &mut chassis, front: &mut front, rear: &mut rear, engine: &mut engine };
    let s = mv::Step { dt, speed: q.speed(), lean: q.lean, ..Default::default() };
    mv::no_hander(&keys, &mut b, &s, &tables(), &mut Hands(body), dt);
}

/// The crossup is taken above this speed (ft/s); the tip-over flag is set with the up axis's `y` at or
/// below `TIPPED_UP`.
const CROSSUP_SPEED: f32 = 10.0;
const TIPPED_UP: f32 = 0.25;
/// `pi/2` as the original's `lui`/`ori` pair holds it.
const HALF_PI: f32 = f32::from_bits(0x3fc9_0fdb);
/// The put out of the world's message: category and id.
pub const OUT_OF_WORLD: (u32, u32) = (8, 13);

/// `[-1, 1]` as the original's compares and moves: anything unordered comes back as 1.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn clamp_unit(c: F) -> F {
    if !(c <= F::ONE) {
        F::ONE
    } else if c < F(-1.0) {
        F(-1.0)
    } else {
        c
    }
}

/// **The bike's vehicle post** (the vehicle's, shared with the quad: `atv_vehicle_post`), the
/// PostProcess's last call but one: with the body on, the origin (the body's position less its turned
/// centre of gravity, [`crate::atv_moto::origin`]) tested against the world
/// ([`crate::atv_moto::in_world`]); inside, the node placed there with the body's orientation
/// ([`Out::Place`]), outside the body's reset and the respawn. Then the spatial update, the node's
/// bounds with bit 1 of the flag byte (read after the calls before it), the collision sync, and bit 2
/// of the flag byte cleared. [`crate::quad::Steps`] runs the same with the scene and the collidables
/// the quad's types carry.
pub fn vehicle_post(q: &mut Quad, body: &mut Body, m: &mut impl Posts) {
    if !m.post().body_on {
        return;
    }
    let at = crate::atv_moto::origin(body, m.rider().cofg);
    let p = *m.post();
    if crate::atv_moto::in_world(p.flags(), &p.bounds, at) {
        m.out(Out::Place { orientation: body.orientation, at }, q, body);
    } else {
        m.call(Call::BodyReset, q, body);
        m.call(Call::Respawn, q, body);
    }
    m.call(Call::SpatialUpdate, q, body);
    if m.post().flags() & 0x02 != 0 {
        m.call(Call::NodeBounds, q, body);
    }
    m.call(Call::CollisionSync, q, body);
    m.post().flag_word &= !(0x04 << 8);
}

/// **The vehicle's end of the net unpack** (F45a), what [`super::update::net_unpack`] does last with
/// a remote bike: the spatial update, the node's bounds and the collision sync, as
/// [`vehicle_post`] makes them, but with no place, no out-of-world test and the node's bounds asked
/// whatever the flags say.
pub fn net_post(q: &mut Quad, body: &mut Body, m: &mut impl Moto) {
    m.call(Call::SpatialUpdate, q, body);
    m.call(Call::NodeBounds, q, body);
    m.call(Call::CollisionSync, q, body);
}

/// **The bike's PostProcess** (its air state), once a frame after the substeps.
///
/// Skipped as the PreProcess is ([`Quad::active`]). With control on and the balance words differing,
/// the balance's state cleared (its inertia, command, point and noise time, the [`Rider`]'s) and the
/// [`no_hander`]. Then as the quad's ([`crate::quad::air_state`]) on two wheels: the speed, the
/// velocity into the bike, the forward and up axes and the euler angles off the quaternion, the spin
/// into the world; on the ground with either wheel down, all down with both (the quad's front and
/// rear pair flags are not written); the hang time; the contact velocity the front's if it touches,
/// else the rear's, else the body's own; the crossup; the tip-over flag. Then, unlike the quad's, the
/// origin tested against the world ([`crate::atv_moto::in_world`]); outside, the wreck reason and the
/// two words with it zeroed, the bike's out-of-world message posted ([`Out::OutOfWorld`]) and the
/// bike put back ([`Out::PutBack`], place and keep). Last, the [`vehicle_post`], the node's position
/// and its kept copy.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn air_state(q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Posts) {
    use crate::air::{cross, normalize};
    use ps2_float::fpu;
    if !q.active {
        return;
    }
    if !q.control_off && !q.balancing {
        let r = m.rider();
        (r.balance_noise_time, r.balance_point, r.balance_inertia, r.balance_command) = (F::ZERO, F::ZERO, F::ZERO, F::ZERO);
        let p = *m.post();
        no_hander(&p, q, body, dt);
    }
    let t = tables();
    let v = body.velocity;
    q.speed_len = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(v.x(), v.x()), v.y(), v.y()), v.z(), v.z())));
    q.local_vel = body.to_local(v);
    let (fwd, up) = crate::math::quat_axes(body.orientation);
    q.heading = Vf::new(fwd[0].0, fwd[1].0, fwd[2].0, q.heading.w());
    q.up = Vf::new(up[0].0, up[1].0, up[2].0, q.up.w());
    q.euler = crate::math::quat_to_euler(body.orientation, t.acos);
    q.world_ang_vel = body.to_world(body.ang_vel);

    q.was_airborne = q.airborne;
    let (front, rear) = (q.wheels[FRONT], q.wheels[REAR]);
    q.on_ground = front.touching || rear.touching;
    q.all_down = front.touching && rear.touching;
    q.airborne = !q.on_ground;
    if !q.was_airborne && q.airborne {
        q.hang_time = F::ZERO;
    }
    if q.airborne {
        q.hang_time = q.hang_time + dt;
    }
    q.contact_vel = if front.touching {
        front.contact_vel
    } else if rear.touching {
        rear.contact_vel
    } else {
        q.local_vel
    };

    let heading = normalize(Vf::new(q.heading.x(), 0.0, q.heading.z(), 1.0));
    let mut angle = F::ZERO;
    if !q.flipping && !q.spinning && !(q.speed_len <= F(CROSSUP_SPEED)) {
        let vel = normalize(Vf::new(v.x(), 0.0, v.z(), 1.0));
        let c = cross(heading, vel);
        let sin = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(c.x(), c.x()), c.y(), c.y()), c.z(), c.z())));
        let sin = clamp_unit(clamp_unit(clamp_unit(sin)));
        angle = F(HALF_PI) - t.acos.at(sin);
        if c.y() < 0.0 {
            angle = angle * F(-1.0);
        }
    }
    q.crossup = if !(q.authority <= F::ZERO) {
        if q.authority_time == F::ZERO {
            F::ZERO
        } else {
            map_clamp(q.authority, q.authority_time, F::ZERO, F::ZERO, angle)
        }
    } else {
        angle
    };
    q.tipped = q.up.y() <= TIPPED_UP;

    let at = crate::atv_moto::origin(body, m.rider().cofg);
    let p = *m.post();
    if !crate::atv_moto::in_world(p.flags(), &p.bounds, at) {
        let p = m.post();
        p.wreck_reason = 0;
        p.cleared = [0; 2];
        let id = p.post_id;
        q.control_off = false;
        m.out(Out::OutOfWorld(id), q, body);
        m.out(Out::PutBack { place: true, keep: true }, q, body);
    }
    vehicle_post(q, body, m);
    m.call(Call::NodePosition, q, body);
    q.node_pos_kept = q.node_pos;
}
