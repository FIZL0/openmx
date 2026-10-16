//! **Movement on the ground**: the step that turns the throttle, the brake and the rider's lean into
//! forces on the chassis, and the order it does everything in.
//!
//! This is the freeroam critical path. One call does the whole ground frame: it poses the bike (where
//! the centre of gravity sits, where the front wheel points), picks a gear, works the clutch, and then
//! runs an inner loop — 55 substeps a second, rounded up — that accumulates gravity, aerodynamic drag
//! and the reverse push into the chassis and calls the suspension, the tires and the rigid-body
//! integrator once each.
//!
//! # What is here and what is not
//!
//! The original's step calls twenty-one other functions, and the three biggest of them (the suspension
//! ray, the tire's ground forces, the roll and pitch control) are each a task's worth of work on their
//! own. So this module ports the *spine* — everything the step does itself, which is where the brief's
//! four questions are actually answered:
//!
//! | The brief | Where it is |
//! |---|---|
//! | turning | [`steer_scale`] and [`steer_command`]: the lean times the wheel's limit, faded out with speed |
//! | acceleration | [`road_rpm`] and [`shift`] choose the gear, [`clutch`] scales the torque that comes off the launch |
//! | friction | [`drag`], the only force the step applies against the direction of travel |
//! | maximum speed | [`drag`] against the engine, with [`engine_post`]'s rev limiter on top of the gear [`shift`] picked |
//! | the update order | [`step`], which returns the ordered list of everything it called ([`Call`]) |
//!
//! The pieces it does not port are not guessed at: [`step`] records each one in the [`Call`] list it
//! returns, in the order and with the substep `dt` the original passes, so the order is proved even
//! where the callee is not. `tests/move.rs` proves the list against the original with every one of
//! those callees hooked, and `research/journal/…-S3-…` lists them with the reason each was deferred.
//!
//! # The substep, and the one call that does not get it
//!
//! `n = (int)(55 * dt) + 1` substeps of `h = dt / n` each. Everything in the loop is handed `h` —
//! except [`Call::Contacts`], which is handed the *frame's* `dt`. That is not a transcription slip:
//! the original really does pass `f23` (the argument) and not `f22` (the substep) to that one call,
//! and a port that "fixes" it drifts.
//!
//! # The state, split four ways
//!
//! The original keeps all of this in one 5.3 KB bike object with the engine, both wheels and the
//! chassis hanging off it by pointer. Here: [`Params`] is what the bike's `bike*.phy` file sets and
//! nothing changes, [`Ride`] is what this step owns, [`Chassis`], [`Wheel`] and [`Engine`] are the
//! three objects it reaches into, and [`Step`] is everything else it reads — the controls after the
//! input layer's rate limits, and the two vectors the chassis' own transform produces.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{Dest, F, Field, Vf, fpu, vu};

/// `-pi/2` and `-pi` as the original's instruction stream holds them. Negative because the stick
/// angle the donut reads runs clockwise from straight ahead, so a right-hand donut is the whole
/// range from `0` to `-pi` and the quadrant boundary is at `-pi/2`.
const NEG_HALF_PI: f32 = -1.5707964;
#[allow(clippy::approx_constant)] // the game's own seven digits, not `f32::consts::PI` rounded
const NEG_PI: f32 = -3.1415927;
/// The substep rate: the loop runs `(int)(dt * 55) + 1` times, so 55 Hz is the floor and a 60 fps
/// frame gets two substeps, not one.
const SUBSTEP_RATE: f32 = 55.0;
/// Radians per second to RPM (`60 / 2pi`) and back (`2pi / 60`), to the seven digits the original's
/// two literals hold.
const RPM_PER_RAD: f32 = 9.549296;
const RAD_PER_RPM: f32 = 0.1047198;
/// The speed window the wheel springs fade across, in the chassis' own units: stopped below the
/// first, fully moving above the second (the second is exactly twice the first).
const SPRING_STOPPED: f32 = 14.6666;
const SPRING_MOVING: f32 = 29.3332;
/// Below this speed, a rider who is not on the throttle has the clutch pulled in for them — which is
/// what stops the engine stalling when the bike rolls to a halt.
const IDLE_SPEED: f32 = 7.3333;
/// How far the stick has to be deflected before a donut counts as commanded.
const DONUT_DEFLECTION: f32 = 0.9;
/// Half, as the drag's own register constant.
const HALF: f32 = 0.5;
/// How close something has to be before the collision record is told to look at it.
const COLLISION_NEAR: f32 = 0.25;

/// The quadword the original's rodata holds for "nothing": a zero vector with `w` of 1, which is what
/// a point at the origin is in homogeneous coordinates. Four things in this step reset to it.
pub const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// Which wheel a [`Call`] is about. The original's loop is written out twice, front then rear, every
/// time; this keeps that order visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Front,
    Rear,
}

/// The bike's own tuning, as its `bike*.phy` file names it. One of these per bike, never written by
/// the simulation. Only the parameters this step reads are here; the file holds about ninety more,
/// and the journal entry has the full offset map.
#[derive(Clone, Copy, Debug, Default)]
pub struct Params {
    /// Where the centre of gravity sits when the rider is neutral, and where it sits instead while
    /// the bike is being balanced on one wheel.
    pub cofg: Vf,
    pub balance_cofg: Vf,
    /// How far the centre of gravity travels with the stick: forward when the stick goes forward,
    /// back when it comes back. Both are *subtracted*, so both are positive in the file.
    pub cofg_travel_forward: F,
    pub cofg_travel_back: F,
    /// Extra forward travel while the brake is on, mapped against how hard the bike is leaning.
    pub brake_cofg_travel_forward: F,
    /// Travel *backwards* while the clutch is being dumped, faded out over the same ramp as the
    /// clutch's torque multiplier.
    pub clutch_cofg_travel_back: F,

    /// The steering fade: the full steering scale up to `steer_speed_1` and `steer_scale_2` of it
    /// from `steer_speed_2` on. The two speeds being equal turns the fade off.
    pub steer_speed_1: F,
    pub steer_scale_1: F,
    pub steer_speed_2: F,
    pub steer_scale_2: F,

    /// Donuts: the speed above which the rear wheel will not break away, and the two stick angles
    /// the roll scale is mapped between. The loader has already turned the file's single
    /// `MinDonutAngle` in degrees into these two: `donut_angle = -deg * pi/180` and
    /// `donut_angle_back = -pi - donut_angle`.
    pub max_donut_speed: F,
    pub donut_angle: F,
    pub donut_angle_back: F,

    /// Each wheel's spring rate stopped and moving — already multiplied out by the loader into the
    /// chassis' units (`|Spring * mass * 32.2|`), which is why they are here and not on the wheel.
    pub front_spring_when_stopped: F,
    pub front_spring: F,
    pub rear_spring_when_stopped: F,
    pub rear_spring: F,

    /// The clutch's torque multiplier: how long the ramp lasts, the speed the multiplier has faded
    /// to 1 by, and what it starts at from a standstill.
    pub clutch_torque_scale_time: F,
    pub max_clutch_torque_scale_speed: F,
    pub max_clutch_torque_scale: F,

    /// Shift points, in engine RPM.
    pub up_shift_rpm: F,
    pub down_shift_rpm: F,

    /// Reverse: the speed it pushes the bike up to, and the force per unit of speed error.
    pub reverse_speed: F,
    pub reverse_gain: F,

    /// Aerodynamic drag, against the square of the speed.
    pub drag: F,

    /// The gravity force, which the bike's constructor has already multiplied by its mass — so it is
    /// a force and not an acceleration, and it is a parameter rather than a constant.
    pub gravity: Vf,
}

/// What the ground step owns: the only fields on the bike itself that it writes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ride {
    /// The peak g load this frame, in the chassis' own frame, kept as the largest *signed* value by
    /// magnitude over the frame's substeps. S10's wreck test reads it.
    pub g_peak: Vf,
    /// The clutch's ramp: how far into it we are, what the multiplier started at, and how much of
    /// the centre-of-gravity travel the launch gets. All three are set on the substep the ramp
    /// starts and only read after that.
    pub clutch_timer: F,
    pub clutch_scale_start: F,
    pub clutch_cofg_weight: F,
    /// The rate-limited clutch the input layer maintains — written here only by the one rule that
    /// pulls the clutch in for a rider who is rolling to a stop off the throttle.
    pub clutch_in: F,
    /// The collision record's "something is close" flag, set from the bike's own node once a frame
    /// after the substep loop.
    pub collision_near: bool,
    /// The four no-hander accumulators the step zeroes before each call to the no-hander step, in
    /// the original's own order (`+0xa6c`, `+0xa64`, `+0xa4c`, `+0xa60`). That step is not ported
    /// here, so they are not named: they are cleared, handed over, and nothing in this module reads
    /// them.
    pub no_hander_scratch: [F; 4],
}

/// The chassis' rigid body, as far as this step is concerned. Everything here belongs to the chassis
/// and is only reached into — the integrator that consumes `force` is [`Call::Integrate`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Chassis {
    /// Speed along the ground, which the chassis measures: the steering fade, the drag and the donut
    /// test all read this one rather than the bike's own forward speed.
    pub speed: F,
    /// The force accumulator the substep adds into, and the two acceleration accumulators it clears
    /// once a frame.
    pub force: Vf,
    pub accel: Vf,
    pub ang_accel: Vf,
    /// Where the body's mass acts, which this step moves about with the rider's weight.
    pub cofg: Vf,
    /// The chassis has a transform. Without one the reverse push has nothing to rotate its force by
    /// and is skipped.
    pub has_transform: bool,
    /// Velocity, whose direction the drag acts against.
    pub vel: Vf,
    /// The three flags [`pose`] sets on the chassis every frame: the third is "the wheels are off the
    /// ground", the other two are always 1.
    pub flags: (i32, i32, i32),
}

/// One wheel, as far as this step is concerned.
#[derive(Clone, Copy, Debug, Default)]
pub struct Wheel {
    /// Set on the *front* wheel only: the bike is being balanced, so the suspension is to hold
    /// itself up rather than follow the ground.
    pub balancing: bool,
    /// Set on the *rear* wheel only: how much of a donut the stick is asking for, 0 to 1.
    pub donut_scale: F,
    /// The spring rate this frame, faded between the two in [`Params`] by speed.
    pub spring: F,
    /// The steering lock, in radians, and the angle [`steer_command`] asks for within it.
    pub max_steer_angle: F,
    pub steer_command: F,
    /// `1 / TireRadius`, which is what turns the bike's speed into wheel RPM.
    pub inv_radius: F,
    /// How fast the wheel is spinning, in radians per second. [`engine_post`] is the only thing here
    /// that writes it.
    pub spin: F,
    /// This tire is touching the ground. Cleared by [`wheel_clear`] and set again by
    /// [`Call::Contacts`].
    pub contact: bool,
    /// Set on the rear wheel: the front is down and the rear is not, which the tire forces use to
    /// move load off a wheel that is in the air.
    pub other_tire_down: bool,
    /// Everything else [`wheel_clear`] empties. See [`Fresh`].
    pub fresh: Fresh,
}

/// The wheel state a substep rebuilds from nothing: what the suspension and the tire forces wrote
/// last substep, which [`wheel_clear`] empties at the top of every one.
///
/// Those two steps are not ported here, so these fields are not named — a guessed name is worse than
/// an offset, and the journal records both the offsets and why they were deferred. They are kept, in
/// the original's order, so that [`wheel_clear`] can be proved against the original word for word.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fresh {
    /// The seven scalars, in the order the original zeroes them: `+0xd0`, `+0xd4`, `+0x1f0`,
    /// `+0x1f4`, `+0xf0`, `+0xf8`, `+0x1a0`. (`+0xdc`, in the middle of that run, is
    /// [`Wheel::contact`].)
    pub scalars: [F; 7],
    /// The three quadwords it sets to [`NOTHING`]: `+0x170`, `+0x1b0`, `+0x1c0`.
    pub vectors: [Vf; 3],
}

/// The engine, as far as this step is concerned: the gearbox and the two numbers that turn engine
/// revs into wheel revs.
#[derive(Clone, Copy, Debug, Default)]
pub struct Engine {
    /// The gear, counted from 1, and how many there are. [`set_gear`] will not leave this range.
    pub gear: i32,
    pub gears: i32,
    /// The final drive, and the per-gear ratios the box holds (the original reads these through a
    /// pointer; `gears` says how many are real).
    pub final_drive: F,
    pub gear_ratios: [F; 8],
    /// The gear's overall ratio and its reciprocal, both written by [`set_gear`].
    pub ratio: F,
    pub inv_ratio: F,
    /// What the clutch is doing to the engine's torque, which [`clutch`] writes.
    pub clutch_torque_scale: F,
    /// The clutch is locked up, so the engine and the wheel turn together — which is the only case
    /// [`engine_post`]'s rev limiter can do anything about.
    pub clutch_locked: bool,
    /// The rev limit, in RPM.
    pub rev_limit: F,
}

/// What the rest of the bike hands the ground step for one frame: the controls as the input layer
/// leaves them, the two flags the air code's contact test wrote, and the two vectors that come out of
/// the chassis' transform.
#[derive(Clone, Copy, Debug, Default)]
pub struct Step {
    pub dt: F,
    /// Either wheel touching, and both touching. Written by the air module's contact step, which
    /// runs before this one.
    pub on_ground: bool,
    pub both_wheels: bool,
    /// The bike's own forward speed, which the steering fade, the shift points and the reverse push
    /// read (as opposed to [`Chassis::speed`], which the drag and the donut read).
    pub speed: F,
    /// The stick's `y`, which moves the centre of gravity; its angle and deflection, which ask for a
    /// donut; and the rider's lean, which steers.
    pub stick_y: F,
    pub stick_angle: F,
    pub deflection: F,
    pub lean: F,
    /// Throttle, brake and clutch, as the rider is holding them.
    pub gas: F,
    pub brake: F,
    pub clutch: F,
    /// The throttle *after* the input layer's rate limit, which is what the idle-clutch rule looks
    /// at — a rider who has just let go still has throttle here.
    pub gas_rate_limited: F,
    /// Reverse is engaged.
    pub reverse: F,
    /// The bike is being balanced on one wheel: a different centre of gravity, no steering, and the
    /// balance step runs instead of the no-hander one.
    pub balancing: bool,
    /// The acceleration the chassis measured, already turned into the bike's own frame. The
    /// transform that does that belongs to the chassis, so the port takes the answer as given and
    /// records the vector it handed over as [`Call::Unrotate`].
    pub local_accel: Vf,
    /// The reverse push's force, already rotated into the world. Likewise the chassis' transform's
    /// work: the port computes the local vector itself, hands it over as [`Call::Rotate`], and adds
    /// this.
    pub reverse_force: Vf,
    /// The field on the bike's own node that the collision record's "look at this" flag is set from.
    pub node_distance: F,
}

/// Everything the ground step calls and this module does not port, in the order it calls them and
/// with the `dt` it passes. [`step`] returns these as a list, which is how the update order gets
/// proved even where the callee does not.
///
/// Each variant names the original's function; `research/oracle.txt` has the address, and
/// `research/journal/…-S3-…` says why that one was deferred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// `bike_stand_upright` — holds the bike upright at low speed.
    StandUpright(F),
    /// `bike_balance` — the balance-on-one-wheel controller, only while [`Step::balancing`].
    Balance(F),
    /// `bike_roll_pitch` — the roll and pitch control: the rider's lean, and what the stick does to it.
    RollPitch(F),
    /// `bike_preload` and `bike_backflip` — the preload and the backflip, both ported in `air.rs` (S10).
    Preload(F),
    Backflip(F),
    /// `engine_update` — the engine: the torque curve, the clutch's slip, and the torque that reaches
    /// the driven wheel.
    EngineUpdate(F),
    /// `bike_airborne_control` — what the stick commands while airborne, also S10's.
    AirborneControl(F),
    /// `transform_rotate` — the chassis' transform rotating the reverse push's local force into the world.
    /// Carries the local vector, so the port's own arithmetic for it is compared.
    Rotate(Vf),
    /// `bike_wheel_contacts` — distributes the frame's collision contacts onto the two wheels. The one call
    /// in the loop that gets the frame's `dt` and not the substep's.
    Contacts(F),
    /// `wheel_suspension` — the suspension ray: where the ground is under this wheel and what the spring
    /// does about it.
    Suspension(Side, F),
    /// `wheel_tire_forces` — the tire's forces on the ground: drive, brake and side.
    TireForces(Side, F),
    /// `wheel_integrate` — the wheel's own integration: its spin, and its steering angle.
    WheelIntegrate(Side, F),
    /// The collision record's per-substep opening call, through the record's vtable at `+0x80`. The
    /// flag is "this is the first substep".
    CollisionBegin(bool),
    /// `transform_unrotate` — the chassis' transform taking the measured acceleration back into the bike's
    /// frame. Carries the vector handed over.
    Unrotate(Vf),
    /// The rigid-body integrator, through the chassis' vtable at `+0x3c`: the one call that turns
    /// everything above into motion.
    Integrate(F),
    /// `bike_no_hander` — the no-hander, after its four accumulators have been cleared.
    NoHander(F),
    /// `actor_search_reset` and `actor_search_gather` — the nearby-actor search the step resets and refills once a
    /// frame, with a radius of 3.
    SearchReset,
    SearchGather,
}

// --- the arithmetic the original inlines everywhere ------------------------------------------------

/// The original's inlined "map and clamp": `x` from the range `a..b` onto `out0..out1`, clamped to
/// whichever of the two outputs is smaller and larger — it does not assume they are in order.
///
/// Written out because the compiler inlines this at a dozen call sites in this step alone, and the
/// order of its six operations (two subtractions, a divide, the `adda`/`madd` lerp, a `max` then a
/// `min`) is what makes the last bit come out right. Callers test `a != b` themselves, because what
/// they do when the input range is empty differs from one to the next.
fn map_clamp(x: F, a: F, b: F, out0: F, out1: F) -> F {
    let u = (x - a) / (b - a);
    let v = out0.lerp(out1, u);
    v.max(out0.min(out1)).min(out0.max(out1))
}

/// Normalise `xyz` the way this code's VU0 does: the sum of squares with `vmul`/`vadday`/`vmaddz`,
/// then `vrsqrt` — an *approximate* reciprocal square root, not a square root and a divide, so it is
/// not the same number as [`vu::normalize`]'s. `w` comes through untouched.
fn normalize(v: Vf) -> Vf {
    let sq = v.mul(v);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, len2)))
}

/// `v.xyz * s`, as `qmtc2` into a register and `vmulx.xyz` off its `x` lane. `w` is `v`'s.
fn scale(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::new(s.0, s.0, s.0, s.0).bc(Field::X)))
}

/// `a.xyz += b.xyz`, the `vadd.xyz` this step adds every force with. `w` is `a`'s, untouched — which
/// matters, because the force accumulator's `w` is 1 and stays 1 all frame.
fn add_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

// --- posing the bike: the centre of gravity, the steering, the flags -------------------------------

/// How far the centre of gravity travels fore and aft with the stick. The stick forward (`y` below
/// zero) uses the forward travel, anything else the back travel, and both are negated before the
/// multiply — so a stick held forward moves the mass forward and a stick held back moves it back.
pub fn cofg_shift(p: &Params, stick_y: F) -> F {
    let travel = if stick_y < 0.0 { p.cofg_travel_forward } else { p.cofg_travel_back };
    F(fpu::neg(travel.0)) * stick_y
}

/// The extra forward travel braking gets, mapped from how hard the bike is leaning. The original
/// divides by the literal 1.0 the map's range gives it rather than folding it away, which is exact,
/// so this is written as the map it is.
pub fn brake_cofg_shift(p: &Params, lean: F) -> F {
    map_clamp(lean.abs(), F::ZERO, F::ONE, F::ZERO, p.brake_cofg_travel_forward)
}

/// How much of the steering lock the rider gets at this speed: all of `steer_scale_1` up to
/// `steer_speed_1`, `steer_scale_2` from `steer_speed_2` on, interpolated between. With the two
/// speeds equal there is no fade at all and the answer is `steer_scale_1`.
pub fn steer_scale(p: &Params, speed: F) -> F {
    if p.steer_speed_1 == p.steer_speed_2 {
        p.steer_scale_1
    } else {
        map_clamp(speed, p.steer_speed_1, p.steer_speed_2, p.steer_scale_1, p.steer_scale_2)
    }
}

/// The angle the front wheel is asked for: the rider's lean times the speed fade, times the wheel's
/// own lock — in that order, which is not the order it reads in. No clamp: the lean is already −1 to
/// 1 and the fade is already clamped between the two scales.
pub fn steer_command(max_steer_angle: F, lean: F, scale: F) -> F {
    max_steer_angle * (lean * scale)
}

/// Pose the bike for this frame: where the mass acts, where the front wheel points, and the three
/// flags the chassis wants.
///
/// Two quite different poses. Balancing on one wheel ([`Step::balancing`]): the balance centre of
/// gravity, the front suspension told to hold itself up, and the "wheels are off the ground" flag
/// set whatever the wheels say. Riding: the neutral centre of gravity moved by the stick, the brake
/// and (in [`clutch`], just after) the clutch; the steering command; and the flag from the contact
/// test.
pub fn pose(p: &Params, ch: &mut Chassis, front: &mut Wheel, s: &Step) {
    if s.balancing {
        front.balancing = true;
        ch.cofg = p.balance_cofg;
        ch.flags = (1, 1, 1);
        return;
    }
    front.balancing = false;
    ch.cofg = p.cofg;
    // The travel goes into `z`: the centre of gravity's fore-and-aft lane.
    ch.cofg = ch.cofg.merge(Dest::Z, Vf::new(0.0, 0.0, (F(ch.cofg.z()) + cofg_shift(p, s.stick_y)).0, 0.0));
    if s.brake != 0.0 {
        let z = F(ch.cofg.z()) + brake_cofg_shift(p, s.lean);
        ch.cofg = ch.cofg.merge(Dest::Z, Vf::new(0.0, 0.0, z.0, 0.0));
    }
    front.steer_command = steer_command(front.max_steer_angle, s.lean, steer_scale(p, s.speed));
    ch.flags = (1, 1, i32::from(!s.on_ground));
}

// --- the donut ------------------------------------------------------------------------------------

/// How much of a donut the stick is asking for: 0 to 1, written on the rear wheel, where the tire
/// forces read it to let the back end break away.
///
/// It takes both wheels down, a speed under [`Params::max_donut_speed`], the stick deflected past
/// [`DONUT_DEFLECTION`] and the throttle open. Then the stick's *angle* decides: the scale runs up
/// from nothing at straight ahead to all of it at `donut_angle`, stays there to `-pi/2`, and runs
/// back down to nothing at `-pi`, with the two ramps either side of `-pi/2` handled by separate
/// arithmetic because the angle wraps there. Each ramp is skipped (leaving the end it would have
/// reached) when it is empty.
pub fn donut_roll(p: &Params, ch: &Chassis, s: &Step) -> F {
    if !s.both_wheels || p.max_donut_speed <= ch.speed || s.deflection <= DONUT_DEFLECTION || s.gas == 0.0 {
        return F::ZERO;
    }
    let a = s.stick_angle;
    if a < NEG_HALF_PI {
        let d = p.donut_angle_back;
        if a < d {
            if d == NEG_PI { F::ONE } else { map_clamp(a, d, F(NEG_PI), F::ONE, F::ZERO) }
        } else if d == NEG_HALF_PI {
            F::ZERO
        } else {
            map_clamp(a, F(NEG_HALF_PI), d, F::ZERO, F::ONE)
        }
    } else {
        let d = p.donut_angle;
        if a < d {
            if d == NEG_HALF_PI { F::ONE } else { map_clamp(a, d, F(NEG_HALF_PI), F::ONE, F::ZERO) }
        } else if d == F::ZERO {
            F::ZERO
        } else {
            map_clamp(a, F::ZERO, d, F::ZERO, F::ONE)
        }
    }
}

// --- the suspension's one number, and the gearbox -------------------------------------------------

/// The spring rate for this frame: the stopped rate below [`SPRING_STOPPED`], the moving rate above
/// [`SPRING_MOVING`], interpolated between — so a stationary bike sits on stiffer (or softer, the
/// file decides) springs than a moving one. Both wheels get the same treatment with their own pair.
pub fn spring(speed: F, stopped: F, moving: F) -> F {
    map_clamp(speed, F(SPRING_STOPPED), F(SPRING_MOVING), stopped, moving)
}

/// The engine speed the *road* is asking for, in RPM: how fast the driven wheel must be turning at
/// this road speed, through this gear. Four multiplies in the original's own order, which is not the
/// order they read in: `9.549296 * (ratio * (speed * (1 / radius)))`.
pub fn road_rpm(eng: &Engine, rear: &Wheel, speed: F) -> F {
    F(RPM_PER_RAD) * (eng.ratio * (speed * rear.inv_radius))
}

/// Put the box in gear `g`, which is where the overall ratio and its reciprocal come from. Out of
/// range (below first, above the top gear) it does nothing at all — so the shift logic's own range
/// tests are belt and braces.
pub fn set_gear(eng: &mut Engine, g: i32) {
    if g <= 0 || eng.gears < g {
        return;
    }
    eng.gear = g;
    let r = eng.final_drive * eng.gear_ratios[(g - 1) as usize];
    eng.ratio = r;
    eng.inv_ratio = F::ONE / r;
}

/// The automatic gearbox, once a frame: up a gear above [`Params::up_shift_rpm`] if there is one to
/// go up to, down a gear below [`Params::down_shift_rpm`] if we are above first. Both tests are
/// against [`road_rpm`] and not the engine's actual revs, which is why the box shifts the instant the
/// speed crosses the point rather than waiting for the engine to catch up.
pub fn shift(p: &Params, eng: &mut Engine, rpm: F) {
    if rpm <= p.up_shift_rpm || eng.gears <= eng.gear {
        if rpm < p.down_shift_rpm && 1 < eng.gear {
            set_gear(eng, eng.gear - 1);
        }
    } else {
        set_gear(eng, eng.gear + 1);
    }
}

/// The rev limiter, after the wheel has been integrated: with the clutch locked up, the wheel and
/// the engine turn together, so holding the engine at its limit means *slowing the wheel down to*
/// the limit. With the clutch slipping it does nothing — the engine's own update handles that case.
pub fn engine_post(eng: &Engine, driven: &mut Wheel) {
    if !eng.clutch_locked {
        return;
    }
    if F(RPM_PER_RAD) * driven.spin * eng.ratio > eng.rev_limit {
        driven.spin = F(RAD_PER_RPM) * eng.rev_limit * eng.inv_ratio;
    }
}

// --- the clutch -----------------------------------------------------------------------------------

/// The clutch, once a frame: the ramp that lets a launch make more torque than the engine does.
///
/// Three cases. Clutch in and throttle pinned — the ramp is rearmed (the rider is holding it, ready).
/// Clutch fully out and the ramp not finished — the ramp runs: on the substep it starts, the
/// multiplier is chosen from the speed (all of [`Params::max_clutch_torque_scale`] from a standstill,
/// fading to 1 by [`Params::max_clutch_torque_scale_speed`]) and so is the share of the
/// centre-of-gravity travel; after that the multiplier and the travel both fade to nothing over
/// [`Params::clutch_torque_scale_time`]. Anything else — nothing happens, and the engine keeps
/// whatever multiplier it had.
///
/// Both of the ramp's fades are skipped when their range is empty, which leaves the multiplier at its
/// start and the travel at the file's value rather than at the end of a fade that cannot be computed.
pub fn clutch(p: &Params, r: &mut Ride, ch: &mut Chassis, eng: &mut Engine, s: &Step) {
    if s.clutch == F::ONE && s.gas == F::ONE {
        r.clutch_timer = F::ZERO;
        return;
    }
    if !(s.clutch == F::ZERO && r.clutch_timer <= p.clutch_torque_scale_time) {
        return;
    }
    if r.clutch_timer == F::ZERO {
        r.clutch_scale_start = if p.max_clutch_torque_scale_speed == F::ZERO {
            p.max_clutch_torque_scale
        } else {
            map_clamp(s.speed, F::ZERO, p.max_clutch_torque_scale_speed, p.max_clutch_torque_scale, F::ONE)
        };
        r.clutch_cofg_weight = if p.max_clutch_torque_scale_speed == F::ZERO {
            F::ONE
        } else {
            map_clamp(s.speed, F::ZERO, p.max_clutch_torque_scale_speed, F::ONE, F::ZERO)
        };
    }
    r.clutch_timer += s.dt;
    eng.clutch_torque_scale = if p.clutch_torque_scale_time == F::ZERO {
        r.clutch_scale_start
    } else {
        map_clamp(r.clutch_timer, F::ZERO, p.clutch_torque_scale_time, r.clutch_scale_start, F::ONE)
    };
    let travel = if p.clutch_torque_scale_time == F::ZERO {
        p.clutch_cofg_travel_back
    } else {
        map_clamp(r.clutch_timer, F::ZERO, p.clutch_torque_scale_time, p.clutch_cofg_travel_back, F::ZERO)
    };
    let z = F(ch.cofg.z()) - r.clutch_cofg_weight * travel;
    ch.cofg = ch.cofg.merge(Dest::Z, Vf::new(0.0, 0.0, z.0, 0.0));
}

// --- the substep loop's own arithmetic ------------------------------------------------------------

/// How many substeps this frame gets and how long each one is: `(int)(dt * 55) + 1` of `dt / n`. The
/// count is a truncation, so it is 1 up to 18.2 ms, 2 up to 36.4 ms, and so on — and the `+ 1` is why
/// a frame always steps at least once however small `dt` is.
pub fn substeps(dt: F) -> (i32, F) {
    let n = (F(SUBSTEP_RATE) * dt).to_i32() + 1;
    (n, dt / F(fpu::from_i32(n)))
}

/// Aerodynamic drag: the force against the direction of travel, proportional to the square of the
/// speed. `(0.5 * speed) * speed`, then negated drag times that — the order matters, and it is not
/// the order a decompiler prints. The direction is the *velocity* normalised by VU0's approximate
/// reciprocal square root, so a bike travelling backwards is dragged forwards.
pub fn drag(p: &Params, ch: &Chassis) -> Vf {
    let q = F(HALF) * ch.speed * ch.speed;
    scale(normalize(ch.vel), F(fpu::neg(p.drag.0)) * q)
}

/// The reverse push, as a force in the bike's own frame: proportional to how far below
/// [`Params::reverse_speed`] the bike is, straight down the `z` axis, on top of the
/// [`NOTHING`] constant — so its `w` is 1 and the transform treats it as a point.
///
/// The caller applies it only with the rear wheel down and reverse engaged, because that is where
/// the original tests it.
pub fn reverse_push(p: &Params, speed: F) -> Vf {
    let z = p.reverse_gain * (p.reverse_speed - speed);
    NOTHING.merge(Dest::Z, Vf::new(0.0, 0.0, z.0, 0.0))
}

/// Empty everything a substep is about to refill: the contact flag, the seven scalars and the three
/// quadwords the suspension and the tire forces write. Both wheels, every substep, before the
/// contacts are distributed.
pub fn wheel_clear(w: &mut Wheel) {
    w.contact = false;
    w.fresh = Fresh { scalars: [F::ZERO; 7], vectors: [NOTHING; 3] };
}

/// Keep the largest g load this frame, lane by lane, by *magnitude* — but keep the signed value, so
/// the wreck test can tell a landing from a take-off. The original compares `|new| <= |kept|` and
/// skips, which is this the other way round.
pub fn g_peak(kept: Vf, g: Vf) -> Vf {
    let mut out = kept;
    for (i, dest) in [Dest::X, Dest::Y, Dest::Z].into_iter().enumerate() {
        let (new, old) = (F(g.0[i]), F(out.0[i]));
        if new.abs() > old.abs() {
            out = out.merge(dest, g);
        }
    }
    out
}

// --- the whole frame ------------------------------------------------------------------------------

/// The bike's own state, in one place, so [`step`] takes the same five objects the original reaches
/// through its one pointer.
pub struct Bike<'a> {
    pub ride: &'a mut Ride,
    pub chassis: &'a mut Chassis,
    pub front: &'a mut Wheel,
    pub rear: &'a mut Wheel,
    pub engine: &'a mut Engine,
}

/// **One ground frame.** Pose the bike, pick a gear, work the clutch, then run the substep loop; and
/// return the ordered list of everything that was called along the way.
///
/// Once a frame, before the loop: [`pose`], the donut scale, both [`spring`] rates, the three
/// accumulators cleared to [`NOTHING`], the idle clutch rule, [`shift`] and [`clutch`].
///
/// Then, every substep: gravity into the force; stand-upright, balance (only while balancing), roll
/// and pitch, preload, backflip, the engine; drag into the force; airborne control; the reverse push
/// into the force; both wheels cleared; the contacts distributed (with the *frame's* `dt`); both
/// suspensions; both sets of tire forces; the rear wheel's "other tire is down" flag; both wheels
/// integrated; the rev limiter; the collision record opened; the g loads measured; the body
/// integrated; and the no-hander, unless the bike is being balanced.
///
/// Once a frame, after the loop: the collision record's proximity flag, and the nearby-actor search.
pub fn step(p: &Params, b: &mut Bike<'_>, s: &Step) -> Vec<Call> {
    let mut calls = Vec::new();

    pose(p, b.chassis, b.front, s);
    b.rear.donut_scale = donut_roll(p, b.chassis, s);
    b.front.spring = spring(b.chassis.speed, p.front_spring_when_stopped, p.front_spring);
    b.rear.spring = spring(b.chassis.speed, p.rear_spring_when_stopped, p.rear_spring);
    b.ride.g_peak = NOTHING;
    b.chassis.accel = NOTHING;
    b.chassis.ang_accel = NOTHING;
    // Rolling to a stop off the throttle: pull the clutch in for the rider so the engine does not
    // stall. The *rate-limited* throttle is what is tested, so this does not fire the instant the
    // rider lets go.
    if s.speed < IDLE_SPEED && s.gas_rate_limited == F::ZERO {
        b.ride.clutch_in = F::ONE;
    }
    let rpm = road_rpm(b.engine, b.rear, s.speed);
    shift(p, b.engine, rpm);
    clutch(p, b.ride, b.chassis, b.engine, s);

    let (n, h) = substeps(s.dt);
    for k in 0..n {
        b.chassis.force = add_xyz(b.chassis.force, p.gravity);
        calls.push(Call::StandUpright(h));
        if s.balancing {
            calls.push(Call::Balance(h));
        }
        calls.push(Call::RollPitch(h));
        calls.push(Call::Preload(h));
        calls.push(Call::Backflip(h));
        calls.push(Call::EngineUpdate(h));
        b.chassis.force = add_xyz(b.chassis.force, drag(p, b.chassis));
        calls.push(Call::AirborneControl(h));
        if b.rear.contact && s.reverse != 0.0 {
            let local = reverse_push(p, s.speed);
            if b.chassis.has_transform {
                calls.push(Call::Rotate(local));
                b.chassis.force = add_xyz(b.chassis.force, s.reverse_force);
            }
        }
        wheel_clear(b.front);
        wheel_clear(b.rear);
        calls.push(Call::Contacts(s.dt));
        calls.push(Call::Suspension(Side::Front, h));
        calls.push(Call::Suspension(Side::Rear, h));
        calls.push(Call::TireForces(Side::Front, h));
        calls.push(Call::TireForces(Side::Rear, h));
        b.rear.other_tire_down = b.front.contact && !b.rear.contact;
        calls.push(Call::WheelIntegrate(Side::Front, h));
        calls.push(Call::WheelIntegrate(Side::Rear, h));
        engine_post(b.engine, b.rear);
        calls.push(Call::CollisionBegin(k == 0));
        calls.push(Call::Unrotate(b.chassis.accel));
        b.ride.g_peak = g_peak(b.ride.g_peak, s.local_accel);
        calls.push(Call::Integrate(h));
        if !s.balancing {
            b.ride.no_hander_scratch = [F::ZERO; 4];
            calls.push(Call::NoHander(h));
        }
    }

    b.ride.collision_near = s.node_distance <= COLLISION_NEAR;
    calls.push(Call::SearchReset);
    calls.push(Call::SearchGather);
    calls
}
