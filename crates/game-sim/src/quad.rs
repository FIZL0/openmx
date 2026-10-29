//! **The ATV's own ground steps** (MD37b), ported from MX vs. ATV Unleashed's PS2 ELF (SLUS-21104),
//! the only game that has the quad: MX Unleashed has no ATV, so every proof here is against
//! SLUS-21104's code (`tests/atv_quad.rs`), never MX Unleashed's.
//!
//! MX vs. ATV's quad is its bike with four wheels: the same `.phy` reader, engine, air, trick and wreck
//! steps, run per substep by the physics world. What the quad replaces in the bike's ground substep
//! is here:
//!
//! - [`stand_upright`] in place of the bike's stand-upright and balance: the four contact normals'
//!   mean, and no balance wobble at all.
//! - [`roll_pitch`]: the wheelie and stoppie controllers, read off a rear and a front wheel of the
//!   pair, and turning both front wheels' steering off while either holds the quad up.
//! - [`ground_pitch_torque`]: the bike's own push under the quad's guard.
//! - [`rear_pair_flag`]: the flag the tire forces leave on both rear wheels when a front wheel is down
//!   and neither rear is.
//!
//! The rest of the quad's step (its PreProcess, the tire models, the air state, the update tail, the
//! wreck, put-back and net unpack) runs on SLUS-21104's bike steps, which are not ported yet; it is
//! filed, not here.
//!
//! The quad is described by what the steps read, [`Quad`] and [`Body`], not by its layout: the field
//! map is research/oracle.txt's (`# MD37`, `# MD37a`, `# MD37b`).

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::math::quat_axes;
use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf, fpu, fpu::Acc};

/// The four wheels, in the quad's order: the steered pair, then the driven pair.
pub const LEFT_FRONT: usize = 0;
pub const RIGHT_FRONT: usize = 1;
pub const LEFT_REAR: usize = 2;
pub const RIGHT_REAR: usize = 3;

/// Below this speed (ft/s, either way) the quad is held from sliding sideways down a slope.
const STAND_UPRIGHT_SPEED: f32 = 10.0;
/// The wheelie needs more than this speed and the stick at least this far back.
const WHEELIE_SPEED: f32 = 5.0;
const WHEELIE_LEAN: f32 = 0.5;
/// The stoppie starts above this speed with the stick fully forward, and ends at or under the end
/// speed or with the stick back past the end lean.
const STOPPIE_SPEED: f32 = 15.0;
const STOPPIE_LEAN: f32 = -1.0;
const STOPPIE_END_LEAN: f32 = -0.1;
const STOPPIE_END_SPEED: f32 = 7.0;
/// The pitch integrator's rate, and the stoppie's yaw gain: both 500 on the quad (the bike's stoppie
/// yaw gain is 1000).
const PITCH_GAIN_RATE: f32 = 500.0;

/// The rodata quadword a zero-length direction is replaced with.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// One wheel, as the quad's own steps see it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    /// The wheel takes the steering (cleared on both fronts while a wheelie or stoppie holds the quad).
    pub steered: bool,
    /// The rear-pair flag [`rear_pair_flag`] sets.
    pub rear_pair: bool,
    /// The wheel is on the ground.
    pub touching: bool,
    /// The ground's normal under it. `w` comes along: the sums keep the first wheel's.
    pub normal: Vf,
}

/// What the quad's steps read and keep of the quad itself.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Quad {
    /// On the ground at all.
    pub on_ground: bool,
    /// The flags the wheel contacts leave: the whole quad down, a front down, a rear down.
    pub all_down: bool,
    pub front_down: bool,
    pub rear_down: bool,
    /// The bike's balance runs when its two balance words agree; the quad has no balance, and its
    /// stand-upright runs only when they do not.
    pub balancing: bool,
    /// Signed speed along the quad, ft/s.
    pub speed: F,
    /// The brake (Square) and the throttle (Cross), and the throttle after its rate limit.
    pub brake: F,
    pub gas: F,
    pub gas_limited: F,
    /// The chassis' euler pitch.
    pub pitch: F,
    /// The chassis' angular velocity in the world.
    pub world_ang_vel: Vf,
    /// The front and rear brakes and the clutch lever as the PreProcess split them.
    pub front_brake: F,
    pub rear_brake: F,
    pub clutch_in: F,
    /// The stick, sideways and fore and aft (forward is negative).
    pub lean: F,
    pub lean_fore_aft: F,
    /// The pitch controllers' integral, and whether a stoppie is on.
    pub pitch_integral: F,
    pub stoppie: bool,
    /// The ground push's gate, target speed and gain (the bike's `[Mass Properties]` pitch-torque keys).
    pub push: bool,
    pub push_target: F,
    pub push_gain: F,
    /// What gates the ground push on the quad.
    pub push_gate: F,
    /// The physics world's gravity.
    pub gravity: Vf,
    pub wheels: [Wheel; 4],
}

/// The quad's wheelie and stoppie keys (the bike's, from the same `.phy` sections).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Params {
    pub max_pitch_command: F,
    pub pitch_gain: F,
    pub pitch_rate_gain: F,
    pub inv_wheelie_top_speed: F,
    pub max_stoppie_pitch_command: F,
    pub stoppie_pitch_gain: F,
    pub stoppie_yaw_rate_scale: F,
    pub stoppie_top_speed: F,
}

/// The quad's rigid body: what the steps push on and read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Body {
    /// The force and moment accumulators. Only `xyz` is ever added to.
    pub force: Vf,
    pub moment: Vf,
    /// The orientation quaternion the body's world/local rotations use.
    pub orientation: Vf,
    /// The angular velocity in the body's own frame (the controllers read `x`, the pitch rate).
    pub ang_vel: Vf,
}

fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(s.0).bc(Field::X)))
}

fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `a·b` over `xyz`, on the FPU's accumulator: `mula`, `madda`, `madd`.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

impl Body {
    /// The body's add-force: `xyz` into the force accumulator.
    pub fn add_force(&mut self, f: Vf) {
        self.force = add_xyz(self.force, f);
    }

    /// The body's add-moment: `xyz` into the moment accumulator.
    pub fn add_moment(&mut self, m: Vf) {
        self.moment = add_xyz(self.moment, m);
    }

    /// A world vector into the body's frame by its orientation `q`, as the body's world-moment call
    /// does it on VU0 before adding the moment: `q* v q` in two `vopmula`/`vmadd*`/`vopmsub` rounds,
    /// one accumulator each. `w` is `v`'s.
    pub fn to_local(&self, v: Vf) -> Vf {
        let q = self.orientation;
        let acc = Vf::madd(Vf::opmula(v, q), v, q.bc(Field::W));
        let t = Vf::opmsub(acc, q, v);
        let qv = q.mul(v);
        let acc = Vf::opmula(t, q);
        let acc = Vf::madd(acc, q, qv.bc(Field::X));
        let acc = Vf::madd(acc, q, qv.bc(Field::Y));
        let acc = Vf::madd(acc, q, qv.bc(Field::Z));
        let acc = Vf::madd(acc, t, q.bc(Field::W));
        v.merge(Dest::XYZ, Vf::opmsub(acc, q, t))
    }

    /// A body-frame vector into the world, the same two rounds with the operands the other way about
    /// (the body's local-force call). `w` is `v`'s.
    pub fn to_world(&self, v: Vf) -> Vf {
        let q = self.orientation;
        let acc = Vf::madd(Vf::opmula(q, v), v, q.bc(Field::W));
        let t = Vf::opmsub(acc, v, q);
        let qv = q.mul(v);
        let acc = Vf::opmula(q, t);
        let acc = Vf::madd(acc, q, qv.bc(Field::X));
        let acc = Vf::madd(acc, q, qv.bc(Field::Y));
        let acc = Vf::madd(acc, q, qv.bc(Field::Z));
        let acc = Vf::madd(acc, t, q.bc(Field::W));
        v.merge(Dest::XYZ, Vf::opmsub(acc, t, q))
    }

    /// A world moment, brought into the body's frame and added.
    pub fn add_world_moment(&mut self, m: Vf) {
        let local = self.to_local(m);
        self.add_moment(local);
    }

    /// A body-frame force, taken into the world and added.
    pub fn add_local_force(&mut self, f: Vf) {
        let world = self.to_world(f);
        self.add_force(world);
    }

    /// The body's forward axis (its rotation's `z` row), `w` set to 1 as the stand-upright does.
    fn forward(&self) -> Vf {
        let (f, _) = quat_axes(self.orientation);
        Vf::new(f[0].0, f[1].0, f[2].0, 1.0)
    }
}

/// **The quad's stand-upright**: a quad rolling almost to a stop on a slope does not slide sideways
/// down it, and with the brake on does not slide at all.
///
/// The bike's does this off its front wheel's ground normal; the quad takes the mean of all four
/// wheels' normals (summed `((LF + RF) + LR) + RR` on VU0, times a quarter), touching or not. It runs
/// with the whole quad down, not on the balance words' agreement, off the throttle and below 10 ft/s
/// either way. The slide is gravity less its part into that ground; its part across the quad's forward
/// (flattened into the ground, renormalised, or [`NOTHING`] if that has no length) is cancelled by a
/// force, and its part along the quad is cancelled too when the brake is on.
///
/// The across force is scaled by the original's map of the speed from `0..10` onto `1..1` — always one
/// — negated.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn stand_upright(q: &Quad, body: &mut Body) {
    if !q.all_down || !q.balancing_differs() || q.gas != F::ZERO || !(q.speed.abs() < F(STAND_UPRIGHT_SPEED)) {
        return;
    }
    let w = &q.wheels;
    let sum = add_xyz(add_xyz(add_xyz(w[LEFT_FRONT].normal, w[RIGHT_FRONT].normal), w[LEFT_REAR].normal), w[RIGHT_REAR].normal);
    let n = scale(sum, F(0.25));
    let g = q.gravity;
    let slide = sub_xyz(g, scale(n, dot3(n, g)));

    let fwd = body.forward();
    let fwd = sub_xyz(fwd, scale(n, dot3(fwd, n)));
    let len2 = dot3(fwd, fwd);
    let fwd = if len2 == F::ZERO {
        NOTHING
    } else {
        let k = F::ONE.rsqrt(len2);
        Vf::new((F(fwd.x()) * k).0, (F(fwd.y()) * k).0, (F(fwd.z()) * k).0, fwd.w())
    };

    let along = scale(fwd, dot3(fwd, slide));
    let across = sub_xyz(slide, along);
    let mut force = scale(across, F(fpu::neg(unit_map(q.speed).0)));
    if q.brake != F::ZERO {
        force = sub_xyz(force, along);
    }
    body.add_force(force);
}

/// The original's `map_clamp(speed, 0, 10, 1, 1)` as it compiled it: a guard on `0 == 10` that never
/// fires, the divide, then `1 + t·(1 − 1)` through the accumulator, clamped to `[1, 1]` with `max.s`
/// and `min.s`.
fn unit_map(speed: F) -> F {
    let (lo, hi, one) = (F::ZERO, F(STAND_UPRIGHT_SPEED), F::ONE);
    let t = (speed - lo) / (hi - lo);
    let v = Acc::adda(F::ZERO, one).madd(t, one - one);
    v.max(one.min(one)).min(one.max(one))
}

impl Quad {
    fn balancing_differs(&self) -> bool {
        !self.balancing
    }

    /// The front wheel the stoppie reads (the left if it touches, else the right), and the rear wheel
    /// the wheelie reads, the same way.
    fn front_normal(&self) -> Vf {
        if self.wheels[LEFT_FRONT].touching { self.wheels[LEFT_FRONT].normal } else { self.wheels[RIGHT_FRONT].normal }
    }
    fn rear_normal(&self) -> Vf {
        if self.wheels[LEFT_REAR].touching { self.wheels[LEFT_REAR].normal } else { self.wheels[RIGHT_REAR].normal }
    }

    fn steer_fronts(&mut self, on: bool) {
        self.wheels[LEFT_FRONT].steered = on;
        self.wheels[RIGHT_FRONT].steered = on;
    }
}

/// **The quad's wheelie and stoppie** (its roll/pitch step): pitch the quad to where the stick asks
/// while it is up on one pair of wheels.
///
/// Off the ground nothing runs and the stoppie ends.
///
/// - **Wheelie**: a rear down, on the (rate-limited) throttle, above 5 ft/s, no rear brake, no clutch,
///   the stick at least half back. The target pitch is `(2·lean − 1)·MaxPitchCommand`; the error
///   `gain·(pitch − target) − pitch rate` integrates at 500/s, and the moment `(integral +
///   rate gain·error)` is faded out towards the top speed (`1 − speed/top`, clamped to `[0, 1]`) and
///   pushed about the quad's own `x`. A yaw term about a rear wheel's normal is built and scaled by a
///   literal zero (the bike's stoppie has the same term live), and still goes through the body's
///   world-moment call. Both fronts stop steering; with no wheelie they steer and the integral clears.
/// - **Stoppie**: latched with a front down, the front brake on, above 15 ft/s and the stick fully
///   forward. The target is the bike's, faded in by speed over `0..StoppieTopSpeed` (zero when that is
///   zero); the same integral and moment, then a yaw moment `500·(StoppieYawRateScale·lean − ω·n)`
///   about a front wheel's normal `n`, in the world. Both fronts stop steering. It unlatches with a
///   rear down, the front brake off, the stick back past −0.1 or at 7 ft/s or less. With no stoppie
///   on, the fronts steer and the integral clears.
///
/// The original keeps the integral through the FPU's accumulator: `0 + integral`, then `+ dt·(500·e)`.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn roll_pitch(p: &Params, q: &mut Quad, body: &mut Body, dt: F) {
    if !q.on_ground {
        q.stoppie = false;
        return;
    }
    let lean = q.lean_fore_aft;

    let wheelie = q.rear_down
        && !(q.gas_limited <= F::ZERO)
        && !(q.speed <= F(WHEELIE_SPEED))
        && q.rear_brake == F::ZERO
        && q.clutch_in == F::ZERO
        && !(lean < F(WHEELIE_LEAN));
    if wheelie {
        let target = F(2.0) * lean * p.max_pitch_command - p.max_pitch_command;
        let err = p.pitch_gain * (q.pitch - target) - F(body.ang_vel.x());
        q.pitch_integral = Acc::adda(F::ZERO, q.pitch_integral).madd(dt, F(PITCH_GAIN_RATE) * err);
        let fade = (F::ONE - q.speed * p.inv_wheelie_top_speed).min(F::ONE).max(F::ZERO);
        let tx = fade * Acc::adda(F::ZERO, q.pitch_integral).madd(p.pitch_rate_gain, err);
        body.add_moment(Vf::new(tx.0, 0.0, 0.0, 0.0));
        // The yaw rate about the rear normal is measured and thrown away: the scale is a literal zero.
        let n = q.rear_normal();
        let _dead = dot3(q.world_ang_vel, n);
        body.add_world_moment(scale(n, F::ZERO));
        q.steer_fronts(false);
    } else {
        q.steer_fronts(true);
        q.pitch_integral = F::ZERO;
    }

    if q.front_down
        && !q.stoppie
        && !(q.front_brake <= F::ZERO)
        && !(q.speed <= F(STOPPIE_SPEED))
        && lean <= F(STOPPIE_LEAN)
    {
        q.stoppie = true;
    }
    if !q.stoppie {
        q.steer_fronts(true);
        q.pitch_integral = F::ZERO;
        return;
    }

    let fade = if F::ZERO == p.stoppie_top_speed {
        F::ZERO
    } else {
        let t = (q.speed - F::ZERO) / (p.stoppie_top_speed - F::ZERO);
        let v = Acc::adda(F::ZERO, F::ZERO).madd(t, F::ONE - F::ZERO);
        v.max(F::ZERO.min(F::ONE)).min(F::ZERO.max(F::ONE))
    };
    let target = fade * Acc::adda(F::ZERO, p.max_stoppie_pitch_command).madd(F(2.0) * lean, p.max_stoppie_pitch_command);
    let err = p.stoppie_pitch_gain * (q.pitch - target) - F(body.ang_vel.x());
    q.pitch_integral = Acc::adda(F::ZERO, q.pitch_integral).madd(dt, F(PITCH_GAIN_RATE) * err);
    let tx = Acc::adda(F::ZERO, q.pitch_integral).madd(p.pitch_rate_gain, err);
    body.add_moment(Vf::new(tx.0, 0.0, 0.0, 0.0));

    let want = p.stoppie_yaw_rate_scale * q.lean;
    let n = q.front_normal();
    let yaw = F(PITCH_GAIN_RATE) * (want - dot3(q.world_ang_vel, n));
    body.add_world_moment(scale(n, yaw));
    q.steer_fronts(false);

    let end = q.rear_down || q.front_brake == F::ZERO || !(lean <= F(STOPPIE_END_LEAN)) || q.speed <= F(STOPPIE_END_SPEED);
    if end {
        q.stoppie = false;
    }
}

/// **The quad's ground push**: with a rear down and its gate open, the bike's own push runs — with its
/// flag set, a force `gain·(target − speed)` along the quad's own `z`, taken into the world. On
/// the bike it is the same push under the bike's guard.
pub fn ground_pitch_torque(q: &Quad, body: &mut Body) {
    if !q.rear_down || q.push_gate == F::ZERO {
        return;
    }
    if !q.push {
        return;
    }
    let z = q.push_gain * (q.push_target - q.speed);
    body.add_local_force(NOTHING.merge(Dest::Z, Vf::new(0.0, 0.0, z.0, 0.0)));
}

/// **The rear-pair flag**, set on both rear wheels after the tire forces: a front down (either) and
/// both rears up — the quad on its front wheels. Cleared on both otherwise.
pub fn rear_pair_flag(q: &mut Quad) {
    let w = &q.wheels;
    let on = (w[LEFT_FRONT].touching || w[RIGHT_FRONT].touching) && !w[LEFT_REAR].touching && !w[RIGHT_REAR].touching;
    q.wheels[LEFT_REAR].rear_pair = on;
    q.wheels[RIGHT_REAR].rear_pair = on;
}
