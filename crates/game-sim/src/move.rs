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
//! The original's step calls twenty-one other functions, and the biggest of them are each a task's
//! worth of work on their own. So this module ports the *spine* — everything the step does itself,
//! plus the engine behind it ([`engine_update`], S15) and the four rider controllers
//! ([`stand_upright`], [`balance`], [`roll_pitch`], [`no_hander`], S17) — which is where the brief's
//! four questions are actually answered:
//!
//! | The brief | Where it is |
//! |---|---|
//! | turning | [`steer_scale`] and [`steer_command`]: the lean times the wheel's limit, faded out with speed |
//! | acceleration | [`engine_update`] is the torque; [`road_rpm`] and [`shift`] choose the gear, [`clutch`] scales a launch |
//! | friction | [`drag`], the only force the step applies against the direction of travel |
//! | maximum speed | [`drag`] against the engine, with [`engine_post`]'s rev limiter on top of the gear [`shift`] picked |
//! | the update order | [`step`], which returns the ordered list of everything it called ([`Call`]) |
//!
//! The pieces it does not port are not guessed at: [`step`] records each one in the [`Call`] list it
//! returns, in the order and with the substep `dt` the original passes, so the order is proved even
//! where the callee is not. Five of them have since been ported and so have left the list: the
//! engine's own update, which runs for real between [`Call::Backflip`] and the drag, and the four
//! controllers, which run where the list used to say their names. `tests/move.rs` proves the list
//! against the original with every remaining callee hooked, and `research/journal/…-S3-…` and
//! `…-S17-…` list them with the reason each was deferred.
//!
//! What the four controllers *cannot* run for real is the engine's transform code — a rotation, a
//! transpose and the scene graph, all of them behind the engine's lazy transform update. That is the
//! [`Outside`] trait: the one seam this module still has.
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

use crate::air::AcosTable;
use crate::camera::{CurveTable, rotate};
use ps2_float::{Dest, F, Field, Vf, fpu, fpu::Acc, vu};

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
/// How many points the engine's power curve is allowed. The original allocates exactly
/// `NumberOfRPMPoints` floats for it at load time, so this is the port's capacity and not a limit the
/// game has; nothing reads past [`Curve::points`].
pub const MAX_RPM_POINTS: usize = 16;
/// The speed window the wheel springs fade across, in the chassis' own units: stopped below the
/// first, fully moving above the second (the second is exactly twice the first).
const SPRING_STOPPED: f32 = 14.6666;
const SPRING_MOVING: f32 = 29.3332;
/// Below this speed, a rider who is not on the throttle has the clutch pulled in for them — which is
/// what stops the engine stalling when the bike rolls to a halt.
pub const IDLE_SPEED: f32 = 7.3333;
/// How far the stick has to be deflected before a donut counts as commanded.
const DONUT_DEFLECTION: f32 = 0.9;
/// Half, as the drag's own register constant.
const HALF: f32 = 0.5;
/// How close something has to be before the collision record is told to look at it.
pub const COLLISION_NEAR: f32 = 0.25;

/// The speed below which the low-speed upright controller runs, and the end of the window it fades
/// its own force across — the same literal twice, which is what makes that fade a constant 1.
const STAND_UPRIGHT_SPEED: f32 = 10.0;
/// The wheelie's two thresholds: the speed it needs, and how far forward the stick has to be.
const WHEELIE_SPEED: f32 = 5.0;
const WHEELIE_LEAN: f32 = 0.5;
/// Both pitch controllers' integral rate: five hundred times the error, a second.
const PITCH_INTEGRAL_RATE: f32 = 500.0;
/// The stoppie's: the speed and the stick it takes to start one, and the stick and the speed it takes
/// to end one. Starting needs the stick all the way back; it only ends once the stick comes almost
/// all the way forward again, which is the hysteresis that stops a stoppie flickering.
const STOPPIE_SPEED: f32 = 15.0;
const STOPPIE_LEAN: f32 = -1.0;
const STOPPIE_END_LEAN: f32 = -0.1;
const STOPPIE_END_SPEED: f32 = 7.0;
/// How hard a stoppie's yaw command is chased, against the rate the body is already turning at.
const STOPPIE_YAW_GAIN: f32 = 1000.0;
/// How far off vertical a *swept* contact's normal has to be before the no-hander will believe it —
/// the ray's own contact is believed without question.
const SWEPT_NORMAL_UP: f32 = 0.17;

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

    /// How much more a donut rolls the bike than ordinary riding does: the top of the range
    /// [`no_hander`] maps the donut scale onto, times [`Params::max_roll_command`]. Default 1.0.
    pub donut_roll_angle_scale: F,
    /// The no-hander's roll: the most the rider can command, in radians (the file is in degrees), and
    /// the speed that much is reached at. Below it the command is faded in from a standstill.
    pub max_roll_command: F,
    pub max_roll_speed: F,
    /// How fast the commanded roll is chased, per second of error.
    pub roll_gain: F,
    /// The speed window the no-hander fades the ground's normal in over: the upright's own normal
    /// below the first, the ground's own from the second on. The two being equal turns the fade off
    /// and leaves the bike flat.
    pub min_stand_upright_speed: F,
    pub max_stand_upright_speed: F,

    /// Balancing on one wheel, the lateral centre-of-gravity command: the gain on the roll angle
    /// error, the gain on the roll rate, and the most centre of gravity either can ask for.
    pub roll_control_gain: F,
    pub roll_control_rate_gain: F,
    pub roll_control_max_cg: F,
    /// How much of the balance point reaches the roll command as a disturbance — the wobble a rider
    /// balancing on one wheel has to correct.
    pub max_roll_rate_disturbance: F,

    /// The wheelie: the most pitch the stick can ask for (radians, from the file's degrees), the gain
    /// on the pitch error and the gain on the pitch rate, and `1 / WheelieTopSpeed` — the torque
    /// fades linearly to nothing as that speed is reached.
    pub max_pitch_command: F,
    pub pitch_gain: F,
    pub pitch_rate_gain: F,
    pub inv_wheelie_top_speed: F,

    /// The stoppie: the same three, plus how much of the rider's roll lean becomes a yaw rate command
    /// while the back wheel is up, and the speed the pitch command is faded in by.
    pub max_stoppie_pitch_command: F,
    pub stoppie_pitch_gain: F,
    pub stoppie_yaw_rate_scale: F,
    pub stoppie_top_speed: F,

    /// The balance controller's inertia: it builds up at `balance_inertia_rate` a second from the
    /// moment the bike goes up, held inside `min..max`, and is both how fast the balance point
    /// diverges and how hard the stick can move it.
    pub balance_inertia_rate: F,
    pub min_balance_inertia: F,
    pub max_balance_inertia: F,
    pub input_inertia: F,
    /// How far the balance point is allowed to drift either side of upright.
    pub max_balance_point: F,
    /// The rider's own unsteadiness: a noise amplitude on the balance target and the rate the noise
    /// is walked through.
    pub balance_noise_amplitude: F,
    pub balance_noise_time_scale: F,
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
    /// The rate-limited clutch the input layer maintains (`bike+0x65c`), as the engine reads it
    /// through its own pointer. It is a copy and not the original of that word: the word itself is
    /// the input layer's, which walks it toward the rider's lever every frame, and the ground step's
    /// one writer of it — the rule that pulls the clutch in for a rider rolling to a stop off the
    /// throttle — writes the input layer's copy so that the same walk feeds the clutch back *out*
    /// afterwards. See `ride.rs`' idle rule.
    pub clutch_in: F,
    /// The collision record's "something is close" flag, set from the bike's own node once a frame
    /// after the substep loop.
    pub collision_near: bool,
    /// The balance controller's four accumulators, which [`balance`] owns and which [`step`] clears
    /// on every substep the bike is *not* being balanced on one wheel — so going up on one wheel
    /// always starts from a standstill. The original clears them in the order
    /// `+0xa6c`, `+0xa64`, `+0xa4c`, `+0xa60`, which is the order [`step`] writes them in.
    ///
    /// `+0xa4c`: how much inertia the balance has built up since the bike went up.
    pub balance_inertia: F,
    /// `+0xa60`: where the rider is asking the balance point to be, from the stick and the rider's
    /// own unsteadiness, held inside `-1..1`.
    pub balance_command: F,
    /// `+0xa64`: where the balance point actually is — it runs *away* from the command at a rate
    /// set by the inertia, which is what makes balancing a thing the rider has to keep doing.
    pub balance_point: F,
    /// `+0xa6c`: how long the bike has been balanced, which is what walks the noise along.
    pub balance_noise_time: F,
    /// The pitch controller's integral term, shared by the wheelie and the stoppie: `+0x724`. Reset
    /// to zero by whichever of the two is not running.
    pub pitch_integral: F,
    /// `+0x728`: the bike is doing a stoppie. Latched — it takes the brake, the speed and the stick
    /// to start one, and letting any of them go to end it.
    pub stoppie: bool,
}

/// The chassis' rigid body, as far as this step is concerned. Everything here belongs to the chassis
/// and is only reached into — the integrator that consumes `force` is [`Call::Integrate`].
#[derive(Clone, Copy, Debug, Default)]
pub struct Chassis {
    /// Speed along the ground, which the chassis measures: the steering fade, the drag and the donut
    /// test all read this one rather than the bike's own forward speed.
    pub speed: F,
    /// The force accumulator the substep adds into (`+0x130`), and the two acceleration accumulators
    /// it clears once a frame: `+0x170` and `+0x180`. Those two are **not** the body's own `+0x70`
    /// and `+0x80`, which the integrate writes from the force and the torque — they are the *impact*
    /// accumulators, and the only thing that writes them is a suspension bottoming out, which takes
    /// the into-the-ground part out of the body's velocity and records what it took over `dt`
    /// ([`crate::wheel::tire_forces`]). A ride that never bottoms out leaves them at zero all frame.
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
    /// The body's orientation as Euler angles, `+0x90`. Only `x` — the pitch — is read here, by both
    /// halves of [`roll_pitch`].
    pub euler: Vf,
    /// The torque accumulator, `+0x140`, which [`roll_pitch`] adds into exactly as the substep adds
    /// into [`Chassis::force`].
    pub torque: Vf,
    /// Angular velocity twice over: `+0x280` in the world's frame, which [`roll_pitch`] measures the
    /// stoppie's yaw rate against, and `+0x290` in the body's own, whose `x` is the pitch rate the
    /// two pitch controllers damp and whose `z` is the roll rate [`balance`] damps.
    pub world_ang_vel: Vf,
    pub ang_vel: Vf,
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
    /// The torque this wheel has been handed this substep. [`engine_update`] *adds* into it, and the
    /// wheel's own integration (S16's) spends it on the spin and then clears it — so it is an
    /// accumulator and not a value, and nothing in the substep loop resets it.
    pub drive_torque: F,
    /// `TireRadius`, the moment arm the tire's own force acts on. [`Wheel::inv_radius`] is the
    /// reciprocal the loader caches beside it.
    pub radius: F,
    /// This wheel's spin comes from the torque on it rather than from the ground under it. The
    /// wheel's own integration (S16's) is what sets it and what mostly reads it; the engine reads it
    /// to decide where the load on the crank comes from.
    pub spin_from_torque: bool,
    /// The tire's force along the direction of travel, at the contact patch — what the engine feels
    /// back through the gearbox. Written by the tire forces, which are S16's, so it is an input here.
    pub contact_force_fwd: F,
    /// The normal of the ground this tire is on, `+0x130` — in the world's frame, which is why the
    /// three controllers that read it bring it back through a transform first.
    pub contact_normal: Vf,
    /// `+0x38`: this wheel steers under its own torque rather than holding the angle it was
    /// commanded. [`roll_pitch`] sets it on the front wheel — true while neither a wheelie nor a
    /// stoppie is running, false while one is.
    pub steered: bool,
    /// Everything else [`wheel_clear`] empties. See [`Fresh`].
    pub fresh: Fresh,
}

impl Wheel {
    /// `+0xd0` and `+0xd4`, [`Fresh`]'s first two: the suspension ray found ground, and the collision
    /// sweep found a contact. Both are read as *words* by the original (`lw`, `beqz`), which is why
    /// [`Fresh`] keeps them as words and these come back as a `bool`.
    fn ray_hit(&self) -> bool {
        self.fresh.scalars[0] != 0
    }
    fn swept_contact(&self) -> bool {
        self.fresh.scalars[1] != 0
    }
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
    ///
    /// Kept as **words**, not as `F`: two of the seven are object pointers and two more are the flags
    /// [`Wheel::ray_hit`] and [`Wheel::swept_contact`] read, and the original's own `1` in one of
    /// those is a denormal as a float — which the FPU flushes to zero, so an `F` could not carry it.
    /// Nothing ported here reads any of the seven as a number.
    pub scalars: [u32; 7],
    /// The three quadwords it sets to [`NOTHING`]: `+0x170`, `+0x1b0`, `+0x1c0`.
    pub vectors: [Vf; 3],
}

/// A cursor into a table of points, which is how the original reads any of its curves: it remembers
/// where it was last time, so a seek is a step or two rather than a search, and it keeps the
/// interpolation weight beside the index so that reading a second table at the same place is free.
///
/// The weight runs *backwards*: it is the distance from the point at `index` down to the input, over
/// the gap between that point and the one below it. So `0` means "exactly on the point at `index`".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Curve {
    /// How many points the table has, and which one the cursor is sitting on. Both unsigned, as the
    /// original keeps them: the bounds test is an unsigned compare.
    pub points: u32,
    pub index: u32,
    /// The weight, kept as bits so [`Curve`] can be compared for equality. [`Curve::frac`] reads it.
    pub(crate) frac_bits: u32,
}

impl Curve {
    /// A cursor over `points` points, at the bottom of the table. What the loader leaves behind.
    pub fn new(points: u32) -> Curve {
        Curve::at(points, 0, F::ZERO)
    }
    /// A cursor parked anywhere, for a proof that wants to start where the last one finished.
    pub fn at(points: u32, index: u32, frac: F) -> Curve {
        Curve { points, index, frac_bits: frac.to_bits() }
    }
    /// The interpolation weight.
    pub fn frac(self) -> F {
        F::from_bits(self.frac_bits)
    }
    fn set_frac(&mut self, w: F) {
        self.frac_bits = w.to_bits();
    }
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
    /// The rev limit, in RPM. The loader sets it to the *last* point of the power curve, so the
    /// engine is never asked for a torque past the end of its own table.
    pub rev_limit: F,

    /// `IdleThrottle`: added to the rider's throttle before the curve is read, which is what keeps a
    /// closed throttle making a little torque instead of none. The sum is capped at 1 and never
    /// floored, so this is an offset and not a minimum.
    pub idle_throttle: F,
    /// `EngineBraking`: the torque the engine makes *against* its own revs, against their square. It
    /// is taken off twice — once inside the throttle's blend and once outside it — so a shut throttle
    /// leaves exactly `-brake` and a pinned one leaves the curve.
    pub engine_braking: F,
    /// `1 / EngineInertia`, which the loader caches because this is the only thing that reads it:
    /// how fast the revs answer the torque that is left over after the clutch has taken its share.
    pub inv_inertia: F,
    /// `ClutchStaticFriction` and `ClutchKineticFriction`, both scaled by how far *out* the lever is:
    /// the torque a locked clutch will hold before it breaks away, and the torque a slipping one
    /// passes while it is slipping.
    pub clutch_static: F,
    pub clutch_kinetic: F,
    /// `MinRPM`: the idle floor the revs are never allowed below, however hard the engine is braking
    /// or however slowly the road is turning it.
    pub min_rpm: F,

    /// The power curve, as the loader leaves it: the RPM of each point, and the torque at it. The
    /// file gives horsepower (`HP%d` against `RPM%d`) and the loader turns each into a torque with
    /// `hp * 5252 / rpm`, so what is here is already torque.
    pub curve_rpm: [F; MAX_RPM_POINTS],
    pub curve_torque: [F; MAX_RPM_POINTS],
    /// Where the curve lookup got to last time. State, not tuning: it is carried from substep to
    /// substep, and the engine's revs are what walks it.
    pub curve: Curve,

    /// What [`engine_update`] works out, in the order it works it out: the torque the engine is
    /// making, the torque that reaches the driven wheel, the revs, and the revs the road is asking
    /// for through this gear. The last one is kept across the substep because the clutch locks up on
    /// the revs *crossing* it, which needs the previous value as well as this one.
    pub torque: F,
    pub wheel_torque: F,
    pub rpm: F,
    pub road_rpm: F,
    /// The scale on the torque that leaves the gearbox. The engine's constructor sets it to 1 and
    /// nothing on the ported path writes it; see the journal entry for what was and was not found.
    pub torque_scale: F,
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
    /// Throttle, brake and clutch, as the rider is holding them. The clutch is the rider's lever
    /// itself (`+0x644`) rather than the rate-limited copy at `+0x65c`: [`clutch`], the only thing
    /// in this module that reads it, is what the original reads the lever for.
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
    /// [`Chassis::accel`] — the impact accumulator, not the body's acceleration — already turned into
    /// the bike's own frame. The transform that does that belongs to the chassis, so the port takes
    /// the answer as given and records the vector it handed over as [`Call::Unrotate`]. The original
    /// re-reads the accumulator and rotates it again **every substep**, so this is a per-substep
    /// input and not a frame constant: the caller refills it at the [`Call::Unrotate`] below.
    pub local_accel: Vf,
    /// The reverse push's force, already rotated into the world. Likewise the chassis' transform's
    /// work: the port computes the local vector itself, hands it over as [`Call::Rotate`], and adds
    /// this.
    pub reverse_force: Vf,
    /// The field on the bike's own node that the collision record's "look at this" flag is set from.
    pub node_distance: F,
    /// The stick's raw `x`, `+0x628` — before the input layer's rate limit, unlike [`Step::lean`],
    /// which is the same stick after it. [`balance`] is the one thing that reads it raw.
    pub stick_x: F,
    /// The front wheel's brake command, `+0x650`, after the input layer's rate limit. The bike's own
    /// setup hands the front wheel a pointer to this word as its `+0x30`, beside the rear's
    /// [`Step::rear_brake`] and the engine's rate-limited throttle and clutch at `+0x658`/`+0x65c`;
    /// the wheel reads it as the brake its surface drag and tire forces answer to. [`roll_pitch`]
    /// reads this one for the stoppie — the brake that stands the bike on its nose is the front one.
    pub front_brake: F,
    /// The rear wheel's brake command, `+0x654`, the same way round: the rear wheel's `+0x30` points
    /// here, and [`roll_pitch`] reads it for the wheelie's veto — a wheelie wants the driven wheel
    /// free, and says nothing about the front brake.
    pub rear_brake: F,
    /// The rate-limited lean fore and aft, `+0x674`: forward of `0.5` asks for a wheelie, back past
    /// `-1.0` asks for a stoppie. [`Step::lean`] is the same stick sideways.
    pub lean_fore_aft: F,
}

/// Everything the ground step calls and this module does not port, in the order it calls them and
/// with the `dt` it passes. [`step`] returns these as a list, which is how the update order gets
/// proved even where the callee does not.
///
/// Each variant names the original's function; `research/oracle.txt` has the address, and
/// `research/journal/…-S3-…` says why that one was deferred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Call {
    /// `bike_preload` and `bike_backflip` — the preload and the backflip, both ported in `air.rs` (S10).
    Preload(F),
    Backflip(F),
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

/// `a.xyz - b.xyz`, the `vsub.xyz` the three controllers take a vector's component along another with.
/// `w` is `a`'s.
fn sub_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// The dot product of two quadwords' `xyz`, as the FPU's `mula.s`/`madda.s`/`madd.s` chain on three
/// pairs of `lwc1`s — which is how all four controllers here spell it, rather than on the VU.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
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

// --- the engine ------------------------------------------------------------------------------------

/// Walk a curve's cursor to `x`, and work out the weight between the point it lands on and the one
/// below it.
///
/// The original's own two loops: up while the input is still at or above the point the cursor is on,
/// then down while it is below the point beneath it. A cursor already in the right place costs two
/// compares, and one that has drifted by a point costs three — which is why the engine can read its
/// curve every substep for nothing.
///
/// Off the top of the table the cursor parks on the *last* point with a weight of 0, which is what
/// makes a curve flat past its end rather than extrapolated. Both bounds tests are unsigned, as the
/// original keeps the index and the count, so a table with no points at all leaves the index at
/// `points - 1` wrapped round — the original then reads off the end of its own allocation, and this
/// panics instead. The loader cannot produce one (the rev limit is read from the last point), so
/// neither case is reachable.
pub fn curve_seek(cur: &mut Curve, xs: &[F; MAX_RPM_POINTS], x: F) {
    while cur.index < cur.points && !(x < xs[cur.index as usize]) {
        cur.index += 1;
    }
    while cur.index != 0 && x < xs[cur.index as usize - 1] {
        cur.index -= 1;
    }
    if cur.index < cur.points {
        if cur.index == 0 {
            cur.set_frac(F::ZERO);
        } else {
            let hi = xs[cur.index as usize];
            cur.set_frac((hi - x) / (hi - xs[cur.index as usize - 1]));
        }
    } else {
        cur.set_frac(F::ZERO);
        cur.index = cur.points.wrapping_sub(1);
    }
}

/// Read a table at the cursor [`curve_seek`] left: the point the cursor is on, pulled back towards
/// the one below it by the weight. On the bottom point there is nothing to pull towards, so the
/// answer is that point exactly.
///
/// The table is a separate argument from the cursor, which is the whole reason the original keeps the
/// weight: the engine seeks once on the RPM axis and could read any number of curves at that place.
pub fn curve_value(cur: &Curve, ys: &[F; MAX_RPM_POINTS]) -> F {
    if cur.index == 0 {
        ys[0]
    } else {
        let hi = ys[cur.index as usize];
        hi - cur.frac() * (hi - ys[cur.index as usize - 1])
    }
}

/// **The engine, once a substep**: the torque the curve makes at these revs, what the clutch does
/// with it, the torque that reaches the driven wheel, and where the revs go next.
///
/// `throttle` and `clutch` are the two floats the engine reads through the pointers the bike hands it
/// when it is built (`+0x30` and `+0x34`): the rider's throttle and clutch *after* the input layer's
/// rate limits, which is why neither jumps.
///
/// Four things happen, in this order.
///
/// **The curve.** [`curve_seek`] walks the cursor to the current revs and [`curve_value`] reads the
/// torque there. The engine's own braking is `EngineBraking * rpm²`, and it is taken off *twice* —
/// once inside the throttle's blend and once outside it: `(curve + brake) * throttle - brake`. So a
/// shut throttle leaves exactly `-brake` and a pinned one leaves the curve, with the throttle fading
/// between them. [`Engine::idle_throttle`] is added to the rider's throttle first and the sum capped
/// at 1, and the whole thing is scaled by the launch multiplier [`clutch`] worked out.
///
/// **The clutch.** Locked, it passes the engine's torque straight through until either the lever
/// moves off its stop at all or that torque is more than `ClutchStaticFriction` will hold — and then
/// it breaks away in the same substep. Slipping, it passes `ClutchKineticFriction` signed towards
/// whichever end is turning slower, and it locks up again when the engine's revs *cross* the revs the
/// road is asking for. Crossing, not matching: that is why [`Engine::road_rpm`] is kept from the last
/// substep as well as computed for this one. Both frictions are scaled by how far *out* the lever is,
/// so a clutch held in holds nothing.
///
/// **The gearbox.** The shaft torque through the overall ratio, and onto the driven wheel's
/// accumulator — added, not stored, because the brake and the tires put their torques in the same
/// place.
///
/// **The revs.** Locked up, the engine turns with the wheel, so its revs are simply the road's.
/// Slipping, what is left of the engine's torque after the clutch's share accelerates the crank
/// against `EngineInertia`. The exception is a wheel whose spin is driven by its torque rather than
/// by the ground ([`Wheel::spin_from_torque`]): then the load on the crank comes back from the tire's
/// own contact force through the gearbox instead of from the clutch. Either way the revs end up
/// clamped between `MinRPM` and the rev limit — and the clamp applies to the locked path too, so the
/// limiter catches a bike being driven over its limit by the road.
pub fn engine_update(eng: &mut Engine, driven: &mut Wheel, throttle: F, clutch: F, dt: F) {
    curve_seek(&mut eng.curve, &eng.curve_rpm, eng.rpm);
    let curve = curve_value(&eng.curve, &eng.curve_torque);
    let brake = eng.rpm * (eng.engine_braking * eng.rpm);
    let thr = (throttle + eng.idle_throttle).min(F::ONE);
    eng.torque = eng.clutch_torque_scale * ((curve + brake) * thr - brake);

    // Both sides of the crossing test, which is why this one is read before it is overwritten.
    let was_road_rpm = eng.road_rpm;
    eng.road_rpm = (F(RPM_PER_RAD) * driven.spin) * eng.ratio;

    // The torque that reaches the gearbox. Every path below writes it before it is read — including
    // the two that write it in a branch delay slot, which is why a decompiler shows this as an
    // uninitialised read and the S3 journal entry recorded it as one.
    let mut shaft = F::ZERO;
    if eng.clutch_locked {
        if clutch == F::ONE || !(eng.torque.abs() <= eng.clutch_static * (F::ONE - clutch)) {
            eng.clutch_locked = false;
        } else {
            shaft = eng.torque;
        }
    }
    if !eng.clutch_locked {
        let sign = if (eng.rpm - eng.road_rpm) < F::ZERO { -1 } else { 1 };
        shaft = (eng.clutch_kinetic * (F::ONE - clutch)) * F(fpu::from_i32(sign));
        // Lever fully out, and the revs crossed the road's revs from either side.
        if clutch < F::ONE
            && ((was_road_rpm < eng.rpm && eng.rpm < eng.road_rpm)
                || (eng.rpm < was_road_rpm && eng.road_rpm < eng.rpm))
        {
            eng.clutch_locked = true;
            shaft = eng.torque;
        }
    }

    eng.wheel_torque = eng.torque_scale * (shaft * eng.ratio);
    driven.drive_torque = driven.drive_torque + eng.wheel_torque;

    if eng.clutch_locked {
        eng.rpm = eng.road_rpm;
    } else {
        let load = if driven.spin_from_torque {
            eng.inv_ratio * (driven.radius * ((F::ONE - clutch) * driven.contact_force_fwd))
        } else {
            shaft
        };
        eng.rpm = eng.rpm + F(RPM_PER_RAD) * ((eng.inv_inertia * (eng.torque - load)) * dt);
    }
    if eng.rpm < eng.min_rpm {
        eng.rpm = eng.min_rpm;
    } else if eng.rev_limit < eng.rpm {
        eng.rpm = eng.rev_limit;
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
    w.fresh = Fresh { scalars: [0; 7], vectors: [NOTHING; 3] };
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

// --- the four controllers the substep drives ------------------------------------------------------

/// Which frame an [`Outside`] call is about.
///
/// The original hands the engine's transform code a pointer. There are only ever two: the bike's own
/// scene node (`bike+0x18`), which is what the no-hander turns and what both roll angles are measured
/// in, and the chassis' orientation (`chassis+0x254`), which the pitch controller's torque and the
/// upright controller's forward axis come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Frame {
    Node,
    Chassis,
}

/// The engine's transform code, which these four controllers call and which this crate does not port.
///
/// Three of the four are arithmetic — a rotation, a transpose, a table — but all three go through the
/// engine's *lazy* transform update, which can rebuild a chain of parent frames and write outside the
/// bike; and `set_axes` writes the scene graph. So they stay behind a trait: a caller supplies the
/// engine's own, and a proof supplies one that answers the same way the hooked original does.
pub trait Outside {
    /// `transform_unrotate(out, frame, v)` — `v` out of the world and into `frame`, which for a pure
    /// rotation is the transpose. Both roll angles and both of the pitch controller's torques go
    /// through this.
    fn unrotate(&mut self, frame: Frame, v: Vf) -> Vf;
    /// `orientation_axes(frame, 0, out, 0)` — the frame's forward axis (the `z` row of its world
    /// matrix), with `w` of zero. Both callers overwrite that `w` with 1 immediately.
    fn forward(&mut self, frame: Frame) -> Vf;
    /// `smooth_noise(t)` — the engine's one-dimensional smooth noise, which is the rider's own
    /// unsteadiness while balancing.
    fn noise(&mut self, t: F) -> F;
    /// `transform_set_axes(node, 0, forward, up, 1, 1, 1)` — point the bike's node along `forward`
    /// with `up` for its up, orthonormalising as it goes. The no-hander's one output.
    fn set_axes(&mut self, forward: Vf, up: Vf);
}

/// The two tables the engine builds at start-up that these controllers read: the arc cosine both roll
/// angles come out of, and the `{cos, sin}` curve the no-hander's rotation uses.
///
/// They are 30001 and 6001 entries, so they are borrowed rather than carried.
pub struct Tables<'a> {
    pub acos: &'a AcosTable,
    pub curve: &'a CurveTable,
}

/// `[-1, 1]`, as the original's two compares and two moves: *not* `min`/`max`, because a branch on an
/// ordered compare and `min.s` disagree about a NaN. Anything unordered comes back as `1.0`.
fn clamp_unit(c: F) -> F {
    if c <= F::ONE {
        if c < F(-1.0) { F(-1.0) } else { c }
    } else {
        F::ONE
    }
}

/// **How far the ground is off level, under this bike**, in radians, signed: positive one way, negative
/// the other. Both [`balance`] and [`no_hander`] measure it, with the same eleven steps.
///
/// `v` is a ground normal already brought into the bike's own frame. The bike's forward axis is thrown
/// away (`z = 0`) so that what is left is the roll plane; that is normalised twice over — once on the
/// VU, approximately, and then again on the FPU across the two surviving lanes — and the `y` of the
/// result is the cosine of the angle, which the arc cosine table turns back into one. The sign comes
/// from `x`: the ground falling away to one side is the opposite correction from the other.
///
/// The double clamp is the original's. It clamps to `[-1, 1]`, then clamps the clamped value again —
/// two separate pairs of compares, which is what a compiler emits for `clamp(clamp(c))` and is
/// idempotent for everything but the NaN the second one cannot see.
fn ground_roll_angle(v: Vf, acos: &AcosTable) -> F {
    let flat = normalize(v.merge(Dest::Z, Vf::ZERO));
    let (x, y) = (F(flat.x()), F(flat.y()));
    let c = y * F::ONE.rsqrt(Acc::mula(x, x).madd(y, y));
    let angle = acos.at(clamp_unit(clamp_unit(c)));
    // The compare is against the *pre*-clamp `x`, and it is issued before the table lookup overwrites
    // that register — so a clamp cannot change which way the correction goes.
    if x < F::ZERO { F(fpu::neg(angle.0)) } else { angle }
}

/// **The low-speed upright controller** (`bike_stand_upright`): a bike rolling almost to a stop on a
/// slope does not slide sideways down it, and with the brake on does not slide at all.
///
/// It runs only with both wheels down, not while balancing, off the throttle and below ten units of
/// speed — a bike being ridden gets none of this.
///
/// The force it applies is a piece of gravity, cancelled. Take the bike's weight, subtract the part of
/// it pressing into the ground, and what is left is the part trying to slide the bike along the
/// surface. Split *that* along the bike's own forward direction — flattened into the surface and
/// renormalised — and the across-the-bike half is subtracted from the chassis' force. The along-the-bike
/// half is only subtracted when the brake is on, which is the difference between "stands up but rolls
/// down the hill" and "stands still".
///
/// The scale on the sideways half is the original's `map_clamp(speed, 0, 10, 1, 1)`: six float
/// operations whose answer is always exactly one. It is kept because it is what the original computes;
/// see the journal.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // an ordered compare, as in `roll_pitch`
pub fn stand_upright(p: &Params, b: &mut Bike<'_>, s: &Step, o: &mut dyn Outside) {
    if !s.both_wheels || s.balancing || s.gas != F::ZERO || !(s.speed.abs() < F(STAND_UPRIGHT_SPEED)) {
        return;
    }
    let n = b.front.contact_normal;
    let g = p.gravity;
    // The weight, less what the ground is holding up: the slide.
    let slide = sub_xyz(g, scale(n, dot3(n, g)));

    // The bike's forward, flattened into the surface and renormalised. A zero-length answer — the
    // bike pointing straight into the ground — is replaced wholesale by the rodata quadword, which is
    // the same `NOTHING` three other things in this step reset to.
    let mut fwd = o.forward(Frame::Chassis);
    fwd.0[3] = 1.0;
    let fwd = sub_xyz(fwd, scale(n, dot3(fwd, n)));
    let len2 = dot3(fwd, fwd);
    let fwd = if len2 == F::ZERO {
        NOTHING
    } else {
        // Three `mul.s` on the lanes, not a `vmulq.xyz`: `w` keeps the 1 written above.
        let q = F::ONE.rsqrt(len2);
        let mut u = fwd;
        for i in 0..3 {
            u.0[i] = (F(fwd.0[i]) * q).0;
        }
        u
    };

    let along = scale(fwd, dot3(fwd, slide));
    let across = sub_xyz(slide, along);
    let scale_by_speed = map_clamp(s.speed, F::ZERO, F(STAND_UPRIGHT_SPEED), F::ONE, F::ONE);
    let mut force = scale(across, F(fpu::neg(scale_by_speed.0)));
    if s.brake != F::ZERO {
        force = sub_xyz(force, along);
    }
    b.chassis.force = add_xyz(b.chassis.force, force);
}

/// **Balancing on one wheel** (`bike_balance`): the lateral centre-of-gravity command that holds a
/// bike up on its back wheel, and the drift that makes holding it up a skill.
///
/// Four accumulators, all of them [`Ride`]'s and all of them cleared by [`step`] whenever the bike is
/// *not* balancing, so every attempt starts level:
///
/// 1. The **inertia** builds up at a fixed rate from the moment the bike goes up, held inside its two
///    limits. It is both how fast the balance point runs away and how much authority the stick has.
/// 2. The **command** is where the rider is asking the balance point to be: the stick moves it at a
///    rate set by that inertia, the engine's smooth noise nudges it, and it is held inside `-1..1`.
/// 3. The **balance point** chases the command the *wrong way* — `inertia * (point - command)`, a
///    positive feedback — so letting go does not recover, which is the whole mechanic. Held inside
///    `+-MaxBalancePoint`.
/// 4. The **noise time** is just the clock the noise is walked along.
///
/// What comes out is one number: `chassis.cofg.x`. The roll angle of the ground under the front wheel
/// is measured through the arc cosine table ([`ground_roll_angle`]), the balance point is added in as a
/// disturbance, the roll rate damps it, and the lot is clamped to `RollControlMaxCG` and written as the
/// lateral offset of the centre of mass — so the bike is balanced by moving its weight, the way a rider
/// does, and not by a torque out of nowhere.
pub fn balance(p: &Params, b: &mut Bike<'_>, s: &Step, t: &Tables<'_>, o: &mut dyn Outside, dt: F) {
    if !b.front.contact {
        return;
    }
    let r = &mut *b.ride;
    r.balance_inertia = r.balance_inertia + p.balance_inertia_rate * dt;
    r.balance_inertia = r.balance_inertia.max(p.min_balance_inertia).min(p.max_balance_inertia);

    let push = F(fpu::neg(p.input_inertia.0)) * r.balance_inertia;
    r.balance_command = r.balance_command + s.stick_x * push * dt;
    let noise = o.noise(r.balance_noise_time * p.balance_noise_time_scale);
    r.balance_command =
        (r.balance_command + p.balance_noise_amplitude * noise).max(F(-1.0)).min(F::ONE);
    r.balance_noise_time = r.balance_noise_time + dt;

    let point = r.balance_point;
    r.balance_point = point + r.balance_inertia * (point - r.balance_command) * dt;
    r.balance_point =
        r.balance_point.max(F(fpu::neg(p.max_balance_point.0))).min(p.max_balance_point);
    let disturbance = r.balance_point * p.max_roll_rate_disturbance;

    let angle = ground_roll_angle(o.unrotate(Frame::Node, b.front.contact_normal), t.acos);
    let err = p.roll_control_gain * (F::ZERO - angle) - disturbance - F(b.chassis.ang_vel.z());
    let cg = p.roll_control_rate_gain * err;
    let max = p.roll_control_max_cg;
    b.chassis.cofg.0[0] = cg.max(F(fpu::neg(max.0))).min(max).0;
}

/// **The wheelie and the stoppie** (`bike_roll_pitch`): what the stick held forward or back does to the
/// bike's pitch while it is on the ground.
///
/// Airborne, it does nothing but forget any stoppie in progress. On the ground both halves run, in
/// order, every substep — the wheelie's conditions and the stoppie's cannot both hold, but the function
/// does not take that for granted and neither does this.
///
/// **The wheelie** needs the back wheel down, the throttle open, more than five units of speed, no
/// brake, no clutch, and the stick at least half forward. The pitch the stick asks for is mapped onto
/// `+-MaxPitchCommand`; the body's pitch error drives a PI controller — a proportional term on the rate
/// and an integral that accumulates at five hundred times the error a second — and the whole torque
/// fades linearly to nothing as `WheelieTopSpeed` is reached, so a wheelie is a thing you do out of a
/// corner and not at full throttle in top gear. The torque goes into `chassis.torque`'s `x`.
///
/// **The stoppie** latches: the front wheel down, the brake on, over fifteen units of speed and the
/// stick all the way back start one, and it ends when the back wheel touches down, the brake comes off,
/// the stick comes forward of a tenth, or the speed falls below seven. While it is latched the same PI
/// controller runs against `MaxStoppiePitchCommand`, without the speed fade; and on top of it the
/// rider's *sideways* lean becomes a yaw rate command about the back wheel's ground normal, with a gain
/// of a thousand against the rate the body is already turning — which is how a stoppie is steered.
///
/// Either half, while it is running, takes the front wheel off its own steering (`front.steered =
/// false`); neither running hands it back and clears the integral.
// Every test below is the original's branch on an *ordered* compare, which is why they are written
// `!(a <= b)` and not `a > b`: the two differ for a NaN, and the shape of the original is the spec.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
pub fn roll_pitch(p: &Params, b: &mut Bike<'_>, s: &Step, o: &mut dyn Outside, dt: F) {
    if !s.on_ground {
        b.ride.stoppie = false;
        return;
    }
    let lean_fa = s.lean_fore_aft;

    // --- the wheelie
    let wheelie = b.rear.contact
        && !(s.gas_rate_limited <= F::ZERO)
        && !(s.speed <= F(WHEELIE_SPEED))
        && s.rear_brake == F::ZERO
        && b.ride.clutch_in == F::ZERO
        && !(lean_fa < F(WHEELIE_LEAN));
    if wheelie {
        let target = F(2.0) * lean_fa * p.max_pitch_command - p.max_pitch_command;
        let err = p.pitch_gain * (F(b.chassis.euler.x()) - target) - F(b.chassis.ang_vel.x());
        b.ride.pitch_integral = b.ride.pitch_integral + dt * (F(PITCH_INTEGRAL_RATE) * err);
        let fade = (F::ONE - s.speed * p.inv_wheelie_top_speed).min(F::ONE).max(F::ZERO);
        let tx = fade * (b.ride.pitch_integral + p.pitch_rate_gain * err);
        b.chassis.torque = add_xyz(b.chassis.torque, Vf::new(tx.0, 0.0, 0.0, 1.0));

        // The original measures the yaw rate about the back wheel's normal here exactly as the stoppie
        // does — and then hands `qmtc2` a literal zero instead of it. Kept because it is what the
        // original computes; nothing it produces can reach a compared word. See the journal.
        let _dead = dot3(b.chassis.world_ang_vel, b.rear.contact_normal);
        let yaw = scale(b.rear.contact_normal, F::ZERO);
        if b.chassis.has_transform {
            b.chassis.torque = add_xyz(b.chassis.torque, o.unrotate(Frame::Chassis, yaw));
        }
        b.front.steered = false;
    } else {
        b.front.steered = true;
        b.ride.pitch_integral = F::ZERO;
    }

    // --- the stoppie
    let start = b.front.contact
        && !b.ride.stoppie
        && !(s.front_brake <= F::ZERO)
        && !(s.speed <= F(STOPPIE_SPEED))
        && lean_fa <= F(STOPPIE_LEAN);
    if start {
        b.ride.stoppie = true;
    }
    if !b.ride.stoppie {
        b.front.steered = true;
        b.ride.pitch_integral = F::ZERO;
        return;
    }

    let fade = if p.stoppie_top_speed == F::ZERO {
        F::ZERO
    } else {
        map_clamp(s.speed, F::ZERO, p.stoppie_top_speed, F::ZERO, F::ONE)
    };
    let target = fade * (p.max_stoppie_pitch_command + F(2.0) * lean_fa * p.max_stoppie_pitch_command);
    let err = p.stoppie_pitch_gain * (F(b.chassis.euler.x()) - target) - F(b.chassis.ang_vel.x());
    b.ride.pitch_integral = b.ride.pitch_integral + dt * (F(PITCH_INTEGRAL_RATE) * err);
    let tx = b.ride.pitch_integral + p.pitch_rate_gain * err;
    b.chassis.torque = add_xyz(b.chassis.torque, Vf::new(tx.0, 0.0, 0.0, 1.0));

    let rate = dot3(b.chassis.world_ang_vel, b.rear.contact_normal);
    let yaw = scale(
        b.rear.contact_normal,
        F(STOPPIE_YAW_GAIN) * (p.stoppie_yaw_rate_scale * s.lean - rate),
    );
    if b.chassis.has_transform {
        b.chassis.torque = add_xyz(b.chassis.torque, o.unrotate(Frame::Chassis, yaw));
    }
    b.front.steered = false;

    let end = b.rear.contact
        || s.front_brake == F::ZERO
        || !(lean_fa <= F(STOPPIE_END_LEAN))
        || s.speed <= F(STOPPIE_END_SPEED);
    if end {
        b.ride.stoppie = false;
    }
}

/// **The no-hander's roll** (`bike_no_hander`): the bike rolled off level under a rider who has let go
/// of the bars, and the one thing in this module that turns the bike's node rather than pushing on its
/// chassis.
///
/// It needs a tire with real ground under it: a loaded wheel whose suspension ray found something, or
/// whose collision sweep found something more than a little way off vertical. The front is asked first.
///
/// The normal it works from is not the ground's: it is the upright `(0, 1, 0)` *faded towards* the
/// ground's as the bike picks up speed, between `MinStandUprightSpeed` and `MaxStandUprightSpeed`, and
/// faded the other way in reverse. Standing still, the bike is rolled relative to level; at speed, it
/// is rolled relative to the ground it is on.
///
/// Then: measure how far off that the bike already is ([`ground_roll_angle`]), work out how far the
/// rider is asking for — the sideways lean times a roll command that fades in with speed, and that a
/// donut opens right up to `DonutRollAngleScale * MaxRollCommand` — and move the angle towards it at
/// `RollGain` a second. The node's forward axis is the hinge, the faded normal is what gets rotated
/// about it, and the result is handed back as the node's new up.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // an ordered compare, as in `roll_pitch`
pub fn no_hander(p: &Params, b: &mut Bike<'_>, s: &Step, t: &Tables<'_>, o: &mut dyn Outside, dt: F) {
    let grounded = |w: &Wheel| {
        w.contact
            && (w.ray_hit()
                || (w.swept_contact() && !(F(w.contact_normal.y()) <= F(SWEPT_NORMAL_UP))))
    };
    let ground = if grounded(b.front) {
        b.front.contact_normal
    } else if grounded(b.rear) {
        b.rear.contact_normal
    } else {
        return;
    };

    // How far towards the ground's own normal, signed by the direction of travel.
    let backwards = s.speed < F::ZERO;
    let fade = if p.min_stand_upright_speed == p.max_stand_upright_speed {
        F::ZERO
    } else {
        map_clamp(
            s.speed.abs(),
            p.min_stand_upright_speed,
            p.max_stand_upright_speed,
            F::ZERO,
            F::ONE,
        )
    };
    let fade = F(fpu::from_i32(if backwards { -1 } else { 1 })) * fade;

    // The fade, lane by lane with the same three-way test each time — not one `lerp` on a vector,
    // because the original writes it out three times. `w` is the ground normal's.
    let mut normal = ground;
    for (i, up) in [F::ZERO, F::ONE, F::ZERO].into_iter().enumerate() {
        let to = F(ground.0[i]);
        normal.0[i] = if fade <= F::ZERO {
            up
        } else if fade < F::ONE {
            up + fade * (to - up)
        } else {
            to
        }
        .0;
    }

    let angle = ground_roll_angle(o.unrotate(Frame::Node, normal), t.acos);
    let angle = F(fpu::neg(angle.0));

    let mut command = if p.max_roll_speed == F::ZERO {
        F::ZERO
    } else {
        map_clamp(s.speed, F::ZERO, p.max_roll_speed, F::ZERO, p.max_roll_command)
    };
    if !(b.rear.donut_scale <= F::ZERO) {
        // The original guards this map with `0.0 != 1.0`, so the guard never fires: see the journal.
        let full = p.donut_roll_angle_scale * p.max_roll_command;
        command = map_clamp(b.rear.donut_scale, F::ZERO, F::ONE, command, full);
    }

    let angle = angle + p.roll_gain * (command * s.lean - angle) * dt;
    let mut axis = o.forward(Frame::Node);
    axis.0[3] = 1.0;
    let up = rotate(F(fpu::neg(angle.0)), axis, normal, t.curve);
    o.set_axes(axis, up);
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
/// Then, every substep: gravity into the force; [`stand_upright`], [`balance`] (only while
/// balancing), [`roll_pitch`], preload, backflip, the engine; drag into the force; airborne control;
/// the reverse push into the force; both wheels cleared; the contacts distributed (with the *frame's*
/// `dt`); both suspensions; both sets of tire forces; the rear wheel's "other tire is down" flag;
/// both wheels integrated; the rev limiter; the collision record opened; the g loads measured; the
/// body integrated; and — unless the bike is being balanced — the balance controller's four
/// accumulators cleared and [`no_hander`] run.
///
/// Once a frame, after the loop: the collision record's proximity flag, and the nearby-actor search.
pub fn step(
    p: &Params,
    b: &mut Bike<'_>,
    s: &Step,
    t: &Tables<'_>,
    o: &mut dyn Outside,
) -> Vec<Call> {
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
        stand_upright(p, b, s, o);
        if s.balancing {
            balance(p, b, s, t, o, h);
        }
        roll_pitch(p, b, s, o, h);
        calls.push(Call::Preload(h));
        calls.push(Call::Backflip(h));
        engine_update(b.engine, b.rear, s.gas_rate_limited, b.ride.clutch_in, h);
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
            // The balance controller's four accumulators, in the order the original clears them.
            b.ride.balance_noise_time = F::ZERO;
            b.ride.balance_point = F::ZERO;
            b.ride.balance_inertia = F::ZERO;
            b.ride.balance_command = F::ZERO;
            no_hander(p, b, s, t, o, h);
        }
    }

    b.ride.collision_near = s.node_distance <= COLLISION_NEAR;
    calls.push(Call::SearchReset);
    calls.push(Call::SearchGather);
    calls
}
