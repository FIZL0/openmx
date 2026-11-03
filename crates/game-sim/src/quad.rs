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
//! Around them, the quad's three physics-world listeners (MD64):
//!
//! - [`ground_head`], the PreProcess: the steering, the donut and the springs, each written to both
//!   wheels of the pair.
//! - [`ground_step`], the Force body: the order the pieces above and the bike's own steps run in, on
//!   four wheels, with [`wheel_contacts`] spreading the frame's contacts onto the four.
//! - [`air_state`], the PostProcess: the bike's contact step on four wheels, without its no-hander.
//!
//! And the quad's once-a-frame [`update`] (MD64a): the bike's vehicle update, then the quad's own
//! tail on four wheels (hazard surfaces, rumble, the wheelie and stoppie clocks, the ride stats).
//!
//! What they call of SLUS-21104's bike steps (engine, preload, backflip, air drag, airborne control,
//! the wheels' suspension, tire forces and integration, the input limiter, the gearbox and clutch) is
//! not ported here: each is a [`Call`] handed to a [`Moto`], in the original's order.
//!
//! The quad's [`put_back`] and [`net_unpack`] (MD65): the bike's put-back on four wheels, and a remote
//! quad set from its network snapshot. The PC slots it had not paired are MD65b's.
//!
//! The quad's wreck body (MD65a): the init's last block, [`build_wreck_body`], makes a ray probe off
//! `WreckATVm.col` that the quad's other collidables ignore, and the wreck entry and the unwreck
//! [`swap_rays`] the quad's collision between it and the riding rays.
//!
//! The quad's small slots (MD65b): the level setup's [`remap_surfaces`], the [`surface_under`] and
//! [`strut_travel`] answers, the [`trigger_segments`] the trigger probe is shaped to, and the
//! [`settle_wheels`] onto the ground.
//!
//! And the quad's tire reader, [`tire_phy`] (MD64b): `[Front Tire]` and `[Rear Tire]` into the four
//! wheels ([`TireSetup`]), with the original's left-to-right copies and their quirks.
//!
//! The quad is described by what the steps read, [`Quad`] and [`Body`], not by its layout: the field
//! map is research/oracle.txt's (`# MD37`, `# MD37a`, `# MD37b`, `# MD64b`, `# MD65`).

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
    /// The velocity at its contact patch, in the quad's frame (the PostProcess reads it).
    pub contact_vel: Vf,
    /// The fronts: held up for the balance, the steering lock and the steering command.
    pub balancing: bool,
    pub max_steer_angle: F,
    pub steer_command: F,
    /// The rears: how much donut the stick asks for.
    pub donut: F,
    /// The spring rate for this speed, the pair's own stopped/moving map.
    pub spring: F,
    /// One over the radius: the left rear's is what the automatic gearbox reads.
    pub inv_radius: F,
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
    /// The velocity at the contact patch, in the quad's frame: the first touching wheel's, else the
    /// body's own. Its `z` is the signed speed along the quad ([`Quad::speed`]).
    pub contact_vel: Vf,
    /// The brake (Square) and the throttle (Cross), and the throttle after its rate limit.
    pub brake: F,
    pub gas: F,
    pub gas_limited: F,
    /// The chassis' euler angles, pitch first ([`Quad::pitch`]).
    pub euler: Vf,
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

    /// The steps' shared guard: the quad has a vehicle state, and it is not one of the two (3 and 4)
    /// in which the PreProcess, Force and PostProcess all return at once.
    pub active: bool,
    /// Set, the Force body skips the quad's own controllers and the bike's (it still runs the four
    /// wheels), and the PreProcess zeroes the steering instead of commanding it.
    pub control_off: bool,
    /// The body's speed, the length of its velocity (the signed one is [`Quad::speed`]).
    pub speed_len: F,
    /// The steering command the PreProcess leaves for the bike's steps, the same as both fronts get.
    pub steer_command: F,
    /// How long the stick has been held at full lock, for [`Params::full_lock`].
    pub full_lock_timer: F,
    /// The stick's angle and how far it is deflected (the donut reads them).
    pub stick_angle: F,
    pub deflection: F,
    /// The peak-rates vector the Force body's last call holds maxima in, reset every PreProcess.
    pub attitude: Vf,

    /// The PostProcess's: the body's velocity in the quad's frame; the quad's forward and up axes
    /// (only `xyz` written); whether it was and is in the air, and for how long; the crossup angle.
    pub local_vel: Vf,
    pub heading: Vf,
    pub up: Vf,
    pub was_airborne: bool,
    pub airborne: bool,
    pub hang_time: F,
    pub crossup: F,
    /// The backflip's and the 360's "rotating" flags: either stops the crossup being measured.
    pub flipping: bool,
    pub spinning: bool,
    /// Control authority after a flip, and the time it ramps back over: the crossup ramps with it.
    pub authority: F,
    pub authority_time: F,
    /// The collision record's flag the PostProcess sets when the quad's up axis is at or below a
    /// quarter (on its side or over).
    pub tipped: bool,
    /// Where the quad's scene node is, as the node asked it, and the copy kept beside it.
    pub node_pos: Vf,
    pub node_pos_kept: Vf,
    /// What the scene node answers the net unpack's [`Call::NodeLocal`] with: the body's quadword
    /// at `+0xb0` brought into the node's frame (what that quadword is was not traced).
    pub node_local: Vf,
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
    /// The steering's speed fade: `steer_scale_1` up to `steer_speed_1`, `steer_scale_2` from
    /// `steer_speed_2` on, read against [`Quad::speed`].
    pub steer_speed_1: F,
    pub steer_scale_1: F,
    pub steer_speed_2: F,
    pub steer_scale_2: F,
    /// The full-lock option: with the stick held hard over, `full_lock_angle` of the lock ramps in over
    /// `full_lock_time` and the speed fade only scales the rest.
    pub full_lock: bool,
    pub full_lock_angle: F,
    pub full_lock_time: F,
    /// The donut: under this speed, the two stick angles its ramps end at.
    pub max_donut_speed: F,
    pub donut_angle: F,
    pub donut_angle_back: F,
    /// Each pair's spring rate stopped and moving.
    pub front_spring: (F, F),
    pub rear_spring: (F, F),
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
    /// The velocity in the world.
    pub velocity: Vf,
    /// A quadword of the body's the PreProcess resets to (0, 0, 0, 1) every frame; what reads it is
    /// not traced.
    pub reset_each_frame: Vf,
    /// The centre of mass in the world, and the correction a bottomed-out wheel takes its closing
    /// speed out of (MD75a: only the wheels' steps, run by [`Steps`], read and write them).
    pub position: Vf,
    pub correction: Vf,
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
    if !q.all_down || !q.balancing_differs() || q.gas != F::ZERO || !(q.speed().abs() < F(STAND_UPRIGHT_SPEED)) {
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
    let mut force = scale(across, F(fpu::neg(unit_map(q.speed()).0)));
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
    /// The signed speed along the quad, ft/s: the contact velocity's `z`.
    pub fn speed(&self) -> F {
        F(self.contact_vel.z())
    }

    /// The chassis' euler pitch.
    pub fn pitch(&self) -> F {
        F(self.euler.x())
    }

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
        && !(q.speed() <= F(WHEELIE_SPEED))
        && q.rear_brake == F::ZERO
        && q.clutch_in == F::ZERO
        && !(lean < F(WHEELIE_LEAN));
    if wheelie {
        let target = F(2.0) * lean * p.max_pitch_command - p.max_pitch_command;
        let err = p.pitch_gain * (q.pitch() - target) - F(body.ang_vel.x());
        q.pitch_integral = Acc::adda(F::ZERO, q.pitch_integral).madd(dt, F(PITCH_GAIN_RATE) * err);
        let fade = (F::ONE - q.speed() * p.inv_wheelie_top_speed).min(F::ONE).max(F::ZERO);
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
        && !(q.speed() <= F(STOPPIE_SPEED))
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
        let t = (q.speed() - F::ZERO) / (p.stoppie_top_speed - F::ZERO);
        let v = Acc::adda(F::ZERO, F::ZERO).madd(t, F::ONE - F::ZERO);
        v.max(F::ZERO.min(F::ONE)).min(F::ZERO.max(F::ONE))
    };
    let target = fade * Acc::adda(F::ZERO, p.max_stoppie_pitch_command).madd(F(2.0) * lean, p.max_stoppie_pitch_command);
    let err = p.stoppie_pitch_gain * (q.pitch() - target) - F(body.ang_vel.x());
    q.pitch_integral = Acc::adda(F::ZERO, q.pitch_integral).madd(dt, F(PITCH_GAIN_RATE) * err);
    let tx = Acc::adda(F::ZERO, q.pitch_integral).madd(p.pitch_rate_gain, err);
    body.add_moment(Vf::new(tx.0, 0.0, 0.0, 0.0));

    let want = p.stoppie_yaw_rate_scale * q.lean;
    let n = q.front_normal();
    let yaw = F(PITCH_GAIN_RATE) * (want - dot3(q.world_ang_vel, n));
    body.add_world_moment(scale(n, yaw));
    q.steer_fronts(false);

    let end = q.rear_down || q.front_brake == F::ZERO || !(lean <= F(STOPPIE_END_LEAN)) || q.speed() <= F(STOPPIE_END_SPEED);
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
    let z = q.push_gain * (q.push_target - q.speed());
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

// --- the three listeners: PreProcess, Force, PostProcess --------------------------------------------

/// The stick counts as held at full lock from here (either way).
const FULL_LOCK: f32 = 0.987;
/// The donut needs the stick deflected past this.
const DONUT_DEFLECTION: f32 = 0.9;
/// The springs' speed map, stopped to moving (ft/s).
const SPRING_STOPPED: f32 = 14.6666;
const SPRING_MOVING: f32 = 29.3332;
const HALF_PI: f32 = 1.5707964;
const PI: f32 = 3.1415927;
/// The crossup is only measured above this speed.
const CROSSUP_SPEED: f32 = 10.0;
/// At or below this, the up axis's `y` says the quad is on its side.
const TIPPED_UP: f32 = 0.25;

/// What the quad's three listeners call of SLUS-21104's bike and vehicle steps, and this module does
/// not port, in the order the original calls them. `F`s are the `dt` handed over; `usize`s the wheel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// The PreProcess's: the bike's input limiter (first), its automatic gearbox (handed the left
    /// rear's one-over-radius), its clutch, and the virtual it ends on.
    InputLimiter(F),
    AutoShift { dt: F, inv_radius: F },
    Clutch(F),
    HeadTail,
    /// The Force body's: the vehicle's gravity into the body (first, always), then — with control on —
    /// the bike's preload, backflip, air spin, the engine, the air drag and the airborne control.
    Gravity,
    Preload(F),
    Backflip(F),
    SpinAir(F),
    EngineUpdate(F),
    AirDrag(F),
    AirborneControl(F),
    /// The four wheels, in the Force body's rounds: each cleared, the contacts spread
    /// ([`wheel_contacts`]), each suspension, each tire's forces, then (after [`rear_pair_flag`]) each
    /// integrated.
    WheelClear(usize),
    Contacts,
    Suspension(usize, F),
    TireForces(usize, F),
    WheelIntegrate(usize, F),
    /// The engine's rev limiter and the peak-rates hold, the Force body's last two.
    RevLimit,
    PeakRates,
    /// The PostProcess's: the vehicle's own post step, then the scene node asked where it is (into
    /// [`Quad::node_pos`]).
    VehiclePost(F),
    NodePosition,
    /// The update's ([`update`]): the bike's own vehicle update (first); the vehicle's hazard-surface
    /// clock, handed the longest of the four wheels' surface times and whether a wheel is sunk in one;
    /// the pad's rumble (`soft` the gentler of the two, `strength` 0..255, for 0.2 s); the rider's
    /// wheelie or stoppie stunt started (the stunt id) and ended; the player's countdown run out, then
    /// each of the player's listeners and each of the race's watchers told so; and the message posted
    /// last (its id).
    BikeVehicleUpdate(F),
    HazardSurface { dt: F, time: F, sunk: bool },
    Rumble { soft: bool, strength: i32 },
    StartStunt(u32),
    EndStunt,
    CountdownOut,
    CountdownListener(u32),
    CountdownWatcher(u32),
    Post(u32),
    /// The put-back's ([`put_back`], MD65): each wheel's contact and suspension state reset; the
    /// quad's attachment taken off what it hangs on; the vehicle's own put-back (`place` and `keep`
    /// handed on as they came, `flags` the quad's flag byte as the vehicle's put-back sees it); the
    /// attachment hung back on; the two frame nodes asked for their world frames, and each hung node
    /// given one (`hung` its place in the list, `frame` 0 or 1); the network snapshot packed; the
    /// actor's listeners told it moved.
    WheelReset(usize),
    Detach,
    VehiclePutBack { place: bool, keep: bool, flags: u8 },
    Reattach,
    ReadFrame(usize),
    SetFrame { hung: usize, frame: usize },
    NetPack,
    ActorMoved,
    /// The net unpack's ([`net_unpack`], MD65): the scene node turned to the snapshot's orientation;
    /// the node asked for a quadword of the body's in its own frame (into [`Quad::node_local`]); and,
    /// after the wheel rounds, the vehicle's own end of the unpack.
    NodeOrientation,
    NodeLocal,
    NetPost(F),
    /// What the bike steps [`Steps`] runs call in turn (MD75a): the preload's cue posted (category 8,
    /// id 14, the bike's `+0xd20` handed on); the bike's wreck entered (the reason) when a backflip
    /// lands mid-clip — the stunt's start and end are [`Call::StartStunt`] and [`Call::EndStunt`];
    /// and a wheel's surface drag and spin, which its integrate runs before the steering hinge.
    PreloadCue,
    Wreck(u8),
    SurfaceDrag(usize, F),
    SpinIntegrate(usize, F),
    /// What the gravity [`Steps`] runs calls in turn (MD75b): the collision record's virtual, handed 1.
    GravityRecord,
    /// The wreck body's ([`build_wreck_body`], [`swap_rays`], MD65a): a ray-probe part made; its flag
    /// byte set from a flags word; it hung in the quad's scene tree; the wreck rays loaded from
    /// [`WRECK_FILE`] on the quad's scene node, then readied; a part shown in or hidden from the scene,
    /// taken off the collision world it is in, or put in the quad's world.
    PartMade(Part),
    PartFlags(Part, u32),
    PartHung(Part),
    PartLoad(Part),
    PartReady(Part),
    PartHide(Part),
    PartShow(Part),
    PartOffWorld(Part),
    PartInWorld(Part),
    /// The quad's level setup's ([`remap_surfaces`], MD65b): a global flag set, then the bike's shared
    /// level setup, handed the level.
    LevelSetup,
}

/// Whoever runs the calls the quad's listeners make. A `Vec<Call>` just lists them.
pub trait Moto {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body);
}

impl Moto for Vec<Call> {
    fn call(&mut self, c: Call, _q: &mut Quad, _body: &mut Body) {
        self.push(c);
    }
}

/// The original's inlined map and clamp: `x` from `a..b` onto `out0..out1`, clamped between the two
/// outputs whichever way round they are. Callers test `a == b` themselves.
fn map_clamp(x: F, a: F, b: F, out0: F, out1: F) -> F {
    crate::air::map_clamp(x, a, b, out0, out1)
}

/// **The quad's PreProcess**: the bike's ground head with every wheel write made to both wheels of
/// the pair.
///
/// The input limiter first; off the ground the front brake is let go. With the balance words equal
/// both fronts are held up; otherwise they are not, and unless control is off they get the steering
/// command — the stick times the steering lock times the speed fade (with the full-lock option, the
/// full-lock angle ramped in over its time while the stick is hard over, and the fade on the rest of
/// the lock only); with control off both fronts' commands are zeroed.
///
/// The donut goes on both rears: all four down, under the donut speed, the stick deflected past 0.9
/// and the throttle open, then the bike's two ramps on the stick angle either side of −π/2. Each pair
/// gets its spring rate off the speed (14.6666 to 29.3332 ft/s), the peak-rates vector and a body
/// quadword are reset, and the gearbox, clutch and the ending virtual are called.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn ground_head(p: &Params, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    if !q.active {
        return;
    }
    m.call(Call::InputLimiter(dt), q, body);
    if !q.on_ground {
        q.front_brake = F::ZERO;
    }
    if q.balancing {
        q.wheels[LEFT_FRONT].balancing = true;
        q.wheels[RIGHT_FRONT].balancing = true;
    } else {
        q.wheels[LEFT_FRONT].balancing = false;
        q.wheels[RIGHT_FRONT].balancing = false;
        if q.control_off {
            q.wheels[LEFT_FRONT].steer_command = F::ZERO;
            q.wheels[RIGHT_FRONT].steer_command = F::ZERO;
        } else {
            let fade = if p.steer_speed_1 == p.steer_speed_2 {
                p.steer_scale_1
            } else {
                map_clamp(q.speed(), p.steer_speed_1, p.steer_speed_2, p.steer_scale_1, p.steer_scale_2)
            };
            let lock = q.wheels[LEFT_FRONT].max_steer_angle;
            let command = if !p.full_lock {
                lock * (q.lean * fade)
            } else {
                q.full_lock_timer = if q.lean.abs() < F(FULL_LOCK) { F::ZERO } else { q.full_lock_timer + dt };
                let ramp = F::ONE.min(q.full_lock_timer / p.full_lock_time);
                Acc::mula(q.lean, p.full_lock_angle * ramp).madd(q.lean * fade, lock - p.full_lock_angle)
            };
            q.wheels[RIGHT_FRONT].steer_command = command;
            q.wheels[LEFT_FRONT].steer_command = command;
            q.steer_command = command;
        }
    }

    let donut = if !q.all_down
        || !(q.speed_len < p.max_donut_speed)
        || q.deflection <= F(DONUT_DEFLECTION)
        || q.gas == F::ZERO
    {
        F::ZERO
    } else {
        donut(p, q.stick_angle)
    };
    q.wheels[LEFT_REAR].donut = donut;
    q.wheels[RIGHT_REAR].donut = donut;

    let (stopped, moving) = (F(SPRING_STOPPED), F(SPRING_MOVING));
    let front = map_clamp(q.speed_len, stopped, moving, p.front_spring.0, p.front_spring.1);
    let rear = map_clamp(q.speed_len, stopped, moving, p.rear_spring.0, p.rear_spring.1);
    q.wheels[LEFT_FRONT].spring = front;
    q.wheels[LEFT_REAR].spring = rear;
    q.wheels[RIGHT_FRONT].spring = front;
    q.wheels[RIGHT_REAR].spring = rear;

    q.attitude = NOTHING;
    body.reset_each_frame = NOTHING;
    let inv_radius = q.wheels[LEFT_REAR].inv_radius;
    m.call(Call::AutoShift { dt, inv_radius }, q, body);
    m.call(Call::Clutch(dt), q, body);
    m.call(Call::HeadTail, q, body);
}

/// The donut's two ramps: nothing at straight ahead up to all of it at the donut angle, all of it
/// to −π/2, and back down past the back angle to nothing at −π. An empty ramp gives the end it
/// would have reached.
fn donut(p: &Params, a: F) -> F {
    let (half, pi) = (F(-HALF_PI), F(-PI));
    if a < half {
        let d = p.donut_angle_back;
        if a < d {
            if d == pi { F::ONE } else { map_clamp(a, d, pi, F::ONE, F::ZERO) }
        } else if half == d {
            F::ZERO
        } else {
            map_clamp(a, half, d, F::ZERO, F::ONE)
        }
    } else {
        let d = p.donut_angle;
        if a < d {
            if d == half { F::ONE } else { map_clamp(a, d, half, F::ONE, F::ZERO) }
        } else if F::ZERO == d {
            F::ZERO
        } else {
            map_clamp(a, F::ZERO, d, F::ZERO, F::ONE)
        }
    }
}

/// **The quad's Force body**: the bike's ground substep on four wheels.
///
/// Gravity first. With control on: [`stand_upright`] and [`roll_pitch`] in place of the bike's
/// stand-upright, balance and roll/pitch, then the bike's preload, backflip, air spin, engine, air
/// drag and airborne control, then [`ground_pitch_torque`]. Then, control or not: the four wheels
/// cleared, the contacts spread ([`wheel_contacts`]), the four suspensions, the four tire forces,
/// [`rear_pair_flag`], the four integrations, the rev limiter and the peak rates. Each round goes left
/// front, right front, left rear, right rear.
pub fn ground_step(p: &Params, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    if !q.active {
        return;
    }
    m.call(Call::Gravity, q, body);
    if !q.control_off {
        stand_upright(q, body);
        roll_pitch(p, q, body, dt);
        for c in [Call::Preload(dt), Call::Backflip(dt), Call::SpinAir(dt), Call::EngineUpdate(dt), Call::AirDrag(dt), Call::AirborneControl(dt)] {
            m.call(c, q, body);
        }
        ground_pitch_torque(q, body);
    }
    for w in 0..4 {
        m.call(Call::WheelClear(w), q, body);
    }
    m.call(Call::Contacts, q, body);
    for w in 0..4 {
        m.call(Call::Suspension(w, dt), q, body);
    }
    for w in 0..4 {
        m.call(Call::TireForces(w, dt), q, body);
    }
    rear_pair_flag(q);
    for w in 0..4 {
        m.call(Call::WheelIntegrate(w, dt), q, body);
    }
    m.call(Call::RevLimit, q, body);
    m.call(Call::PeakRates, q, body);
}

/// One contact of the frame's collision record, as the quad's distributor reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// Where the contact is along and across the quad: `z` above zero is a front wheel's, `x` above
    /// zero a right wheel's.
    pub at: Vf,
    pub normal: Vf,
    pub point: Vf,
    /// The collision node hit; the body it belongs to (zero for the world) and that body's
    /// touch-listener count; the vehicle it belongs to and that vehicle's kind (only kind 2 counts).
    pub node: u32,
    pub owner: u32,
    pub listeners: i32,
    pub vehicle: u32,
    pub vehicle_kind: i32,
}

/// **The quad's contact distributor**, the bike's on four wheels: the record cleared and swept;
/// then, if the sweep found anything to look at, each contact's owner's listeners told it was
/// touched, and the contact set on the wheel its place picks — `z > 0` a front, else a rear; `x > 0`
/// the right, else the left. The last rear contact's node is kept (`rear_node`; zero when none).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn wheel_contacts(
    wheels: &mut [crate::chassis::WheelContact; 4],
    rear_node: &mut u32,
    look: bool,
    cs: &[Contact],
) -> Vec<crate::chassis::ContactCall> {
    use crate::chassis::{ContactCall, set_contact};
    *rear_node = 0;
    let mut calls = vec![ContactCall::ClearRecord, ContactCall::Collide];
    if !look {
        return calls;
    }
    for c in cs {
        if c.owner != 0 {
            for i in 0..c.listeners {
                calls.push(ContactCall::Notify(c.owner, i));
            }
        }
        let vehicle = if c.vehicle != 0 && c.vehicle_kind == 2 { c.vehicle } else { 0 };
        // `c.ole` against zero: anything not at or below it, NaN too, is the right or the front.
        let side = usize::from(!(F(c.at.x()) <= F::ZERO));
        let wheel = if !(F(c.at.z()) <= F::ZERO) { LEFT_FRONT + side } else { LEFT_REAR + side };
        let at = crate::chassis::Contact { normal: c.normal, point: c.point, owner: c.owner, ..Default::default() };
        set_contact(&mut wheels[wheel], &at, vehicle);
        if wheel >= LEFT_REAR {
            *rear_node = c.node;
        }
    }
    calls
}

/// **The quad's PostProcess**: the bike's air state on four wheels, with no no-hander.
///
/// The speed (the velocity's length), the velocity into the quad's frame, the forward and up axes and
/// the euler angles off the orientation, the angular velocity into the world. On the ground with any
/// wheel down, all down with all four, a front or a rear down with either of the pair; the hang time
/// restarts on leaving the ground and counts while off it; the contact velocity is the first touching
/// wheel's (left front, right front, left rear, right rear), else the body's own. The crossup is the
/// bike's — the flattened heading against the flattened velocity, through the arc-cosine table — when
/// neither flipping nor spinning and above 10 ft/s, ramped with the authority after a flip. The
/// collision record is told the quad is tipped when its up axis's `y` is at or below 0.25. Then the
/// vehicle's post step and the scene node's position, kept in a second copy.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn air_state(q: &mut Quad, body: &mut Body, dt: F, table: &crate::air::AcosTable, m: &mut impl Moto) {
    use crate::air::{cross, normalize};
    if !q.active {
        return;
    }
    let v = body.velocity;
    q.speed_len = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(v.x(), v.x()), v.y(), v.y()), v.z(), v.z())));
    q.local_vel = body.to_local(v);
    let (fwd, up) = quat_axes(body.orientation);
    q.heading = Vf::new(fwd[0].0, fwd[1].0, fwd[2].0, q.heading.w());
    q.up = Vf::new(up[0].0, up[1].0, up[2].0, q.up.w());
    q.euler = crate::math::quat_to_euler(body.orientation, table);
    q.world_ang_vel = body.to_world(body.ang_vel);

    q.was_airborne = q.airborne;
    let t = q.wheels.map(|w| w.touching);
    q.on_ground = t.iter().any(|&t| t);
    q.all_down = t.iter().all(|&t| t);
    q.front_down = t[LEFT_FRONT] || t[RIGHT_FRONT];
    q.rear_down = t[LEFT_REAR] || t[RIGHT_REAR];
    q.airborne = !q.on_ground;
    if !q.was_airborne && q.airborne {
        q.hang_time = F::ZERO;
    }
    if q.airborne {
        q.hang_time = q.hang_time + dt;
    }
    q.contact_vel = match q.wheels.iter().find(|w| w.touching) {
        Some(w) => w.contact_vel,
        None => q.local_vel,
    };

    let heading = normalize(Vf::new(q.heading.x(), 0.0, q.heading.z(), 1.0));
    let mut angle = F::ZERO;
    if !q.flipping && !q.spinning && !(q.speed_len <= F(CROSSUP_SPEED)) {
        let vel = normalize(Vf::new(v.x(), 0.0, v.z(), 1.0));
        let c = cross(heading, vel);
        let sin = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(c.x(), c.x()), c.y(), c.y()), c.z(), c.z())));
        let sin = clamp_unit(clamp_unit(clamp_unit(sin)));
        angle = F(HALF_PI) - table.at(sin);
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

    m.call(Call::VehiclePost(dt), q, body);
    m.call(Call::NodePosition, q, body);
    q.node_pos_kept = q.node_pos;
}

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

// --- MD64a: the update ------------------------------------------------------------------------------

/// The stick's rumble: above this speed, and only past the harder of two suspension squeezes.
const RUMBLE_SPEED: f32 = 10.0;
const RUMBLE_HARD: f32 = 0.95;
const RUMBLE_SOFT: f32 = 0.73;
/// The rumble's strength off the squeeze (× 255), and the least each kind gives.
const RUMBLE_HARD_SCALE: f32 = 0.45;
const RUMBLE_SOFT_SCALE: f32 = 0.5;
const RUMBLE_FULL: f32 = 255.0;
const RUMBLE_HARD_LEAST: i32 = 95;
const RUMBLE_SOFT_LEAST: i32 = 40;
/// A wheelie or a stoppie is a stunt after this long on the one pair (s).
const ONE_PAIR: f32 = 1.0;
/// Off the ground for longer than this, a wheelie or a stoppie is over (s).
const ONE_PAIR_HANG: f32 = 0.1;
/// The player flag that stops the ride clock.
const RIDE_CLOCK_STOPPED: u32 = 0x200;

/// One entry of the quad's surface hazard table, one per surface: whether riding on it is a hazard,
/// and how long the vehicle's clock lets the quad stay on it (−1: no time at all).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    pub hazard: bool,
    pub time: F,
}

/// The player the quad belongs to, as the update reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Player {
    /// Its flags; [`RIDE_CLOCK_STOPPED`] holds the ride clock.
    pub flags: u32,
    /// Its distance this frame, added to a running wheelie or stoppie's.
    pub step: F,
    /// A word whose fall to zero the update counts (guessed: not traced to a writer).
    pub word: u32,
    /// Its speed, which the quad keeps the top of.
    pub speed: F,
    /// A countdown the update runs while the race's clock is at or below zero.
    pub countdown: F,
    /// How many listeners it tells when the countdown runs out.
    pub listeners: u32,
}

/// The race the player is in, as the update reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Race {
    /// The countdown only runs while this is at or below zero (guessed: the race's start clock).
    pub clock: F,
    /// How many watchers it tells when the player's countdown runs out.
    pub watchers: u32,
}

/// What the quad's [`update`] reads and keeps beyond [`Quad`]. The field map is research/oracle.txt's
/// `# MD64a`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tail {
    /// The vehicle state's mode, or none (no state: only the bike's update runs). The rumble and the
    /// second countdown run in mode 1 alone.
    pub vehicle_state: Option<u32>,
    /// The rider's hold: set, the update does nothing at all.
    pub rider_held: bool,
    /// Below this speed (the contact velocity's `z`), a wheelie or a stoppie neither starts nor runs.
    pub stunt_speed: F,
    pub surfaces: [Surface; 16],
    /// Each wheel's surface (an index into `surfaces`), its sunk flag (guessed from use), and its
    /// suspension's squeeze.
    pub wheel_surface: [u8; 4],
    pub wheel_sunk: [bool; 4],
    pub wheel_squeeze: [F; 4],
    /// Some wheel is on a hazard surface.
    pub on_hazard: bool,
    /// A request word kept for one frame only: still set with its last-frame copy set, it is cleared.
    pub request: u32,
    pub request_was: u32,
    /// The pad to rumble, and whether rumble is on for it.
    pub pad: bool,
    pub rumble_on: bool,
    /// The wheelie (rear pair only) and stoppie (front pair only) clocks: the stunt starts once one
    /// passes [`ONE_PAIR`].
    pub wheelie_clock: F,
    pub stoppie_clock: F,
    pub wheelie_stunt: u32,
    pub stoppie_stunt: u32,
    /// The running wheelie's and stoppie's time and distance, and the bests of each.
    pub wheelie_time: F,
    pub stoppie_time: F,
    pub wheelie_dist: F,
    pub stoppie_dist: F,
    pub best_wheelie_time: F,
    pub best_stoppie_time: F,
    pub best_wheelie_dist: F,
    pub best_stoppie_dist: F,
    pub player: Option<Player>,
    pub race: Option<Race>,
    /// The ride stats: time ridden, top speed, distance forward, and how often the player's word fell
    /// to zero (with the word as last seen).
    pub ride_time: F,
    pub top_speed: F,
    pub distance: F,
    pub word_falls: u32,
    pub word_was: u32,
    /// The scene node's up and forward rows: the distance is measured along the forward row flattened,
    /// or the up row when the forward one points straight up or down.
    pub node_up: Vf,
    pub node_forward: Vf,
    /// A second countdown (mode 1 only): on, it counts down until below zero, then turns itself off.
    pub second_on: u32,
    pub second_countdown: F,
    /// A clock that only counts.
    pub clock: F,
    /// The message the update posts last.
    pub post: u32,
}

/// **The quad's update**, once a frame: the bike's vehicle update, then the quad's own tail.
///
/// With no vehicle state it is the bike's update alone, and with the rider held nothing runs. Else,
/// after the bike's update, in order:
/// - the hazard surfaces: on one when any wheel's surface is a hazard, sunk when also any wheel is
///   sunk; the vehicle's hazard clock is handed the longest of the four wheels' surface times;
/// - the one-frame request cleared once it has been seen;
/// - the rumble, above 10 ft/s in mode 1 and not wrecked: the four suspensions' biggest squeeze past
///   0.95 rumbles hard (0.45 × 255 × squeeze, 95 at the least), past 0.73 soft (0.5 × 255, 40);
/// - unless wrecked (when both clocks are zeroed): on the rear pair alone and not slow, the wheelie
///   clock runs, and past a second the wheelie stunt starts (when its time is not yet running) and its
///   time and distance run, keeping their bests; the same for the stoppie on the front pair alone;
///   down on all four, or slow on the ground, or in the air past 0.1 s, a running stunt is ended and
///   both clocks, times and distances are zeroed; anything else leaves them as they are;
/// - with a player: the ride clock (unless the player holds it), the top speed, the distance forward
///   (the speed with the front pair down, else the body's velocity along the node's flattened forward
///   row; never backward), the word's falls counted, and the player's countdown, which runs out by
///   telling the player's listeners and the race's watchers;
/// - the second countdown in mode 1, the clock, and the message posted.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn update(t: &mut Tail, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    let Some(mode) = t.vehicle_state else {
        m.call(Call::BikeVehicleUpdate(dt), q, body);
        return;
    };
    if t.rider_held {
        return;
    }
    let slow = q.speed() < t.stunt_speed;
    let (node_up, node_forward) = (t.node_up, t.node_forward);
    m.call(Call::BikeVehicleUpdate(dt), q, body);

    let surface = t.wheel_surface.map(|s| t.surfaces[usize::from(s)]);
    t.on_hazard = surface.iter().any(|s| s.hazard);
    let sunk = t.on_hazard && t.wheel_sunk.iter().any(|&s| s);
    let time = fpu_max(fpu_max(surface[RIGHT_REAR].time, surface[LEFT_REAR].time), surface[RIGHT_FRONT].time);
    let time = fpu_max(surface[LEFT_FRONT].time, time);
    m.call(Call::HazardSurface { dt, time, sunk }, q, body);

    if t.request != 0 && t.request_was != 0 {
        t.request = 0;
    }
    t.request_was = t.request;

    if !(q.speed_len <= F(RUMBLE_SPEED)) && mode == 1 && !q.control_off {
        let sq = t.wheel_squeeze;
        let squeeze = fpu_max(fpu_max(sq[LEFT_FRONT], sq[RIGHT_FRONT]), fpu_max(sq[LEFT_REAR], sq[RIGHT_REAR]));
        let (hard, strength) = if !(squeeze <= F(RUMBLE_HARD)) {
            (true, rumble_strength(squeeze, RUMBLE_HARD_SCALE, RUMBLE_HARD_LEAST))
        } else if !(squeeze <= F(RUMBLE_SOFT)) {
            (false, rumble_strength(squeeze, RUMBLE_SOFT_SCALE, RUMBLE_SOFT_LEAST))
        } else {
            (false, 0)
        };
        if strength > 0 && t.pad && t.rumble_on {
            m.call(Call::Rumble { soft: !hard, strength }, q, body);
        }
    }

    if q.control_off {
        t.wheelie_clock = F::ZERO;
        t.stoppie_clock = F::ZERO;
    } else if !q.front_down && q.rear_down && !slow {
        one_pair(t, q, body, dt, m, true);
    } else if q.front_down && !q.rear_down && !slow {
        one_pair(t, q, body, dt, m, false);
    } else if (q.front_down && q.rear_down) || (!q.airborne && slow) || (q.airborne && !(q.hang_time <= F(ONE_PAIR_HANG))) {
        if !(t.wheelie_time <= F::ZERO) || !(t.stoppie_time <= F::ZERO) {
            m.call(Call::EndStunt, q, body);
        }
        t.wheelie_clock = F::ZERO;
        t.stoppie_clock = F::ZERO;
        t.wheelie_time = F::ZERO;
        t.stoppie_time = F::ZERO;
        t.wheelie_dist = F::ZERO;
        t.stoppie_dist = F::ZERO;
    }

    if let Some(mut p) = t.player {
        if p.flags & RIDE_CLOCK_STOPPED == 0 {
            t.ride_time = t.ride_time + dt;
        }
        if !(p.speed <= t.top_speed) {
            t.top_speed = p.speed;
        }
        let mut forward = if q.front_down {
            q.speed()
        } else {
            let row = if node_forward.y() <= -1.0 || 1.0 <= node_forward.y() { node_up } else { node_forward };
            let dir = crate::air::normalize(Vf::new(row.x(), 0.0, row.z(), row.w()));
            let v = body.velocity;
            Acc::adda(F::ZERO, F(v.z()) * F(dir.z())).madd(v.x(), dir.x())
        };
        if forward < F::ZERO {
            forward = F::ZERO;
        }
        t.distance = Acc::adda(F::ZERO, t.distance).madd(forward, dt);
        if t.word_was != 0 && p.word == 0 {
            t.word_falls = t.word_falls.wrapping_add(1);
        }
        t.word_was = p.word;
        if let Some(race) = t.race.filter(|r| !(p.countdown <= F::ZERO) && r.clock <= F::ZERO) {
            p.countdown = p.countdown - dt;
            if p.countdown <= F::ZERO {
                m.call(Call::CountdownOut, q, body);
                for i in 0..p.listeners {
                    m.call(Call::CountdownListener(i), q, body);
                }
                for i in 0..race.watchers {
                    m.call(Call::CountdownWatcher(i), q, body);
                }
            }
        }
        t.player = Some(p);
    }

    if t.second_on != 0 && mode == 1 {
        if t.second_countdown < F::ZERO {
            t.second_on = 0;
        } else {
            t.second_countdown = t.second_countdown - dt;
        }
    }
    t.clock = t.clock + dt;
    m.call(Call::Post(t.post), q, body);
}

/// `max.s`.
fn fpu_max(a: F, b: F) -> F {
    a.max(b)
}

/// The rumble's strength: `255 × (scale × squeeze)` truncated, and `least` when that is under it.
fn rumble_strength(squeeze: F, scale: f32, least: i32) -> i32 {
    let s = (F(RUMBLE_FULL) * (F(scale) * squeeze)).to_i32();
    if s <= least { least } else { s }
}

/// A wheelie's (`wheelie`) or a stoppie's frame on the one pair: its clock runs, and past a second
/// the stunt starts if its time is not running yet, and its time and distance run with their bests.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn one_pair(t: &mut Tail, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto, wheelie: bool) {
    let step = t.race.and(t.player).map(|p| p.step);
    let (clock, stunt, time, best_time, dist, best_dist) = if wheelie {
        (&mut t.wheelie_clock, t.wheelie_stunt, &mut t.wheelie_time, &mut t.best_wheelie_time, &mut t.wheelie_dist, &mut t.best_wheelie_dist)
    } else {
        (&mut t.stoppie_clock, t.stoppie_stunt, &mut t.stoppie_time, &mut t.best_stoppie_time, &mut t.stoppie_dist, &mut t.best_stoppie_dist)
    };
    *clock = *clock + dt;
    if *clock <= F(ONE_PAIR) {
        return;
    }
    if *time <= F::ZERO {
        m.call(Call::StartStunt(stunt), q, body);
    }
    *time = *time + dt;
    if !(*time <= *best_time) {
        *best_time = *time;
    }
    if let Some(step) = step {
        *dist = *dist + step;
        if !(*dist <= *best_dist) {
            *best_dist = *dist;
        }
    }
}

// --- the tire reader (MD64b) ----------------------------------------------------------------------

/// One surface's row of a wheel's tables, in the three setters' argument order: the drive setter's
/// eight (`SideForce`, `SlippingSideForce`, `MaxDriveForce`, `MaxSideForce`, `MaxSlippingSideForce`,
/// `SkidDriveForce`, `SkidSideForce`, `SkidSlippingSideForce`), the side setter's thirteen
/// (`SlippingSideForce`, `BrakingSideForce` and `SideForce` `RoundOff`/`DropOff`/`Scale`/`SpeedScale`,
/// then `DonutsSideForceSpeedScale`) and the drag setter's three (`MaxBrakingTorque`, `WheelFriction`,
/// `WheelDrag`). The wheel keeps each as a table of sixteen surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireSurface {
    pub drive: [F; 8],
    pub side: [F; 13],
    pub drag: [F; 3],
}

impl TireSurface {
    fn of(s: &crate::phy::Surface) -> TireSurface {
        TireSurface {
            drive: [
                s.side_force,
                s.slipping_side_force,
                s.max_drive_force,
                s.max_side_force,
                s.max_slipping_side_force,
                s.skid_drive_force,
                s.skid_side_force,
                s.skid_slipping_side_force,
            ],
            side: [
                s.slipping_side_force_round_off,
                s.slipping_side_force_drop_off,
                s.slipping_side_force_scale,
                s.slipping_side_force_speed_scale,
                s.braking_side_force_round_off,
                s.braking_side_force_drop_off,
                s.braking_side_force_scale,
                s.braking_side_force_speed_scale,
                s.side_force_round_off,
                s.side_force_drop_off,
                s.side_force_scale,
                s.side_force_speed_scale,
                s.donuts_side_force_speed_scale,
            ],
            drag: [s.max_braking_torque, s.wheel_friction, s.wheel_drag],
        }
    }
}

/// What the tire reader sets on one of the four wheels (and the two things of the wheel it reads:
/// where the wheel is mounted, and whether it has a sweep shape). Whatever the reader does not reach
/// keeps what it had: the wheel's constructor's, or the last read's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelTire {
    /// `UseParametricCurve` as the ini's integer read gives it (any number, not only 0 or 1).
    pub parametric: i32,
    /// The suspension setter's: travel, the two rate limits, the spring rate (the file's rate times
    /// the quad's weight) and the two damping rates against it.
    pub strut_compression: F,
    pub max_compression_rate: F,
    pub max_decompression_rate: F,
    pub spring: F,
    pub damping_in: F,
    pub damping_out: F,
    /// The tire setter's: the spin inertia and its inverse (positive radius only), `TireInertia`, the
    /// radius, the two widths it is handed (24 and 6), the tire's mass, one over the radius (positive
    /// radius only) and a scale nothing reads.
    pub spin_inertia: F,
    pub inv_spin_inertia: F,
    pub tire_inertia: F,
    pub radius: F,
    pub ply: F,
    pub width: F,
    pub tire_mass: F,
    pub inv_radius: F,
    pub ply_scale: F,
    /// The steering setter's: damping, inertia and its inverse (a positive inertia only), torque.
    pub steer_damping: F,
    pub steer_inertia: F,
    pub inv_steer_inertia: F,
    pub steer_torque: F,
    /// The steering lock, radians, and its negative.
    pub max_steer_angle: F,
    pub min_steer_angle: F,
    /// `BodyTireForcePosition`, and the rear's `OtherTireNormalLoadScale`.
    pub force_position: Vf,
    pub other_load_scale: F,
    /// Where the wheel is mounted (read), and the derived setter's rest point: the mount, a radius
    /// lower.
    pub mount: Vf,
    pub rest: Vf,
    /// The wheel has a sweep shape (read): the derived setter rebuilds it.
    pub sweep: bool,
    pub surfaces: [TireSurface; crate::phy::MAX_SURFACES],
}

/// What the tire reader sets: the four wheels, each pair's spring rate stopped and moving (as
/// [`Params::front_spring`] and [`Params::rear_spring`]), and the front's mud per surface.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireSetup {
    pub wheels: [WheelTire; 4],
    pub front_spring: (F, F),
    pub rear_spring: (F, F),
    pub mud_time: [F; crate::phy::MAX_SURFACES],
    pub mud_amount: [F; crate::phy::MAX_SURFACES],
}

/// The suspension setter: the rates follow the spring it is handed, so the last call wins.
fn set_suspension(w: &mut WheelTire, t: &Keys, spring: F, mass: F) {
    w.strut_compression = t.strut;
    w.max_compression_rate = t.max_compression;
    w.max_decompression_rate = t.max_decompression;
    w.spring = crate::phy::spring_rate(spring, mass);
    w.damping_in = crate::phy::damping(t.bounce, mass, w.spring);
    w.damping_out = crate::phy::damping(t.rebound, mass, w.spring);
}

/// The tire setter, with the 6 and 24 the reader always hands it.
fn set_tire(w: &mut WheelTire, radius: F, inertia: F) {
    w.radius = radius;
    w.width = F(6.0);
    w.ply = F(24.0);
    w.tire_inertia = inertia;
    w.ply_scale = crate::tire::tire_ply_scale();
    w.tire_mass = crate::tire::tire_mass(radius);
    if let Some((inv_radius, spin, inv_spin)) = crate::tire::spin_inertia(radius) {
        w.inv_radius = inv_radius;
        w.spin_inertia = spin;
        w.inv_spin_inertia = inv_spin;
    }
}

/// The steering setter: damping, inertia (only a positive one), torque.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compare
fn set_steering(w: &mut WheelTire, damping: F, inertia: F, torque: F) {
    w.steer_damping = damping;
    w.steer_torque = torque;
    if F::ZERO < inertia {
        w.steer_inertia = inertia;
        w.inv_steer_inertia = F::ONE / inertia;
    }
}

/// The derived setter: the sweep shape (if any) rebuilt as the segment from the strut's top down to a
/// radius below the mount, and the rest point a radius under the mount.
fn set_derived(w: &mut WheelTire) -> Option<[Vf; 2]> {
    w.rest = w.mount;
    w.rest.0[1] = (F(w.mount.y()) - w.radius).0;
    w.sweep.then(|| [Vf::new(0.0, w.strut_compression.0, 0.0, 1.0), Vf::new(0.0, (-w.radius).0, 0.0, 1.0)])
}

/// One section's scalar keys, with the reader's defaults.
struct Keys {
    parametric: i32,
    strut: F,
    max_compression: F,
    max_decompression: F,
    radius: F,
    inertia: F,
    spring: F,
    spring_stopped: F,
    bounce: F,
    rebound: F,
}

impl Keys {
    fn read(s: crate::phy::Section<'_>) -> Keys {
        Keys {
            parametric: s.int("UseParametricCurve").unwrap_or(0),
            strut: s.float_or("StrutCompression", F::ONE),
            max_compression: s.float_or("MaxCompressionRate", F(20.0)),
            max_decompression: s.float_or("MaxDecompressionRate", F::ONE),
            radius: s.float_or("TireRadius", F::ONE),
            inertia: s.float_or("TireInertia", F::ONE),
            spring: s.float_or("Spring", F::ZERO),
            spring_stopped: s.float_or("SpringWhenStopped", F::ZERO),
            bounce: s.float_or("BounceDampingRatio", F::ZERO),
            rebound: s.float_or("ReboundDampingRatio", F::ZERO),
        }
    }
}

/// **The quad's tire reader**: `[Front Tire]` into the two front wheels and `[Rear Tire]` into the two
/// rear ones. `mass` is the quad's (its weight over 32.2), which the spring rates are scaled by. It
/// returns the segment each wheel's sweep shape was rebuilt to, for the wheels that have one.
///
/// The left wheel of each pair is read into and the right one copies it, with these quirks of the
/// original kept:
/// - the stopped spring is set first and read back, then the moving one, so the damping rates are the
///   moving spring's;
/// - `MaxSteerAngle` (degrees) is the front's only; the rears keep their own lock, the right rear
///   taking the left rear's;
/// - the left rear gets no steering at all, and the right rear gets **the front's** steering keys;
/// - the front's mud keys go to the quad, one pair per surface, and the front's donut scale is zero;
/// - a `[Rear Tire]` with no `NumberOfSurfaces` reads the front's count (the same stack word).
pub fn tire_phy(t: &mut TireSetup, ini: &crate::phy::Ini, mass: F) -> [Option<[Vf; 2]>; 4] {
    use crate::phy::{DEG, MAX_SURFACES, Surface};
    const UNIT_W: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
    let mut sweeps = [None; 4];

    let s = ini.section("Front Tire");
    let k = Keys::read(s);
    let (steer_torque, steer_damping, steer_inertia) = (
        s.float_or("SteeringTorque", F::ZERO),
        s.float_or("SteeringDamping", F::ZERO),
        s.float_or("SteeringInertia", F::ZERO),
    );
    let lf = &mut t.wheels[LEFT_FRONT];
    lf.parametric = k.parametric;
    lf.max_steer_angle = s.float_or("MaxSteerAngle", F::ZERO);
    lf.force_position = s.vector("BodyTireForcePosition", UNIT_W);
    set_suspension(lf, &k, k.spring_stopped, mass);
    let stopped = lf.spring;
    set_suspension(lf, &k, k.spring, mass);
    t.front_spring = (stopped, lf.spring);
    set_tire(lf, k.radius, k.inertia);
    set_steering(lf, steer_damping, steer_inertia, steer_torque);
    sweeps[LEFT_FRONT] = set_derived(lf);
    lf.max_steer_angle = lf.max_steer_angle * F(DEG);
    lf.min_steer_angle = -lf.max_steer_angle;
    let count = s.int("NumberOfSurfaces");
    for i in 1..=count.unwrap_or(0).min(MAX_SURFACES as i32) {
        let row = Surface::read(s, i, true);
        let j = (i - 1) as usize;
        t.mud_time[j] = row.mud_time;
        t.mud_amount[j] = row.mud_amount;
        for w in [LEFT_FRONT, RIGHT_FRONT] {
            t.wheels[w].surfaces[j] = TireSurface::of(&row);
        }
    }
    let lf = t.wheels[LEFT_FRONT];
    let rf = &mut t.wheels[RIGHT_FRONT];
    rf.parametric = lf.parametric;
    rf.force_position = lf.force_position;
    rf.max_steer_angle = lf.max_steer_angle;
    rf.min_steer_angle = lf.min_steer_angle;
    set_suspension(rf, &k, k.spring, mass);
    set_tire(rf, k.radius, k.inertia);
    set_steering(rf, steer_damping, steer_inertia, steer_torque);
    sweeps[RIGHT_FRONT] = set_derived(rf);

    let s = ini.section("Rear Tire");
    let k = Keys::read(s);
    let lr = &mut t.wheels[LEFT_REAR];
    lr.parametric = k.parametric;
    lr.force_position = s.vector("BodyTireForcePosition", UNIT_W);
    lr.other_load_scale = s.float_or("OtherTireNormalLoadScale", F::ONE);
    set_suspension(lr, &k, k.spring_stopped, mass);
    let stopped = lr.spring;
    set_suspension(lr, &k, k.spring, mass);
    t.rear_spring = (stopped, lr.spring);
    set_tire(lr, k.radius, k.inertia);
    sweeps[LEFT_REAR] = set_derived(lr);
    for i in 1..=s.int("NumberOfSurfaces").or(count).unwrap_or(0).min(MAX_SURFACES as i32) {
        let row = TireSurface::of(&Surface::read(s, i, false));
        for w in [LEFT_REAR, RIGHT_REAR] {
            t.wheels[w].surfaces[(i - 1) as usize] = row;
        }
    }
    let lr = t.wheels[LEFT_REAR];
    let rr = &mut t.wheels[RIGHT_REAR];
    rr.parametric = lr.parametric;
    rr.force_position = lr.force_position;
    rr.max_steer_angle = lr.max_steer_angle;
    rr.min_steer_angle = lr.min_steer_angle;
    rr.other_load_scale = lr.other_load_scale;
    set_suspension(rr, &k, k.spring, mass);
    set_tire(rr, k.radius, k.inertia);
    set_steering(rr, steer_damping, steer_inertia, steer_torque);
    sweeps[RIGHT_REAR] = set_derived(rr);
    sweeps
}

// --- MD65: the put-back and the net unpack ----------------------------------------------------------

/// What the put-back reads and writes beyond [`Quad`]. The field map is research/oracle.txt's `# MD65`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PutBack {
    /// The quad's flag byte: its [`HIDDEN_BIT`] is cleared across the vehicle's put-back and set
    /// again after, if it was set.
    pub flags: u8,
    /// The nodes hung on the quad, by which frame each takes: 0 the first frame node's, 1 the
    /// second's, anything else neither.
    pub hung: Vec<i32>,
    /// Where the player is put back to, as three words; the first one zero is none.
    pub respawn: [u32; 3],
    /// The player's own copy of it, written only with a player and a respawn point; `None`, no
    /// player.
    pub player_respawn: Option<[u32; 3]>,
    /// A word the put-back sets to a half (what reads it was not traced).
    pub settle: F,
    /// The update's one-frame request word ([`Tail::request`]), set by the put-back.
    pub request: u32,
}

/// The flag-byte bit the vehicle's put-back must not see set.
pub const HIDDEN_BIT: u8 = 0x2;
/// What the put-back leaves in [`PutBack::settle`].
const SETTLE: f32 = 0.5;

/// **The quad's put-back**: the bike's own put-back override, word for word, on four wheels.
///
/// The four wheels reset, the attachment taken off, then the vehicle's put-back (with the flag
/// byte's [`HIDDEN_BIT`] cleared for it, and set again after if it was set), the attachment hung back
/// on, the two frame nodes' world frames read and handed to each hung node by its kind. The quad is
/// then on the ground, not airborne, with no crossup, [`PutBack::settle`] a half and the request
/// word set; the network snapshot is packed, the player given the respawn point (when there is a
/// player and a point), and the actor's listeners told.
pub fn put_back(pb: &mut PutBack, q: &mut Quad, body: &mut Body, place: bool, keep: bool, m: &mut impl Moto) {
    for w in 0..4 {
        m.call(Call::WheelReset(w), q, body);
    }
    m.call(Call::Detach, q, body);
    let hidden = pb.flags & HIDDEN_BIT != 0;
    if hidden {
        pb.flags &= !HIDDEN_BIT;
    }
    m.call(Call::VehiclePutBack { place, keep, flags: pb.flags }, q, body);
    if hidden {
        pb.flags = pb.flags & !HIDDEN_BIT | HIDDEN_BIT;
    }
    m.call(Call::Reattach, q, body);
    m.call(Call::ReadFrame(0), q, body);
    m.call(Call::ReadFrame(1), q, body);
    for (i, &kind) in pb.hung.iter().enumerate() {
        if kind == 0 || kind == 1 {
            m.call(Call::SetFrame { hung: i, frame: kind as usize }, q, body);
        }
    }
    q.on_ground = true;
    q.airborne = false;
    pb.settle = F(SETTLE);
    q.crossup = F::ZERO;
    pb.request = 1;
    m.call(Call::NetPack, q, body);
    if pb.player_respawn.is_some() && pb.respawn[0] != 0 {
        pb.player_respawn = Some(pb.respawn);
    }
    m.call(Call::ActorMoved, q, body);
}

/// What the net unpack reads and writes beyond [`Quad`] and [`Body`]. The field map is
/// research/oracle.txt's `# MD65`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Remote {
    /// The network snapshot: where the quad is, its orientation and its velocity.
    pub position: Vf,
    pub orientation: Vf,
    pub velocity: Vf,
    /// The body's centre of mass in its own frame, and where the unpack puts it in the world.
    pub com_offset: Vf,
    pub com: Vf,
    /// The steering's lag filter: its value, its time constant and the fraction it last moved by.
    pub steer: F,
    pub steer_time: F,
    pub steer_fraction: F,
    /// The left front's negative steering lock (the tire reader's −MaxSteerAngle).
    pub min_steer_angle: F,
    /// Each wheel's spin angle (kept within ±π) and rate.
    pub spin: [F; 4],
    pub spin_rate: [F; 4],
}

/// The steering the snapshot's node quadword asks for, per unit of its `x`.
const NET_STEER: f32 = 0.5;
const ONE_OVER_TWO_PI: f32 = 0.15915494;
const TWO_PI: f32 = 6.2831855;

/// A spin angle back within ±π: less the whole turns in it (truncated), then one turn more either
/// way if it is still past π.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn wrap_angle(a: F) -> F {
    let turns = F(fpu::from_i32((F(ONE_OVER_TWO_PI) * a).to_i32()));
    let w = Acc::adda(F::ZERO, a).msub(F(TWO_PI), turns);
    if !(w <= F(PI)) {
        w - F(TWO_PI)
    } else if w < -F(PI) {
        F(TWO_PI) + w
    } else {
        w
    }
}

/// **The quad's net unpack**: a remote quad put where its network snapshot says, its wheels spun
/// and steered to match, and the four wheels run against the ground.
///
/// The body takes the snapshot's velocity and orientation, the node's position (and its kept copy)
/// the snapshot's position, and the centre of mass that position plus the body's offset turned into
/// the world. The body's own angular velocity is zeroed and the speed is the velocity's length. The
/// scene node is turned to the orientation; each wheel spins at the velocity's forward part (in the
/// quad's frame) over its radius, its angle integrated and wrapped to ±π. The steering is half the
/// node's answer's `x` within the left front's lock, lagged by the filter (the fraction `dt` over
/// its time, at most one) into the left front's command and the quad's, which the right front
/// copies. Then the Force body's wheel rounds without the integrate (clear, [`wheel_contacts`],
/// suspension, tire forces), the ground flags off the four wheels, and the vehicle's end of the
/// unpack.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn net_unpack(r: &mut Remote, q: &mut Quad, body: &mut Body, dt: F, m: &mut impl Moto) {
    body.velocity = r.velocity;
    body.orientation = r.orientation;
    q.node_pos_kept = r.position;
    r.com = add_xyz(r.position, body.to_world(r.com_offset));
    q.node_pos = q.node_pos_kept;
    body.ang_vel = NOTHING;
    let v = body.velocity;
    q.speed_len = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(v.x(), v.x()), v.y(), v.y()), v.z(), v.z())));
    m.call(Call::NodeOrientation, q, body);
    let forward = F(body.to_local(body.velocity).z());
    let spin = |r: &mut Remote, q: &Quad, w: usize| {
        r.spin_rate[w] = forward * q.wheels[w].inv_radius;
        r.spin[w] = wrap_angle(Acc::adda(F::ZERO, r.spin[w]).madd(r.spin_rate[w], dt));
    };
    spin(r, q, LEFT_FRONT);
    m.call(Call::NodeLocal, q, body);
    let target = (F(NET_STEER) * F(q.node_local.x())).max(r.min_steer_angle).min(q.wheels[LEFT_FRONT].max_steer_angle);
    r.steer_fraction = dt.min(r.steer_time) / r.steer_time;
    r.steer = Acc::adda(F::ZERO, r.steer).madd(r.steer_fraction, target - r.steer);
    q.wheels[LEFT_FRONT].steer_command = r.steer;
    q.steer_command = q.wheels[LEFT_FRONT].steer_command;
    spin(r, q, RIGHT_FRONT);
    q.wheels[RIGHT_FRONT].steer_command = q.steer_command;
    spin(r, q, LEFT_REAR);
    spin(r, q, RIGHT_REAR);
    for w in 0..4 {
        m.call(Call::WheelClear(w), q, body);
    }
    m.call(Call::Contacts, q, body);
    for w in 0..4 {
        m.call(Call::Suspension(w, dt), q, body);
    }
    for w in 0..4 {
        m.call(Call::TireForces(w, dt), q, body);
    }
    q.was_airborne = q.airborne;
    let t = q.wheels.map(|w| w.touching);
    q.on_ground = t.iter().any(|&b| b);
    q.all_down = t.iter().all(|&b| b);
    q.front_down = t[LEFT_FRONT] || t[RIGHT_FRONT];
    q.rear_down = t[LEFT_REAR] || t[RIGHT_REAR];
    q.airborne = !q.on_ground;
    m.call(Call::NetPost(dt), q, body);
}

// --- the physics frame (MD75) ---------------------------------------------------------------------

/// SLUS-21104's physics world as the quad's frame sees it: one rigid body, the quad's three
/// listeners, no constraints.
struct Frame<'a, M> {
    p: &'a Params,
    q: &'a mut Quad,
    body: &'a mut crate::atv_body::Body,
    table: &'a crate::air::AcosTable,
    m: &'a mut M,
}

impl<M: Moto> Frame<'_, M> {
    /// A listener on what it reads of the body, written back after.
    fn listen(&mut self, f: impl FnOnce(&Params, &mut Quad, &mut Body, &crate::air::AcosTable, &mut M)) {
        let mut b = self.body.view();
        f(self.p, self.q, &mut b, self.table, self.m);
        self.body.set_view(&b);
    }
}

impl<M: Moto> crate::biplane::Dynamics for Frame<'_, M> {
    fn count(&self, list: crate::biplane::List) -> i32 {
        i32::from(list != crate::biplane::List::Constraints)
    }
    fn preforce(&mut self, _: i32, dt: F) {
        self.listen(|p, q, b, _, m| ground_head(p, q, b, dt, m));
    }
    fn force(&mut self, _: i32, h: F) {
        self.listen(|p, q, b, _, m| ground_step(p, q, b, h, m));
    }
    fn enabled(&self, _: i32) -> bool {
        self.body.enabled
    }
    fn integrate(&mut self, _: i32, h: F) {
        self.body.integrate(h);
    }
    fn end_substep(&mut self, _: i32) {
        self.body.end_substep();
    }
    fn constrain(&mut self, _: i32, _: bool) {}
    fn post(&mut self, _: i32, dt: F) {
        self.listen(|_, q, b, table, m| air_state(q, b, dt, table, m));
    }
}

/// **The quad's physics frame**: SLUS-21104's world step over the quad alone. The world is
/// MX Unleashed's ([`crate::biplane::World::step`]; the game's has 1 iteration at 115 a second, so
/// four substeps at 30 frames a second): [`ground_head`] once with the frame's `dt`, then each
/// substep [`ground_step`], the body's [`crate::atv_body::Body::integrate`] and
/// [`crate::atv_body::Body::end_substep`] (while the body is enabled); then [`air_state`] with the
/// frame's `dt`.
///
/// In the game the world's lists hold the other riders' listeners too (the second quad's PreProcess
/// and four more), run between the quad's own; they are not part of this. SLUS-21104's world hands
/// the constraints the substep's `h` as well, which MX Unleashed's does not: the quad's world has
/// none.
pub fn frame(
    world: &crate::biplane::World,
    p: &Params,
    q: &mut Quad,
    body: &mut crate::atv_body::Body,
    dt: F,
    table: &crate::air::AcosTable,
    m: &mut impl Moto,
) {
    world.step(dt, &mut Frame { p, q, body, table, m });
}

// --- the bike's steps on the quad (MD75a) -----------------------------------------------------------

/// What SLUS-21104's bike steps keep on the quad beyond [`Quad`] and [`Body`], in the steps' own
/// types: the quad is the bike with four wheels, and its words sit where the bike's do.
///
/// Where a word is in both, [`Quad`]'s (or [`Body`]'s) is the one that counts: [`Steps`] copies it
/// into the step's types before each step and back out after. The pairs are the levers and the
/// stick's lean ([`crate::ride::Held`]'s gas, brakes, clutch, lean and reverse are
/// [`Quad::gas_limited`], [`Quad::front_brake`], [`Quad::rear_brake`], [`Quad::clutch_in`],
/// [`Quad::lean`]/[`Quad::lean_fore_aft`] and [`Quad::push_gate`]), the air state's `on_ground`,
/// `contact_vel`, `crossup`, `authority` and `flipping`, and each wheel's steering, contact, normal,
/// steering lock and angle ([`Wheel::steer_command`]), spring rate and contact velocity.
#[derive(Clone, Debug)]
pub struct Bike {
    /// The input limiter's: the rate limiter, the gas cut and how long it holds, the `[Joystick]`
    /// rates, the stick as the pad left it (the limiter leaves it as it is), the clutch lever as
    /// pressed (the clutch ramp reads it) and the start gate's penalty ([`crate::ride::Controls::gate_cut`]).
    pub held: crate::ride::Held,
    pub gas_cut: crate::handling::step::GasCut,
    pub cut_time: F,
    pub joystick: crate::phy::Joystick,
    pub stick: crate::trick::Stick,
    pub clutch: F,
    pub gate_cut: bool,
    /// The clutch ramp's keys, clock and the engine's multiplier it writes.
    pub clutch_keys: crate::r#move::Params,
    pub ride: crate::r#move::Ride,
    pub engine: crate::r#move::Engine,
    /// The air steps' keys (its `control_authority_time` is [`Quad::authority_time`]'s) and the
    /// preload's and backflip's state ([`crate::air::Air`]).
    pub air_keys: crate::air::Params,
    pub air: crate::air::Air,
    /// How long the stick has been held, and the preload cue's latch.
    pub stick_hold: F,
    pub preload_cue: bool,
    /// The 360's torque went in, and the 360 is on: the backflip's lip refuses either.
    pub spin_applied: bool,
    pub spin_active: bool,
    /// The collision record's: the quad touches something, and what it struck is of the class the
    /// mid-clip wreck's reason asks about.
    pub touching: bool,
    pub struck_class: bool,
    /// The trick buttons' byte ([`crate::handling::trick_nibble`] in its low nibble), the scene
    /// node's right and up axes' heights, the rider's stunt id and current stunt (the backflip reads
    /// `+0x1528`, the airborne control `+0x1524`), the backflip's clip and its stunt's id.
    pub buttons: u8,
    pub right_y: F,
    pub up_y: F,
    pub stunt_id: i32,
    pub rider_stunt: i32,
    pub flip_clip: crate::handling::FlipClip,
    pub flip_stunt: u32,
    /// `[Airborne Control] GasGyroTorque` and the gyro options.
    pub gas_gyro_torque: F,
    pub gyro: crate::handling::Gyro,
    /// The four wheels as SLUS-21104's wheel steps see them ([`crate::atv_wheel`]), what stands under
    /// each, and the body's centre of mass in its own frame.
    pub wheels: [crate::wheel::Wheel; 4],
    pub more: [crate::atv_wheel::More; 4],
    pub ground: [crate::atv_wheel::Ground; 4],
    pub cofg: Vf,
    /// MD75b's ([`crate::atv_moto`]): the engine's wheels, the control-off pose, the body's last
    /// position and enabled word, the quad's flag byte and record. The gearbox's shift points, the
    /// head and tail's poses and travels and the drag are [`Bike::clutch_keys`]', the engine's state
    /// and gearbox [`Bike::engine`]'s.
    pub parts: crate::atv_moto::Parts,
}

/// **The quad's [`Moto`] with SLUS-21104's bike steps ported**: the input limiter and clutch
/// ([`crate::handling::step`]), the preload, backflip and airborne control ([`crate::handling`]),
/// each wheel's suspension and tire forces ([`crate::atv_wheel`], whose contact velocity and hinge
/// force [`crate::atv_wheel::run`] answers) and its integrate's steering hinge, run on the quad's
/// fields through [`Bike`].
///
/// Every other call, and what those steps call in turn and is not ported (the stunt's start and end,
/// the wreck, the preload's cue, the wheels' surface drag and spin), goes to `rest` in the original's
/// order. `ground` is the world's ground query the suspension asks (the probe's end and the wheel's
/// contact record in, the point on the ground and its normal out; on Desert Oasis,
/// [`crate::atv_ground::Terrain::ground`]). The stunt start always answers yes, as SLUS-21104's does.
pub struct Steps<R, G> {
    pub bike: Bike,
    pub rest: R,
    pub ground: G,
}

impl<R: Moto, G: FnMut(Vf, &mut u8) -> (Vf, Vf)> Moto for Steps<R, G> {
    fn call(&mut self, c: Call, q: &mut Quad, body: &mut Body) {
        match c {
            Call::InputLimiter(dt) => self.input_limiter(q, dt),
            Call::Clutch(dt) => {
                let b = &mut self.bike;
                let s = crate::r#move::Step { dt, gas: q.gas, clutch: b.clutch, speed: F(q.contact_vel.z()), ..Default::default() };
                crate::handling::step::clutch(&b.clutch_keys, &mut b.ride, &mut b.engine, &s);
            }
            Call::Preload(dt) => {
                let (keys, mut air, mut ch, st) = self.air_in(q, body, dt);
                let posted = crate::handling::step::preload(&keys, &mut air, &mut ch, &st, &mut self.bike.preload_cue);
                self.air_out(q, body, air, ch);
                if posted {
                    self.rest.call(Call::PreloadCue, q, body);
                }
            }
            Call::Backflip(dt) => self.backflip(q, body, dt),
            Call::AirborneControl(dt) => {
                let (keys, mut air, mut ch, st) = self.air_in(q, body, dt);
                let b = &self.bike;
                let inp = crate::handling::AirIn { stunt_id: b.rider_stunt, gas: q.gas_limited, gyro: b.gyro };
                crate::handling::airborne_control(&keys, b.gas_gyro_torque, &mut air, &mut ch, &st, &inp);
                self.air_out(q, body, air, ch);
            }
            Call::Suspension(i, _) => {
                self.wheel_in(q, i);
                let mut wb = self.wheel_body(body);
                let Steps { bike, ground, .. } = self;
                crate::atv_wheel::suspension(&mut bike.wheels[i], &mut bike.more[i], &wb, ground);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::TireForces(i, dt) => {
                self.wheel_in(q, i);
                let mut wb = self.wheel_body(body);
                let Bike { wheels, more, ground, .. } = &mut self.bike;
                let under = &mut ground[i];
                crate::atv_wheel::tire_forces(&mut wheels[i], &mut wb, dt,
                    &mut |c, w, b, x| crate::atv_wheel::run(c, w, b, x, under), &mut more[i]);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::WheelIntegrate(i, dt) => {
                self.rest.call(Call::SurfaceDrag(i, dt), q, body);
                self.rest.call(Call::SpinIntegrate(i, dt), q, body);
                self.wheel_in(q, i);
                let calls = crate::handling::step::wheel_integrate(&mut self.bike.wheels[i], dt);
                debug_assert_eq!(calls, [crate::wheel::Call::SurfaceDrag(dt), crate::wheel::Call::SpinIntegrate(dt)]);
                let mut wb = self.wheel_body(body);
                self.wheel_out(q, body, &mut wb, i);
            }
            Call::AutoShift { inv_radius, .. } => {
                let b = &mut self.bike;
                crate::atv_moto::auto_shift(&b.clutch_keys, &mut b.engine, q, inv_radius);
            }
            Call::HeadTail => {
                let b = &mut self.bike;
                crate::atv_moto::head_tail(&b.clutch_keys, &mut b.parts, &mut b.cofg, &b.ride, b.stick.y, q, body);
            }
            Call::Gravity => {
                if crate::atv_moto::gravity(&mut self.bike.parts, q, body) {
                    self.rest.call(Call::GravityRecord, q, body);
                }
            }
            Call::EngineUpdate(dt) => {
                let b = &mut self.bike;
                let radius = b.wheels.map(|w| w.probe_length);
                crate::atv_moto::engine_update(&mut b.engine, &mut b.parts.drive, radius, b.parts.driven, q.gas_limited, q.clutch_in, dt);
            }
            Call::AirDrag(_) => crate::atv_moto::air_drag(self.bike.clutch_keys.drag, q, body),
            Call::WheelClear(i) => {
                q.wheels[i].touching = false;
                let Bike { wheels, more, ground, .. } = &mut self.bike;
                crate::atv_moto::wheel_clear(&mut wheels[i], &mut more[i], &mut ground[i]);
            }
            Call::RevLimit => crate::atv_moto::rev_limit(&self.bike.engine, &mut self.bike.parts.drive, self.bike.parts.driven),
            Call::PeakRates => crate::atv_moto::peak_rates(q, body),
            c => self.rest.call(c, q, body),
        }
    }
}

impl<R: Moto, G> Steps<R, G> {
    /// The input limiter on the quad's levers: the controls as the pad left them (the gas and brake
    /// levers, the clutch lever, the balance stunt as the gas's zero gate) in, the levers it leaves
    /// and the rate limiter's out.
    fn input_limiter(&mut self, q: &mut Quad, dt: F) {
        let b = &mut self.bike;
        let h = &mut b.held;
        (h.gas, h.front_brake, h.rear_brake, h.clutch) = (q.gas_limited, q.front_brake, q.rear_brake, q.clutch_in);
        (h.lean, h.reverse) = ((q.lean, q.lean_fore_aft), q.push_gate);
        let c = crate::ride::Controls {
            gas: q.gas,
            brake: q.brake,
            clutch: b.clutch,
            hands_off: q.balancing,
            gas_cut: b.gate_cut,
            ..Default::default()
        };
        let inp = crate::handling::step::LimiterIn { airborne: q.airborne, cut_time: b.cut_time };
        let c = crate::handling::step::input_limiter(h, &mut b.gas_cut, &c, b.stick, &b.joystick, dt, &inp);
        (q.gas, q.brake, b.clutch) = (c.gas, c.brake, c.clutch);
        (q.gas_limited, q.front_brake, q.rear_brake, q.clutch_in) = (h.gas, h.front_brake, h.rear_brake, h.clutch);
        (q.lean, q.lean_fore_aft, q.push_gate) = (h.lean.0, h.lean.1, h.reverse);
    }

    /// The air steps' keys, state, chassis and step off the quad.
    fn air_in(&self, q: &Quad, body: &Body, dt: F) -> (crate::air::Params, crate::air::Air, crate::air::Chassis, crate::air::Step) {
        let b = &self.bike;
        let mut keys = b.air_keys;
        keys.control_authority_time = q.authority_time;
        let mut air = b.air;
        (air.on_ground, air.contact_vel, air.crossup, air.authority, air.flipping) =
            (q.on_ground, q.contact_vel, q.crossup, q.authority, q.flipping);
        let ch = crate::air::Chassis {
            speed: q.speed_len,
            pitch: F(q.euler.x()),
            roll: F(q.euler.z()),
            force: body.force,
            moment: body.moment,
            vel: body.velocity,
            local_vel: q.local_vel,
            ang_vel: body.ang_vel,
        };
        let st = crate::air::Step {
            dt,
            heading: q.heading,
            up: q.up,
            stick: (b.stick.x, b.stick.y),
            stick_hold: b.stick_hold,
            dpad_y: b.stick.digital[1],
            brake: q.brake,
            front_brake: q.front_brake,
            rear_brake: q.rear_brake,
            lean: q.lean,
            touching: b.touching,
            spinning: b.spin_applied,
            spin_active: b.spin_active,
            spin_ramp: q.spinning,
            ..Default::default()
        };
        (keys, air, ch, st)
    }

    fn air_out(&mut self, q: &mut Quad, body: &mut Body, air: crate::air::Air, ch: crate::air::Chassis) {
        (q.on_ground, q.contact_vel, q.crossup, q.authority, q.flipping) =
            (air.on_ground, air.contact_vel, air.crossup, air.authority, air.flipping);
        self.bike.air = air;
        (body.force, body.moment) = (ch.force, ch.moment);
    }

    /// The backflip, its mid-clip wreck's reason asked of the ported test on the peak-rates vector
    /// ([`Quad::attitude`], the g loads it reads: `x` sideways, `z` fore and aft).
    fn backflip(&mut self, q: &mut Quad, body: &mut Body, dt: F) {
        let (keys, mut air, mut ch, st) = self.air_in(q, body, dt);
        let b = &mut self.bike;
        let inp = crate::handling::FlipIn { buttons: b.buttons, right_y: b.right_y, up_y: b.up_y, stunt_id: b.stunt_id };
        let loads = crate::air::Loads { side: F(q.attitude.x()), fore_aft: F(q.attitude.z()), ..Default::default() };
        let steep = crate::handling::step::flip_wreck_steep(&keys, b.struck_class, q.airborne, &loads);
        let mut started = false;
        let out = crate::handling::backflip(&keys, &mut air, &mut ch, &st, &inp, &mut b.flip_clip, || {
            started = true;
            true
        }, || steep);
        if out.stunt_cleared {
            b.stunt_id = 255;
        }
        let stunt = b.flip_stunt;
        self.air_out(q, body, air, ch);
        if started {
            self.rest.call(Call::StartStunt(stunt), q, body);
        }
        if out.stunt_ended {
            self.rest.call(Call::EndStunt, q, body);
        }
        if let Some(reason) = out.wreck {
            self.rest.call(Call::Wreck(reason), q, body);
        }
    }

    /// Wheel `i`'s fields the quad keeps, into the wheel step's wheel.
    fn wheel_in(&mut self, q: &Quad, i: usize) {
        let (w, x, qw) = (&mut self.bike.wheels[i], &mut self.bike.more[i], &q.wheels[i]);
        (w.steered, w.balancing, w.contact, w.contact_normal) = (qw.steered, qw.balancing, qw.touching, qw.normal);
        (w.max_steer_angle, w.steer_angle, w.spring_rate) = (qw.max_steer_angle, qw.steer_command, qw.spring);
        x.chassis_velocity_local = qw.contact_vel;
    }

    fn wheel_body(&self, body: &Body) -> crate::atv_wheel::Body {
        crate::atv_wheel::Body {
            position: body.position,
            velocity: body.velocity,
            force: body.force,
            moment: body.moment,
            correction: body.correction,
            orientation: body.orientation,
            cofg: self.bike.cofg,
            spin: body.ang_vel,
        }
    }

    fn wheel_out(&mut self, q: &mut Quad, body: &mut Body, wb: &mut crate::atv_wheel::Body, i: usize) {
        let (w, x, qw) = (&self.bike.wheels[i], &self.bike.more[i], &mut q.wheels[i]);
        (qw.steered, qw.balancing, qw.touching, qw.normal) = (w.steered, w.balancing, w.contact, w.contact_normal);
        (qw.max_steer_angle, qw.steer_command, qw.spring) = (w.max_steer_angle, w.steer_angle, w.spring_rate);
        qw.contact_vel = x.chassis_velocity_local;
        (body.position, body.force, body.moment, body.correction) = (wb.position, wb.force, wb.moment, wb.correction);
    }
}

// --- MD65a: the wreck body ----------------------------------------------------------------------------

/// The file the quad's wreck rays load from, on its scene node.
pub const WRECK_FILE: &str = "WreckATVm.col";
/// The category the wreck rays are in, and the one the trigger probe's category list holds.
pub const PHYSICS_BODY: &str = "PHYSICSBODY";
pub const TRIGGER: &str = "TRIGGER";

/// The quad's collidables the wreck body is about: the three the vehicle's own init made (the riding
/// rays, a ray probe off the bike's `m.col`; the frame mesh; the sweep) and the two the quad's init
/// adds (a ray probe with no file in the trigger category, and the wreck rays).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Riding,
    Frame,
    Sweep,
    Trigger,
    Wreck,
}

/// A collidable's list as the original keeps it: a mode word and its entries, a 0 entry an empty slot.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct List {
    pub mode: u32,
    pub list: Vec<u32>,
}

impl List {
    /// Another collidable added to an object list: nothing for none (0); nothing if it is listed;
    /// else into the first empty slot, else on the end.
    pub fn add_object(&mut self, handle: u32) {
        if handle != 0 {
            self.add(handle);
        }
    }

    /// A category word added to a category list: as [`List::add_object`], but a 0 is added too (it
    /// fills no slot, so it is listed once, on the end, if no slot is empty).
    pub fn add_category(&mut self, word: u32) {
        self.add(word);
    }

    fn add(&mut self, x: u32) {
        if self.list.contains(&x) {
            return;
        }
        match self.list.iter_mut().find(|e| **e == 0) {
            Some(slot) => *slot = x,
            None => self.list.push(x),
        }
    }
}

/// One of the quad's collidables: its handle (what the others' lists name it by, never 0) and its
/// object and category lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Col {
    pub handle: u32,
    pub objects: List,
    pub categories: List,
}

impl Col {
    /// A part as its constructor leaves it: every list empty, and in mode 1 (ignore what is listed).
    pub fn made(handle: u32) -> Col {
        let empty = List { mode: 1, list: Vec::new() };
        Col { handle, objects: empty.clone(), categories: empty }
    }
}

/// The quad's collidables as the wreck body's block reads and leaves them. The field map is
/// research/oracle.txt's `# MD65a`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WreckBody {
    pub riding: Col,
    pub frame: Col,
    pub sweep: Col,
    pub trigger: Col,
    pub wreck: Col,
    /// The wreck rays' own category word (looked up as [`PHYSICS_BODY`]), and whether they were given
    /// the quad as their owner and the quad's collision record as theirs: the wreck rays' hits go into
    /// the record the riding rays' do.
    pub wreck_category: u32,
    pub wreck_owned: bool,
    pub wreck_record: bool,
    /// Whether the trigger probe was given the quad as its owner.
    pub trigger_owned: bool,
    /// The riding rays' handle, kept for the swap back ([`swap_rays`]).
    pub riding_kept: u32,
}

/// The flags words the two parts are made with (both leave the flag byte's bit 1 set and the rest
/// as they were).
const TRIGGER_FLAGS: u32 = 2;
const WRECK_FLAGS: u32 = 3;

/// **The quad's init, its last collision block**: the two ray-probe parts made, and every collidable
/// of the quad told to ignore them, and them the others.
///
/// The trigger probe (`trigger`, the handle its allocation gave) is made, flagged, given the quad and
/// hung in the scene tree; the riding rays are kept. The wreck rays (`wreck`) are made, flagged,
/// loaded from [`WRECK_FILE`] on the scene node and readied, put in [`PHYSICS_BODY`]'s category,
/// given the quad and its collision record, and hung. Then the riding rays and the sweep ignore
/// both; the wreck rays (object mode 1) ignore the trigger probe, the riding rays, the frame and the
/// sweep; the trigger probe (object mode 1) the riding rays, the frame, the sweep and the wreck rays,
/// and (category mode 1) [`TRIGGER`]'s category and the quad's own word `actor`. The frame mesh's
/// list is left alone. `category` answers a name with its category word, in the order asked.
#[allow(clippy::too_many_arguments)] // the original's inputs, each one a word it reads
pub fn build_wreck_body(
    w: &mut WreckBody,
    trigger: u32,
    wreck: u32,
    actor: u32,
    mut category: impl FnMut(&str) -> u32,
    q: &mut Quad,
    body: &mut Body,
    m: &mut impl Moto,
) {
    m.call(Call::PartMade(Part::Trigger), q, body);
    w.trigger = Col::made(trigger);
    m.call(Call::PartFlags(Part::Trigger, TRIGGER_FLAGS), q, body);
    w.trigger_owned = true;
    m.call(Call::PartHung(Part::Trigger), q, body);
    w.riding_kept = w.riding.handle;
    m.call(Call::PartMade(Part::Wreck), q, body);
    w.wreck = Col::made(wreck);
    m.call(Call::PartFlags(Part::Wreck, WRECK_FLAGS), q, body);
    m.call(Call::PartLoad(Part::Wreck), q, body);
    m.call(Call::PartReady(Part::Wreck), q, body);
    w.wreck_category = category(PHYSICS_BODY);
    w.wreck_owned = true;
    w.wreck_record = true;
    m.call(Call::PartHung(Part::Wreck), q, body);

    let (t, k) = (w.trigger.handle, w.wreck.handle);
    let (r, f, s) = (w.riding.handle, w.frame.handle, w.sweep.handle);
    w.riding.objects.add_object(t);
    w.riding.objects.add_object(k);
    w.sweep.objects.add_object(t);
    w.sweep.objects.add_object(k);
    w.wreck.objects.mode = 1;
    for h in [t, r, f, s] {
        w.wreck.objects.add_object(h);
    }
    w.trigger.objects.mode = 1;
    for h in [r, f, s, k] {
        w.trigger.objects.add_object(h);
    }
    w.trigger.categories.mode = 1;
    let trigger_word = category(TRIGGER);
    w.trigger.categories.add_category(trigger_word);
    w.trigger.categories.add_category(actor);
}

/// Which rays the quad's collision runs on, as the swap leaves it: the handle in the quad's two
/// active-rays words and in its collision record's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rays {
    pub active: u32,
    pub active_too: u32,
    pub record: u32,
    /// The two candidates: the wreck rays and the riding rays kept ([`WreckBody::riding_kept`]).
    pub wreck: u32,
    pub riding: u32,
}

/// **The swap between the riding and the wreck rays**: the wreck entry's (`wrecked`) and the unwreck's
/// (not), the bike's two shared virtuals, on the quad's two parts.
///
/// The one swapped to goes into the quad's two active-rays words and the collision record's; the
/// other is hidden from the scene and taken off its collision world. The one swapped to is then
/// shown and put in the quad's world, unless it is in a world already (`in_world`).
pub fn swap_rays(r: &mut Rays, wrecked: bool, in_world: bool, q: &mut Quad, body: &mut Body, m: &mut impl Moto) {
    let (to, to_part, from_part) =
        if wrecked { (r.wreck, Part::Wreck, Part::Riding) } else { (r.riding, Part::Riding, Part::Wreck) };
    r.active = to;
    r.active_too = to;
    r.record = to;
    m.call(Call::PartHide(from_part), q, body);
    m.call(Call::PartOffWorld(from_part), q, body);
    if !in_world {
        m.call(Call::PartShow(to_part), q, body);
        m.call(Call::PartInWorld(to_part), q, body);
    }
}

// --- MD65b: the quad's small slots --------------------------------------------------------------------

/// **The quad's level setup, its surface remap**: the bike's shared level setup ([`Call::LevelSetup`]),
/// then each of the sixteen surfaces the level maps (`map[i]` not negative) takes the tire tables of
/// surface `map[i]`, on all four wheels, in order: surface 0 first, and per surface the left front,
/// right front, left rear, right rear. A copy is made only with both surfaces under sixteen, and
/// it copies every table but the last side one (`DonutsSideForceSpeedScale`), which each surface
/// keeps. Being in place, a later surface mapped to one already remapped takes the remapped tables.
pub fn remap_surfaces(
    t: &mut TireSetup,
    map: &[i32; crate::phy::MAX_SURFACES],
    q: &mut Quad,
    body: &mut Body,
    m: &mut impl Moto,
) {
    m.call(Call::LevelSetup, q, body);
    for (i, &src) in map.iter().enumerate() {
        if src < 0 {
            continue;
        }
        for w in &mut t.wheels {
            copy_surface(w, i, src);
        }
    }
}

/// The wheel's surface copy: `dst` takes `src`'s tables but the donut scale, both under sixteen.
fn copy_surface(w: &mut WheelTire, dst: usize, src: i32) {
    let n = crate::phy::MAX_SURFACES;
    if src < 0 || src as usize >= n || dst >= n {
        return;
    }
    let from = w.surfaces[src as usize];
    let to = &mut w.surfaces[dst];
    to.drive = from.drive;
    to.side[..12].copy_from_slice(&from.side[..12]);
    to.drag = from.drag;
}

/// What the quad answers for the surface it is on: the left rear wheel's, as the ground query left it
/// ([`WheelGround::surface`]).
pub fn surface_under(wheels: &[WheelGround; 4]) -> u8 {
    wheels[2].surface
}

/// What the quad answers for its strut travel: the left front wheel's.
pub fn strut_travel(t: &TireSetup) -> F {
    t.wheels[0].strut_compression
}

/// **The trigger probe's segments**: with a trigger probe made ([`build_wreck_body`]), one segment per
/// wheel (left front, right front, left rear, right rear), from the mount raised by the strut travel
/// down to the mount lowered by the radius (`y` only; `x`, `z` and `w` are the mount's). The original
/// hands the eight points to the probe's shape on the quad's scene node; with none it does nothing
/// (`None`).
pub fn trigger_segments(t: &TireSetup, has_trigger: bool) -> Option<[[Vf; 2]; 4]> {
    if !has_trigger {
        return None;
    }
    Some(t.wheels.map(|w| {
        let top = F(fpu::add(w.mount.y(), w.strut_compression.0));
        let bottom = F(fpu::sub(w.mount.y(), w.radius.0));
        [w.mount.merge(Dest::Y, Vf::splat(top.0)), w.mount.merge(Dest::Y, Vf::splat(bottom.0))]
    }))
}

/// One wheel as the quad's ground settle reads and leaves it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelGround {
    /// Where the wheel is mounted, and the offset from it the settle probes at (body frame).
    pub mount: Vf,
    pub probe: Vf,
    /// The probe point in the world, as the ground query leaves it, the ground's normal there and
    /// the surface it is; and the point kept once all four are settled.
    pub point: Vf,
    pub normal: Vf,
    pub surface: u8,
    pub kept: Vf,
}

/// **The quad's ground settle**: per wheel (left front, right front, left rear, right rear), the mount
/// plus the probe (`xyz`; `w` the mount's) turned into the world by the body's orientation and moved
/// to the quad's scene node (`node_pos`, `xyz`), and the ground queried there: `ground` answers the
/// point (in place), the normal and the surface, one wheel at a time, in order. Then each wheel keeps
/// its point.
pub fn settle_wheels(wheels: &mut [WheelGround; 4], body: &Body, node_pos: Vf, mut ground: impl FnMut(&mut WheelGround)) {
    for w in wheels.iter_mut() {
        let p = add_xyz(w.mount, w.probe);
        w.point = add_xyz(body.to_world(p), node_pos);
        ground(w);
    }
    for w in wheels.iter_mut() {
        w.kept = w.point;
    }
}
