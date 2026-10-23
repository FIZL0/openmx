//! **The bike in the air**: when the wheels count as off the ground, what the stick can do about it,
//! the preload and the backflip carried off the lip, and the test a landing has to pass.
//!
//! The original runs five steps per substep, in this order, and the order is part of the answer because
//! each one reads flags the previous one wrote:
//!
//! 1. [`contact`] — which wheels touch, how long since they left the ground, and the *crossup*: the
//!    signed angle between where the bike points and where it is going.
//! 2. [`preload`] — stick down then up off a lip: a one-shot force into the chassis.
//! 3. [`backflip`] — stick up then down: a pitch torque that decays while it is held. One row of
//!    [`Rotations`]; see below.
//! 4. [`airborne_control`] — the moment the stick commands in the air, damped or auto-levelled, plus
//!    the lean force that drifts the bike sideways.
//! 5. [`wreck_test`] — the g loads, the pitch and roll limits, and the landing crossup test.
//!
//! Only step 1 runs on the ground; steps 2 and 3 only *arm* on the ground; steps 4 and 5 are where
//! riding in the air comes from.
//!
//! # Rotations: the disc's, and the port's
//!
//! The original has one rotating move, the backflip, with its gesture, axis and sign written into the
//! step. Here a rotation is a [`Rotation`] row and [`rotate`] runs whichever row is asked for, so a
//! frontflip or a 360 is a row someone adds and not a change to the step.
//!
//! *The disc's:* everything [`BACKFLIP`] holds — the gesture (stick `y` past `-HELD` then `+HELD`, on
//! a lip), the negative pitch lane, the six `.phy` keys it reads through [`Params`], the 1.25 floor,
//! the 0.6 s grace, the upright ending and the control-authority ramp — and every rule of the step
//! itself: the lip, the three phases, the decay only while the d-pad is idle, the clamp, `w` set to 1.
//!
//! *The port's:* the shape (the row, [`Axis`], [`StickAxis`], [`Gesture`], [`Tuning`]); choosing
//! among several rows (which one is armed, and asking "fire?" before "arm?", which is the same
//! answer for one row); clearing the other rows' lanes on a fire; and every row in [`INVENTED`],
//! which only joins a set through [`Rotations::new`]'s flag. With the flag off — the default — the
//! set is [`BACKFLIP`] alone and the step is the original's to the bit, which is what the backflip
//! oracle proof runs.
//!
//! # The two state structs
//!
//! The original keeps all of this in one 2.8 KB bike object, interleaved with the drivetrain, the
//! rider, the AI and the audio. Split here into [`Params`] (what the bike's `.phy` file sets, constant
//! for a bike) and [`Air`] (what the air code itself owns and changes), with the chassis' rigid body in
//! [`Chassis`] and everything else the steps read in [`Step`] — so a reader can see at a glance that no
//! step writes anything outside `Air` and the chassis' two accumulators.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use ps2_float::{Dest, F, Vf, fpu, vu};

/// `pi/2`, as the original's instruction stream holds it.
const HALF_PI: f32 = 1.5707964;
/// The angle past which the bike counts as pointing up or down rather than along — 70 degrees. The
/// original's literal is this plus `pi`, because the sine table it looks the angle up in starts at
/// `-pi`, and the lookup happens once at compile time: see [`STEEP_SINE`].
pub const STEEP: f32 = 1.2217305;
/// Below this the stick is not asking for anything: the dead zone on the lean axis and on each
/// angular velocity the auto-levelling damps.
const DEAD: f32 = 0.1;
/// How far the stick has to go before it counts as held for a preload or a backflip.
const HELD: f32 = 0.8;
/// The speed, in the chassis' own units, below which the bike is too slow to measure a crossup angle
/// or to be pushed sideways by the rider's lean.
const SLOW: f32 = 10.0;
/// What a preload or a backflip needs: this much upward velocity and this much forward speed.
const LIP_RISE: f32 = 5.0;
const LIP_SPEED: f32 = 30.0;
/// How long a backflip has to get the wheels off the ground before it gives up — 0.6 s.
const FLIP_GRACE: f32 = 0.6;
/// The two vector constants the original's rodata holds for this code: nothing, and up.
const NOTHING: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);
const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);

/// The bike's own tuning, as its `.phy` file names it. One of these per bike, never changed by the
/// simulation.
///
/// `control_authority_time` is the exception: it is not in the file, it is the 1.0 the bike's
/// constructor writes, and it is the length of the ramp that gives control back after a backflip.
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub airborne_control_moment: Vf,
    pub airborne_damping: F,
    pub airborne_control_force: F,
    pub brake_gyro_torque: F,
    pub unroll_gain: F,
    pub unroll_torque: F,
    pub unyaw_gain: F,
    pub unyaw_torque: F,
    pub control_authority_time: F,

    pub preload_time: F,
    pub max_preload: F,
    pub min_preload_time: F,
    pub max_preload_time: F,
    pub min_dpad_preload_time: F,
    pub max_dpad_preload_time: F,

    pub min_backflip_time: F,
    pub max_backflip_time: F,
    pub min_backflip_torque: F,
    pub max_backflip_torque: F,
    pub backflip_torque_rate_increase: F,
    pub backflip_torque_rate_decrease: F,

    pub front_wreck_speed_limit: F,
    pub back_wreck_speed_limit: F,
    pub side_wreck_speed_limit: F,
    pub racing_down_wreck_speed_limit: F,
    pub freestyle_down_wreck_speed_limit: F,
    pub ground_pitch_down_wreck_angle: F,
    pub ground_pitch_up_wreck_angle: F,
    pub airborne_pitch_down_wreck_angle: F,
    pub airborne_pitch_up_wreck_angle: F,
    pub airborne_roll_wreck_angle: F,
    pub no_hander_wreck_speed_limit: F,
    pub no_hander_max_roll_angle: F,
    pub joust_wreck_height: F,
    pub crossup_wreck: F,
    pub crossup_wreck_time: F,

    /// Not the original's: the rotation-speed modifier ([`RotationSpeed`], P96), which every [`Rotation`] row's
    /// tuning is read through. `None` — what the loader writes — is the original, and touches nothing.
    pub rotation_speed: Option<RotationSpeed>,
}

/// Everything the air code owns: the only state the five steps write, apart from the chassis' two
/// accumulators.
#[derive(Clone, Copy, Debug, Default)]
pub struct Air {
    /// Either wheel touching, both wheels touching, neither — and what `airborne` was last substep,
    /// which is how a landing is noticed.
    pub on_ground: bool,
    pub both_wheels: bool,
    pub airborne: bool,
    pub was_airborne: bool,
    /// Seconds since the wheels left the ground; zeroed on the substep they leave.
    pub hang_time: F,
    /// The velocity the bike is travelling at the contact patch: the touching wheel's, or the
    /// chassis' own in its local frame when neither touches.
    pub contact_vel: Vf,
    /// The crossup angle: how far the bike points away from where it is going, signed, in radians,
    /// zero whenever it cannot be measured (mid-backflip, or below [`SLOW`]).
    pub crossup: F,
    /// The control-authority ramp a finished backflip starts: counts down from
    /// [`Params::control_authority_time`], scaling both the stick's moment and the crossup angle to
    /// nothing and back.
    pub authority: F,

    /// The preload: ready (the lip is right), armed (the stick went down), firing (it went back up).
    pub preload_ready: bool,
    pub preload_firing: bool,
    pub preload_armed: bool,
    /// Counts up while armed — that is the "how long you held it" a preload is measured by — then
    /// down from [`Params::preload_time`] while firing.
    pub preload_timer: F,
    /// The force the preload adds to the chassis each substep while it fires. Only `y` is ever set.
    pub preload_force: Vf,

    /// A backflip torque went into the chassis this substep (so the stick does not also get a say,
    /// and a wreck lands differently).
    pub flip_torque_applied: bool,
    /// A backflip is in the air; set every substep the torque is applied and cleared when it ends.
    pub flipping: bool,
    /// A backflip has been triggered, and (`flip_armed`) is still waiting for the wheels to leave the
    /// ground, or for its grace period to run out.
    pub flip_active: bool,
    pub flip_armed: bool,
    /// Counts down from [`FLIP_GRACE`] while a triggered backflip is still on the ground.
    pub flip_grace: F,
    /// The torque the rotation adds to the chassis. The backflip only ever sets `x`, negative; another
    /// row sets its own [`Axis`] lane with its own sign.
    pub flip_torque: Vf,
    /// The velocity a wreck throws the rider with, when the wreck came from landing on someone.
    pub wreck_impact: Vf,
}

/// The chassis' rigid body, as far as this module can see it: four fields it reads, two accumulators
/// it adds to, and nothing else.
///
/// `force` and `moment` are the accumulators the integrator empties each substep. Every step here adds
/// to their `xyz` and leaves `w` alone, because every one of the original's adds is a `vadd.xyz`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Chassis {
    pub speed: F,
    pub pitch: F,
    pub roll: F,
    pub force: Vf,
    pub moment: Vf,
    pub vel: Vf,
    pub local_vel: Vf,
    pub ang_vel: Vf,
}

/// What the rest of the bike hands the air code for one substep: the wheels, the controller, and the
/// two measurements other subsystems make.
#[derive(Clone, Copy, Debug, Default)]
pub struct Step {
    pub dt: F,
    /// The chassis' forward axis, sampled from its orientation. `y` is how steeply it points up.
    ///
    /// The original takes this and [`Step::up`] off the chassis' transform at the top of [`contact`],
    /// as the third and second rows of its orientation matrix — the port takes them as given, because
    /// the transform belongs to the chassis and not to this module.
    pub heading: Vf,
    /// The chassis' up axis. Its `y` is the cosine of however far the bike is tilted out of level,
    /// which is what the no-hander wreck is measured against.
    pub up: Vf,
    pub front_contact: bool,
    pub rear_contact: bool,
    /// Each wheel's own velocity at its contact patch.
    pub front_vel: Vf,
    pub rear_vel: Vf,
    /// The left stick, `+0x628`/`+0x62c`: the player-range deflection with the d-pad override, then
    /// squared with its sign kept by the input layer's rate limiter (P30) — so [`HELD`] is a stick
    /// pushed past about 0.894. `x` is lean/yaw, `y` is pitch.
    pub stick: (F, F),
    /// `+0x630`, the fore-aft axis's last flick speed in stick travel a second — the player record's
    /// measure of how fast the stick crossed from past 0.8 one way to past 0.8 the other (see
    /// `ride::Flick`), not a time it has been held. The backflip scales its torque by it.
    pub stick_hold: F,
    /// The preload came from the d-pad, which gets its own pair of times because it has no travel.
    pub dpad_y: bool,
    /// `+0x9fc`, the trick buttons' group ([`crate::trick::group`]), zero for none. A backflip only
    /// decays its torque while no trick button is held.
    pub dpad_dir: i32,
    /// The brake, and the two brakes the gyro torque watches.
    pub brake: F,
    pub front_brake: F,
    pub rear_brake: F,
    /// The rider's lean, −1 to 1.
    pub lean: F,
    /// A trick is being held, which halves what the stick may do and silences the lean force.
    pub stunt: bool,
    /// The bike is touching something that is not the ground: a backflip gives up, and a landing
    /// counts as a landing.
    pub touching: bool,
    /// **The port's own (MD16):** R1 is held and no other trick button is — the modifier MX vs.
    /// ATV's air moves are asked for under. Only a [`Rotation`] with [`Rotation::modifier`] reads it,
    /// and only a ride on MX vs. ATV's tricks ever sets it.
    pub modifier: bool,
    /// **The port's own (MD16):** MX vs. ATV's 360 put its torque into the chassis this substep
    /// ([`Spin::applied`]). The airborne control and the wreck test treat it as they treat a
    /// backflip's, and a backflip cannot start under it. False on every ride on the original's tricks.
    pub spinning: bool,
}

/// What the wreck test is looking at outside this module: the g loads the chassis measured, the
/// rider's pose, and whether the bike is touching anything.
#[derive(Clone, Copy, Debug, Default)]
pub struct Loads {
    /// Peak g load this substep in the chassis' own frame: sideways, vertical, fore-and-aft.
    pub side: F,
    pub vertical: F,
    pub fore_aft: F,
    /// The crossup the rider's pose is actually showing, mapped to 0..1 with 0.5 straight — which is
    /// what a landing is judged on, not [`Air::crossup`].
    pub landing_crossup: F,
    /// The rider has let go of the bars — which both lowers the vertical g limit and brings in a
    /// wreck of its own if the bike is tilted ([`Step::up`]).
    pub no_hander: bool,
    /// A racing event rather than freestyle: the two have different vertical g limits.
    pub racing: bool,
    /// The trick the rider is in, the one they asked for, and how far between them they are: a trick
    /// held to the end lands, one abandoned part-way jousts.
    pub pose: i32,
    pub pose_wanted: i32,
    pub pose_blend: F,
    /// The trick system says this landing is already a wreck.
    pub trick_wrecked: bool,
}

/// Why the rider came off. The numbers are the original's own wreck ids, because the animation,
/// the camera and the respawn all switch on them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wreck {
    /// Over the bars.
    Front = 1,
    /// Off to one side.
    Left = 2,
    Right = 3,
    /// Off the back.
    Back = 4,
    /// Landed too hard.
    Flat = 5,
    /// Hit the scenery with the rider: one of the body's rays found something that is not another
    /// actor, and the ray reaches high enough on the bike to be the rider's head ([`hit_wreck`]).
    Scenery = 6,
    /// Hit another rider, or landed on one.
    Joust = 7,
    /// Nose-dived out of the air.
    NoseDive = 8,
    /// Landed on the rear, or never got the trick back.
    Tail = 0xb,
    /// Landed with no hands on the bars.
    NoHander = 0xc,
}

/// The original's inlined "map and clamp": `x` from the range `a..b` onto `out0..out1`, clamped to
/// whichever of the two outputs is smaller and larger — it does not assume they are in order.
///
/// Written out because the compiler inlines this at a dozen call sites and the order of its six
/// operations (two subtractions, a divide, the `adda`/`madd` lerp, a `max` then a `min`) is what makes
/// the last bit come out right. Callers test `a != b` themselves, because what they do when the input
/// range is empty differs from one to the next.
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

/// `a x b`, as the original's `vopmula`/`vopmsub` pair with its operands swapped between the two
/// instructions. `w` is the *first* operand's, which is what the hardware leaves in the destination
/// register, and which nothing here ever reads.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

// --- the table the crossup angle is read out of -----------------------------------------------------

/// How many entries the startup code builds, and the step and scale it builds them with: `x` runs from
/// −1 to nearly 1, and a lookup finds its entry by `(x + 1) * SCALE`.
const ACOS_ENTRIES: usize = 30000;
const ACOS_STEP: f32 = 6.666889e-05;
const ACOS_SCALE: f32 = 14999.5;

/// The arc cosine table the game builds at startup and interpolates for every angle between two
/// directions: 30000 entries from −1 to 1, with the last one duplicated so that a lookup at `x = 1`
/// can read `table[i + 1]` without running off the end.
///
/// It is built by the game's *double-precision* `acos` and then rounded to `f32`, which is why
/// [`acos`] below is `f64`: a `f32` arc cosine gives different last bits and so a different table.
pub struct AcosTable(Box<[f32]>);

impl AcosTable {
    /// Build the table, entry for entry as the startup code does: the index to a float, times the
    /// step, minus one, clamped at the top only — and then the double-precision arc cosine of that.
    pub fn build() -> AcosTable {
        let mut t = vec![0.0f32; ACOS_ENTRIES + 1];
        for (i, e) in t.iter_mut().take(ACOS_ENTRIES).enumerate() {
            let x = F(fpu::add(-1.0, fpu::mul(fpu::from_i32(i as i32), ACOS_STEP)));
            let x = if x <= 1.0 { x } else { F::ONE };
            *e = narrow(acos(widen(x.0)));
        }
        t[ACOS_ENTRIES] = t[ACOS_ENTRIES - 1];
        AcosTable(t.into_boxed_slice())
    }

    /// `acos(x)` for an `x` already in −1..1, by linear interpolation between two entries: the index
    /// and the weight both come out of the same `(x + 1) * scale`, the integer part by `cvt.w.s` and
    /// the fraction by subtracting it back off.
    /// The table itself, for the test that compares it with the one the game builds at startup.
    pub fn entries(&self) -> &[f32] {
        &self.0
    }

    pub fn at(&self, x: F) -> F {
        let s = F(fpu::mul(fpu::add(1.0, x.0), ACOS_SCALE));
        let i = s.to_i32();
        let frac = s - F(fpu::from_i32(i));
        let (lo, hi) = (self.0[i as usize], self.0[i as usize + 1]);
        F(fpu::madd(fpu::mula(fpu::sub(1.0, frac.0), lo), frac.0, hi))
    }
}

/// `f32` to `f64`, which is exact, and back by rounding *towards zero*, which is what the game's
/// soft-float conversion routine does — the R5900 has no double-precision FPU, so both are library
/// calls, and the narrowing one chops. Rounding to nearest instead would put 18595 of the 30001 arc
/// cosine entries one ulp too high.
#[allow(clippy::float_arithmetic)]
pub(crate) fn widen(x: f32) -> f64 {
    x as f64
}
#[allow(clippy::float_arithmetic)]
pub(crate) fn narrow(x: f64) -> f32 {
    let near = x as f32;
    let back = near as f64;
    let away = (back > x && near > 0.0) || (back < x && near < 0.0);
    if away { f32::from_bits(near.to_bits() - 1) } else { near }
}

/// `acos` in double precision, the game's own: fdlibm's `__ieee754_acos`, with the ten polynomial
/// coefficients and the three pieces of `pi` read out of the executable's constant pool rather than
/// copied from fdlibm's source, so that the port stands on what the disc holds. (They turn out to be
/// fdlibm's published values exactly.)
///
/// `f64` is deliberate and is the only place in this crate that uses it (with [`cos_sin`] below, for
/// the same reason): the four basic operations and `sqrt` are specified exactly by IEEE-754 and give
/// the same bits on every host, which is what makes this reproducible, and nothing else here would
/// reproduce the game's table.
#[allow(clippy::float_arithmetic)]
pub(crate) fn acos(x: f64) -> f64 {
    const PS: [u64; 6] = [
        0x3FC5555555555555,
        0xBFD4D61203EB6F7C,
        0x3FC9C1550E884454,
        0xBFA48228B5688F3B,
        0x3F49EFE07501B288,
        0x3F023DE10DFDF709,
    ];
    const QS: [u64; 4] = [0xC0033A271C8A2D4A, 0x40002AE59C598AC7, 0xBFE6066C1B8D0158, 0x3FB3B8C5B12E9281];
    const PIO2_HI: u64 = 0x3FF921FB54442D18;
    const PIO2_LO: u64 = 0x3C91A62633145C07;
    const PI: u64 = 0x400921FB54442D18;
    let (p_s, q_s) = (PS.map(f64::from_bits), QS.map(f64::from_bits));
    let (pio2_hi, pio2_lo, pi) = (f64::from_bits(PIO2_HI), f64::from_bits(PIO2_LO), f64::from_bits(PI));
    // Horner, innermost first, which is the order the game's instructions evaluate it in.
    let rat = |z: f64| {
        let p = z * (p_s[0] + z * (p_s[1] + z * (p_s[2] + z * (p_s[3] + z * (p_s[4] + z * p_s[5])))));
        let q = 1.0 + z * (q_s[0] + z * (q_s[1] + z * (q_s[2] + z * q_s[3])));
        p / q
    };
    let hx = (x.to_bits() >> 32) as u32 as i32;
    let ix = hx & 0x7fff_ffff;
    if ix >= 0x3ff0_0000 {
        // |x| >= 1: exactly 1 is an answer, past 1 is not a number.
        if ix == 0x3ff0_0000 && x.to_bits() as u32 == 0 {
            return if hx > 0 { 0.0 } else { pi };
        }
        return (x - x) / (x - x);
    }
    if ix < 0x3fe0_0000 {
        // |x| < 0.5: the rational approximation of asin, taken off pi/2.
        if ix <= 0x3c60_0000 {
            return pio2_hi;
        }
        let r = rat(x * x);
        return pio2_hi - (x - (pio2_lo - x * r));
    }
    if hx < 0 {
        // x <= -0.5
        let z = (1.0 + x) * 0.5;
        let s = z.sqrt();
        let w = rat(z) * s - pio2_lo;
        return pi - 2.0 * (s + w);
    }
    // x >= 0.5, where the subtraction above would lose bits: split the square root in two.
    let z = (1.0 - x) * 0.5;
    let s = z.sqrt();
    let df = f64::from_bits(s.to_bits() & 0xffff_ffff_0000_0000);
    let c = (z - df * df) / (s + df);
    let w = rat(z) * s + c;
    // `df`, not `s`: the correction `c` is the one `df` needs, so the sum has to be built on `df`.
    2.0 * (df + w)
}

/// `cos` and `sin` in double precision, the game's own (S104): fdlibm 5.3's `cos` and `sin` over its
/// `__kernel_cos`, `__kernel_sin` and `__ieee754_rem_pio2`, which is what the SDK links and what the
/// startup code builds the `{cos, sin}` table with ([`crate::camera::CurveTable::build`]). The same
/// reasons as [`acos`] for `f64`: only the four operations, which every host rounds the same way.
///
/// Only for `|x| <= pi` and a little over, the table's range: the reduction here is fdlibm's for
/// `|x| < 3pi/4` and its medium case at `n = 2`; past that it panics rather than guess at the rest.
#[allow(clippy::float_arithmetic)]
pub(crate) fn cos_sin(x: f64) -> (f64, f64) {
    let ix = high(x) & 0x7fff_ffff;
    if ix < 0x3fe9_21fc {
        return (kernel_cos(x, 0.0), kernel_sin(x, 0.0, false));
    }
    let (n, y0, y1) = rem_pio2(x);
    // `0 - v`, as the game negates, rather than `-v`.
    match n & 3 {
        0 => (kernel_cos(y0, y1), kernel_sin(y0, y1, true)),
        1 => (0.0 - kernel_sin(y0, y1, true), kernel_cos(y0, y1)),
        2 => (0.0 - kernel_cos(y0, y1), 0.0 - kernel_sin(y0, y1, true)),
        _ => (kernel_sin(y0, y1, true), 0.0 - kernel_cos(y0, y1)),
    }
}

/// The high word of a double, which fdlibm's tests are all made on.
fn high(x: f64) -> u32 {
    (x.to_bits() >> 32) as u32
}

/// fdlibm's `__kernel_cos(x, y)`: the cosine of `x + y` for `|x| <= pi/4`, `y` the tail of a reduced
/// argument.
#[allow(clippy::float_arithmetic)]
fn kernel_cos(x: f64, y: f64) -> f64 {
    const C: [u64; 6] = [
        0x3FA555555555554C,
        0xBF56C16C16C15177,
        0x3EFA01A019CB1590,
        0xBE927E4F809C52AD,
        0x3E21EE9EBDB4B1C4,
        0xBDA8FAE9BE8838D4,
    ];
    let c = C.map(f64::from_bits);
    let ix = high(x) & 0x7fff_ffff;
    // `(int) x == 0`, which a value this small always is.
    if ix < 0x3e40_0000 {
        return 1.0;
    }
    let z = x * x;
    let r = z * (c[0] + z * (c[1] + z * (c[2] + z * (c[3] + z * (c[4] + z * c[5])))));
    if ix < 0x3fd3_3333 {
        return 1.0 - (0.5 * z - (z * r - x * y));
    }
    let qx = if ix > 0x3fe9_0000 { 0.28125 } else { f64::from_bits(u64::from(ix - 0x0020_0000) << 32) };
    let hz = 0.5 * z - qx;
    let a = 1.0 - qx;
    a - (hz - (z * r - x * y))
}

/// fdlibm's `__kernel_sin(x, y, iy)`: the sine of `x + y` for `|x| <= pi/4`; `tail` says whether `y`
/// is one (fdlibm's `iy`).
#[allow(clippy::float_arithmetic)]
fn kernel_sin(x: f64, y: f64, tail: bool) -> f64 {
    const S: [u64; 6] = [
        0xBFC5555555555549,
        0x3F8111111110F8A6,
        0xBF2A01A019C161D5,
        0x3EC71DE357B1FE7D,
        0xBE5AE5E68A2B9CEB,
        0x3DE5D93A5ACFD57C,
    ];
    let s = S.map(f64::from_bits);
    let ix = high(x) & 0x7fff_ffff;
    if ix < 0x3e40_0000 {
        return x;
    }
    let z = x * x;
    let v = z * x;
    let r = s[1] + z * (s[2] + z * (s[3] + z * (s[4] + z * s[5])));
    if !tail {
        x + v * (s[0] + z * r)
    } else {
        x - ((z * (0.5 * y - v * r) - y) - v * s[0])
    }
}

/// fdlibm's `__ieee754_rem_pio2(x)` for `pi/4 < |x| <= pi`: `x - n pi/2` as a head and a tail, and
/// `n`. Under `3pi/4` the one-step case; above it the medium case, which here is always `n = 2`, with
/// the second and third pieces of `pi/2` taken in where the first cancels too much.
#[allow(clippy::float_arithmetic)]
fn rem_pio2(x: f64) -> (i32, f64, f64) {
    const INVPIO2: u64 = 0x3FE45F306DC9C883;
    const PIO2: [(u64, u64); 3] = [
        (0x3FF921FB54400000, 0x3DD0B4611A626331),
        (0x3DD0B4611A600000, 0x3BA3198A2E037073),
        (0x3BA3198A2E000000, 0x397B839A252049C1),
    ];
    let pio2 = PIO2.map(|(h, t)| (f64::from_bits(h), f64::from_bits(t)));
    let hx = high(x) as i32;
    let ix = hx & 0x7fff_ffff;
    if ix < 0x4002_d97c {
        let (mut z, mut tail) = (x, pio2[0].1);
        if hx > 0 {
            z = z - pio2[0].0;
            if ix == 0x3ff9_21fb {
                z = z - pio2[1].0;
                tail = pio2[1].1;
            }
            let y0 = z - tail;
            return (1, y0, (z - y0) - tail);
        }
        z = z + pio2[0].0;
        if ix == 0x3ff9_21fb {
            z = z + pio2[1].0;
            tail = pio2[1].1;
        }
        let y0 = z + tail;
        return (-1, y0, (z - y0) + tail);
    }
    let t = x.abs();
    let n = (t * f64::from_bits(INVPIO2) + 0.5) as i32;
    assert_eq!(n, 2, "rem_pio2 is ported for the curve table's angles only, and {x} is not one");
    let fln = f64::from(n);
    let mut r = t - fln * pio2[0].0;
    let mut w = fln * pio2[0].1;
    let mut y0 = r - w;
    // fdlibm's `npio2_hw[n - 1]`: the high word of `n pi/2`, near which the first piece cancels.
    if ix == 0x4009_21fb {
        let j = ix >> 20;
        let exponent = |y: f64| ((high(y) >> 20) & 0x7ff) as i32;
        for (piece, (head, tail)) in pio2.iter().enumerate().skip(1) {
            if j - exponent(y0) <= [16, 49][piece - 1] {
                break;
            }
            let t = r;
            w = fln * head;
            r = t - w;
            w = fln * tail - ((t - r) - w);
            y0 = r - w;
        }
    }
    let y1 = (r - y0) - w;
    if hx < 0 { (-n, 0.0 - y0, 0.0 - y1) } else { (n, y0, y1) }
}

/// The sine of [`STEEP`], as the game's startup sine table holds it — `0.9393` where the real sine of
/// 70 degrees is `0.9397`, because the table is 6000 entries around the circle and the index is
/// truncated, so the entry that answers for 70 degrees is a fifteenth of a degree short of it.
///
/// The original looks this up every substep: it wraps the angle into `-pi..pi`, scales `pi + angle` by
/// the table's index scale, truncates, and reads the entry's sine half. All three of those are
/// startup constants, so the answer never changes and the port holds it as one — which is what
/// `steep_sine_is_the_curve_tables_entry` checks, by building the table with the game's own code and
/// indexing it the same way.
pub const STEEP_SINE: f32 = 0.93934375;

// --- 1: which wheels touch, and which way the bike points ------------------------------------------

/// The contact step: the wheels, the hang time, the velocity at the contact patch and the crossup
/// angle. The only one of the five that runs whether the bike is in the air or not.
///
/// The crossup is the signed angle between the heading flattened into the horizontal plane and the
/// velocity flattened the same way. It comes out of a cross product rather than a dot: the sine of the
/// angle, through the arc cosine table at `pi/2 - acos(sin)`, which is the arc *sine* and so is signed
/// and bounded at a quarter turn either way. The sign is the cross product's `y` — which way round the
/// bike is sideways. It is only measured while the bike is not mid-backflip and is moving faster than
/// [`SLOW`]; otherwise it is zero.
pub fn contact(p: &Params, air: &mut Air, chassis: &Chassis, step: &Step, table: &AcosTable) {
    air.was_airborne = air.airborne;
    air.on_ground = step.front_contact || step.rear_contact;
    air.both_wheels = step.front_contact && step.rear_contact;
    air.airborne = !air.on_ground;
    if !air.was_airborne && air.airborne {
        air.hang_time = F::ZERO;
    }
    if air.airborne {
        air.hang_time += step.dt;
    }
    air.contact_vel = if step.front_contact {
        step.front_vel
    } else if step.rear_contact {
        step.rear_vel
    } else {
        chassis.local_vel
    };

    // The heading, flattened and normalised. `w` is 1 in both of these because the original builds
    // them on the stack a lane at a time and leaves the 1 it put there.
    let heading = normalize(Vf::new(step.heading.x(), 0.0, step.heading.z(), 1.0));
    let mut angle = F::ZERO;
    if !air.flipping && chassis.speed > SLOW {
        let vel = normalize(Vf::new(chassis.vel.x(), 0.0, chassis.vel.z(), 1.0));
        let c = cross(heading, vel);
        // The length of the cross product, through the FPU accumulator in x, y, z order. The three
        // clamps are the original's: it inlines the same clamp three times over.
        let sin = F(fpu::sqrt(fpu::madd(fpu::madda(fpu::mula(c.x(), c.x()), c.y(), c.y()), c.z(), c.z())));
        let sin = sin.clamp(-1.0, 1.0).clamp(-1.0, 1.0).clamp(-1.0, 1.0);
        angle = F(HALF_PI) - table.at(sin);
        if c.y() < 0.0 {
            angle = angle * -1.0;
        }
    }
    // While control is ramping back in after a backflip, the crossup ramps with it.
    air.crossup = if air.authority > 0.0 {
        if p.control_authority_time == F::ZERO {
            F::ZERO
        } else {
            map_clamp(air.authority, p.control_authority_time, F::ZERO, F::ZERO, angle)
        }
    } else {
        angle
    };
}

// --- 2: the preload ---------------------------------------------------------------------------------

/// Is the bike on a lip? Both the preload and the backflip want the same thing: a wheel on the ground,
/// the chassis already rising, real speed, and no brake.
fn on_a_lip(air: &Air, chassis: &Chassis, step: &Step) -> bool {
    air.on_ground && chassis.vel.y() > LIP_RISE && air.contact_vel.z() > LIP_SPEED && step.brake == 0.0
}

/// The preload step: push down on the lip and let go, and the suspension throws the bike up.
///
/// Pull the stick back (`y` above [`HELD`]) while the lip conditions hold and it arms, with the timer
/// counting up; push it forward again and it fires, the timer reloading with
/// [`Params::preload_time`]. [`Step::stick_hold`], mapped through the Min/Max preload times onto
/// 0..[`Params::max_preload`], is the force — and the d-pad gets its own pair of times, because it
/// cannot be held half way. Despite the keys' names that is not how long the stick was held but how
/// fast it last crossed (P30, `ride::Flick`).
///
/// While it fires the same force goes into the chassis every substep until the timer runs out, which
/// is why a preload is a shove and not an impulse.
pub fn preload(p: &Params, air: &mut Air, chassis: &mut Chassis, step: &Step) {
    if air.preload_firing {
        air.preload_timer = air.preload_timer - step.dt;
        if air.preload_timer > 0.0 {
            chassis.force = chassis.force.merge(Dest::XYZ, chassis.force.add(air.preload_force));
        } else {
            air.preload_firing = false;
        }
        return;
    }
    if !on_a_lip(air, chassis, step) {
        air.preload_ready = false;
        return;
    }
    air.preload_ready = true;
    if step.stick.1 > HELD {
        air.preload_armed = true;
        air.preload_timer = F::ZERO;
    } else if step.stick.1 < -HELD && air.preload_armed {
        let (from, to) = if step.dpad_y {
            (p.min_dpad_preload_time, p.max_dpad_preload_time)
        } else {
            (p.min_preload_time, p.max_preload_time)
        };
        // An empty range gives nothing at all, which is what the original's uninitialised zero does.
        let force = if from == to {
            F::ZERO
        } else {
            map_clamp(step.stick_hold.abs(), from, to, F::ZERO, p.max_preload)
        };
        air.preload_force = Vf::new(air.preload_force.x(), force.0, air.preload_force.z(), air.preload_force.w());
        air.preload_timer = p.preload_time;
        air.preload_armed = false;
        air.preload_firing = true;
    } else if air.preload_armed {
        air.preload_timer += step.dt;
    }
}

// --- 3: the backflip, and any other rotation ---------------------------------------------------------

/// Which of the moment's three lanes a rotation turns the bike about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    /// `x`: nose up or down — the backflip's.
    Pitch,
    /// `y`: about the vertical.
    Yaw,
    /// `z`: about the bike's own length.
    Roll,
}

/// Which half of the left stick ([`Step::stick`]) a rotation is triggered and steered with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StickAxis {
    /// `x`, the lean.
    Lean,
    /// `y`, the pitch — the backflip's.
    Pitch,
}

/// How a rotation is asked for: the stick past [`HELD`] one way on the lip to arm it, then past it the
/// other way to fire it. The same axis, read with the firing direction counted as positive, is what
/// steers the torque while the rotation turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gesture {
    pub stick: StickAxis,
    /// The direction that fires: `true` for past `+HELD` (the backflip's, after arming past `-HELD`).
    pub fire_positive: bool,
}

/// The six numbers a rotation is tuned by, as a bike's `.phy` gives them: how long the stick may be
/// held, the torque that maps onto, and the two rates it decays at — the first while the stick is
/// pushed the firing way, the second otherwise.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    pub min_time: F,
    pub max_time: F,
    pub min_torque: F,
    pub max_torque: F,
    pub rate_increase: F,
    pub rate_decrease: F,
}

/// One rotating move, as data. [`rotate`] is the one step that runs any of them, and the disc's
/// backflip is [`BACKFLIP`]; a new rotation is a new row, not a change to the step.
#[derive(Clone, Copy, Debug)]
pub struct Rotation {
    pub name: &'static str,
    pub gesture: Gesture,
    /// The lane of [`Air::flip_torque`] (and so of the chassis' moment) the torque goes into …
    pub axis: Axis,
    /// … and its sign there. The backflip's is negative.
    pub negative: bool,
    /// Where the six tuning numbers come from. A function of [`Params`] so that the backflip reads the
    /// very fields it always has, and a new row can read others or hold constants.
    pub tuning: fn(&Params) -> Tuning,
    /// The fire torque's floor is the minimum torque times this — the backflip's 1.25.
    pub floor: f32,
    /// Seconds a fired rotation has to get the wheels off the ground — the backflip's [`FLIP_GRACE`].
    pub grace: f32,
    /// What gives control back: once the torque has stopped, is the bike the right way up? The wheels
    /// touching gives it back regardless.
    pub over: fn(&Params, &Air, &Chassis) -> bool,
    /// The length of the control-authority ramp the end of the rotation starts.
    pub authority: fn(&Params) -> F,
    /// **The port's own (MD16):** the row is only armed and fired while [`Step::modifier`] holds —
    /// R1 alone. False for the disc's backflip, which reads the stick with or without it.
    pub modifier: bool,
}

impl Rotation {
    /// The row's tuning as the step reads it: [`Rotation::tuning`], through [`Params::rotation_speed`] when the
    /// rotation-speed modifier is on. This is the one place a row's numbers are read, so a row added
    /// later turns at the modifier's speed with nothing of its own to do.
    pub fn tuned(&self, p: &Params) -> Tuning {
        let t = (self.tuning)(p);
        match p.rotation_speed {
            None => t,
            Some(s) => s.tuning(t),
        }
    }
}

/// The disc's backflip, and the only rotation the original has: stick forward on the lip, then back,
/// and a negative pitch torque.
pub const BACKFLIP: Rotation = Rotation {
    name: "backflip",
    gesture: Gesture { stick: StickAxis::Pitch, fire_positive: true },
    axis: Axis::Pitch,
    negative: true,
    tuning: |p| Tuning {
        min_time: p.min_backflip_time,
        max_time: p.max_backflip_time,
        min_torque: p.min_backflip_torque,
        max_torque: p.max_backflip_torque,
        rate_increase: p.backflip_torque_rate_increase,
        rate_decrease: p.backflip_torque_rate_decrease,
    },
    floor: 1.25,
    grace: FLIP_GRACE,
    over: upright,
    authority: |p| p.control_authority_time,
    modifier: false,
};

/// **Not the original's.** A flat 360 about the vertical: lean left on the lip, then right. It borrows
/// the backflip's tuning, grace and ending, because the disc has no numbers for a spin; only
/// [`Rotations::new`] with its flag on lets it into a set.
pub const FLAT_SPIN: Rotation = Rotation {
    name: "flat spin",
    gesture: Gesture { stick: StickAxis::Lean, fire_positive: true },
    axis: Axis::Yaw,
    negative: false,
    ..BACKFLIP
};

/// The rotations the port adds on top of the disc's. None of them is the original's, and none of them
/// is in a set unless [`Rotations::new`]'s flag asks for it.
pub const INVENTED: &[Rotation] = &[FLAT_SPIN];

/// **Not the original's.** How fast everything turns in the air: one factor `k` that every rotating
/// move reads (the rotation-speed modifier, P96).
///
/// It speeds the air's rotation up in time rather than pushing it harder: whatever the bike would
/// have turned through in `t` seconds it turns through in `t / k`, at `k` times the angular velocity.
/// In the air the turning is `I w' = M - D w` — a moment the stick or a rotation applies against
/// damping proportional to the spin — and that is the same motion `k` times as fast when every moment
/// is `k^2` times as big ([`RotationSpeed::moment`]), every term against the angular velocity `k` times
/// ([`RotationSpeed::damping`]), every moment that ramps ramps `k^3` times as fast ([`RotationSpeed::moment_rate`]),
/// every window on the angular velocity is `k` times as wide ([`RotationSpeed::rate`]) and every clock that
/// times a rotation runs `k` times as fast ([`RotationSpeed::time`]). The flight itself — gravity, the jump's
/// height and the hang time — is not sped up, so a rotation that takes a whole jump at 100% has room
/// to spare at 150%. The inertia is left alone because the ground's contacts turn the chassis through
/// it too.
///
/// Where it is read:
/// - every [`Rotation`] row, through [`Rotation::tuned`] — the backflip, the port's front flip and
///   flat spin, and any row added later, with no change here;
/// - MX vs. ATV's 360 off a trick pack, through [`SpinTuning::tuned`] at the top of [`three_sixty`];
/// - the stick's own rotation in the air (front flips and back by pitching, 360s by yawing, rolls),
///   its damping, its auto-levelling and the brake's gyro, through [`RotationSpeed::params`];
/// - the chassis' angular windows, through [`RotationSpeed::rate`] where the ride is built
///   (`modifiers::Mods::ride`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotationSpeed(pub F);

impl RotationSpeed {
    /// A moment: `k^2`.
    pub fn moment(self, m: F) -> F {
        m * (self.0 * self.0)
    }

    /// A moment's rate of change, per second: `k^3`.
    pub fn moment_rate(self, r: F) -> F {
        r * (self.0 * self.0 * self.0)
    }

    /// A gain against the angular velocity: `k`.
    pub fn damping(self, d: F) -> F {
        d * self.0
    }

    /// An angular velocity: `k`.
    pub fn rate(self, w: F) -> F {
        w * self.0
    }

    /// A time a rotation runs for: `1 / k`.
    pub fn time(self, t: F) -> F {
        t / self.0
    }

    /// A rotation row's tuning. The stick's times are not touched: they are how fast the stick was
    /// flicked, which picks the torque, and not how long anything turns.
    pub fn tuning(self, t: Tuning) -> Tuning {
        Tuning {
            min_torque: self.moment(t.min_torque),
            max_torque: self.moment(t.max_torque),
            rate_increase: self.moment_rate(t.rate_increase),
            rate_decrease: self.moment_rate(t.rate_decrease),
            ..t
        }
    }

    /// MX vs. ATV's 360's `[Three Sixty]` keys, as a row's: torques `k^2`, their rates `k^3`, and the
    /// flick times left alone for the same reason.
    pub fn three_sixty(self, t: SpinTuning) -> SpinTuning {
        SpinTuning {
            min_torque_x: self.moment(t.min_torque_x),
            max_torque_x: self.moment(t.max_torque_x),
            min_torque_y: self.moment(t.min_torque_y),
            max_torque_y: self.moment(t.max_torque_y),
            rate_increase: self.moment_rate(t.rate_increase),
            rate_decrease: self.moment_rate(t.rate_decrease),
            ..t
        }
    }

    /// The bike's air numbers, once, when its file is loaded: the stick's moments, the damping, the
    /// auto-levelling (each gain `k`, so its moment per radian is `k^2` and its moment per radian a
    /// second `k`), the brake's gyro and the control-authority ramp; and the factor itself, kept for
    /// the rows. The backflip's own six keys are left as the file has them — [`Rotation::tuned`]
    /// scales whatever a row reads, so scaling them here as well would count them twice.
    pub fn params(self, p: &mut Params) {
        let acm = p.airborne_control_moment;
        p.airborne_control_moment = acm.merge(Dest::XYZ, acm.mul_q((self.0 * self.0).0));
        p.brake_gyro_torque = self.moment(p.brake_gyro_torque);
        p.airborne_damping = self.damping(p.airborne_damping);
        p.unroll_gain = self.damping(p.unroll_gain);
        p.unroll_torque = self.damping(p.unroll_torque);
        p.unyaw_gain = self.damping(p.unyaw_gain);
        p.unyaw_torque = self.damping(p.unyaw_torque);
        p.control_authority_time = self.time(p.control_authority_time);
        p.rotation_speed = Some(self);
    }
}

/// The backflip's ending: roll under a quarter turn and pitch inside the airborne wreck angles, with
/// no torque applied this substep.
fn upright(p: &Params, air: &Air, chassis: &Chassis) -> bool {
    !air.flip_torque_applied
        && chassis.roll.abs() < HALF_PI
        && chassis.pitch > -p.airborne_pitch_down_wreck_angle
        && chassis.pitch < p.airborne_pitch_up_wreck_angle
}

/// The rotations a bike can do, and which of them [`Air`]'s flip state belongs to.
///
/// The default is the disc's set, the backflip alone, with which [`Rotations::step`] is the original's
/// backflip step exactly. `active` is the port's: the original has one rotation and so nothing to
/// remember about which.
#[derive(Clone, Debug)]
pub struct Rotations {
    rows: Vec<Rotation>,
    active: usize,
}

impl Default for Rotations {
    fn default() -> Rotations {
        Rotations::new(false)
    }
}

impl Rotations {
    /// The disc's set, plus [`INVENTED`] when `invented` is on. Off is the original.
    pub fn new(invented: bool) -> Rotations {
        let mut rows = vec![BACKFLIP];
        if invented {
            rows.extend_from_slice(INVENTED);
        }
        Rotations::of(rows)
    }

    /// Any set at all, for someone adding a row. The first row is the one an idle bike starts on.
    pub fn of(rows: Vec<Rotation>) -> Rotations {
        assert!(!rows.is_empty(), "a set of rotations needs at least one");
        Rotations { rows, active: 0 }
    }

    pub fn rows(&self) -> &[Rotation] {
        &self.rows
    }

    /// The rotation armed, turning, or last turned.
    pub fn active(&self) -> &Rotation {
        &self.rows[self.active]
    }

    /// One substep of whichever rotation is asked for. See [`rotate`].
    pub fn step(&mut self, p: &Params, air: &mut Air, chassis: &mut Chassis, step: &Step) {
        rotate(&self.rows, &mut self.active, p, air, chassis, step);
    }
}

fn lane(v: Vf, axis: Axis) -> F {
    F(match axis {
        Axis::Pitch => v.x(),
        Axis::Yaw => v.y(),
        Axis::Roll => v.z(),
    })
}

fn with_lane(v: Vf, axis: Axis, x: F) -> Vf {
    match axis {
        Axis::Pitch => Vf::new(x.0, v.y(), v.z(), v.w()),
        Axis::Yaw => Vf::new(v.x(), x.0, v.z(), v.w()),
        Axis::Roll => Vf::new(v.x(), v.y(), x.0, v.w()),
    }
}

/// The stick along a gesture, with the firing direction positive. Negation is exact, so for the
/// backflip this is the original's `stick.y` bit for bit.
fn along(g: Gesture, step: &Step) -> F {
    let s = match g.stick {
        StickAxis::Lean => step.stick.0,
        StickAxis::Pitch => step.stick.1,
    };
    if g.fire_positive { s } else { -s }
}

/// The backflip step: push the stick forward on the lip and pull it back, and the bike goes over
/// backwards. The disc's set run through [`rotate`]; this is the function the oracle proves.
pub fn backflip(p: &Params, air: &mut Air, chassis: &mut Chassis, step: &Step) {
    rotate(&[BACKFLIP], &mut 0, p, air, chassis, step);
}

/// The rotation step, for any set of rows.
///
/// It runs in three phases. *Triggered* — the torque is chosen from [`Step::stick_hold`] (how fast
/// the stick last crossed, P30), between the floor times the minimum and the maximum, and the bike has the row's grace to get
/// airborne or the rotation is cancelled. *Turning* — the torque goes into the chassis every substep
/// and decays towards zero at a rate the stick controls (one rate pushing the firing way, another
/// otherwise), but only while the d-pad is not asking for a trick; it ends when a wheel touches, when
/// the bike hits something, or when the torque has decayed below the minimum. *Over* — the
/// control-authority ramp starts, either because the bike is back on the ground or because the row's
/// ending says so, which is how a flip that comes round cleanly gives the stick back sooner than one
/// that does not.
///
/// With more than one row, an idle bike arms whichever row's gesture comes first in the set, and fires
/// the row it armed. The original asks "arm?" before "fire?"; with one row the two cannot both hold
/// (one wants the stick past `-HELD`, the other past `+HELD`), so asking "fire the armed row?" first is
/// the same answer — and it is the order that lets two rows share a stick axis in opposite directions.
fn rotate(rows: &[Rotation], active: &mut usize, p: &Params, air: &mut Air, chassis: &mut Chassis, step: &Step) {
    air.flip_torque_applied = false;
    if !air.flip_active {
        let armed = &rows[*active];
        if !on_a_lip(air, chassis, step) || step.spinning {
            air.flip_active = false;
            air.flip_armed = false;
        } else if air.flip_armed && (!armed.modifier || step.modifier) && along(armed.gesture, step) > HELD {
            let t = armed.tuned(p);
            let out0 = t.min_torque * armed.floor;
            // An empty range gives the low end — not nothing, as the preload's does.
            let torque =
                if t.min_time == t.max_time { out0 } else { map_clamp(step.stick_hold.abs(), t.min_time, t.max_time, out0, t.max_torque) };
            // Only the row's own lane is written. Any other lane a row of this set turns about is
            // cleared, so one rotation's leftover torque cannot ride along with the next; the disc's
            // set has no other row, and so clears nothing.
            let mut v = air.flip_torque;
            for r in rows.iter().filter(|r| r.axis != armed.axis) {
                v = with_lane(v, r.axis, F::ZERO);
            }
            air.flip_torque = with_lane(v, armed.axis, if armed.negative { -torque } else { torque });
            air.flip_armed = false;
            air.flip_active = true;
            air.flip_grace = F(armed.grace);
        } else if let Some(k) = rows.iter().position(|r| (!r.modifier || step.modifier) && along(r.gesture, step) < -HELD) {
            air.flip_armed = true;
            *active = k;
        }
    } else if !air.flip_armed {
        // Triggered, and still waiting for the wheels to leave the ground.
        if !air.on_ground {
            air.flip_armed = true;
        } else {
            air.flip_grace = air.flip_grace - step.dt;
            if air.flip_grace <= 0.0 {
                air.flip_active = false;
                air.flip_armed = false;
            }
        }
    } else {
        let row = &rows[*active];
        let t = row.tuned(p);
        if air.on_ground || step.touching || lane(air.flip_torque, row.axis).abs() <= t.min_torque {
            air.flip_active = false;
            air.flip_armed = false;
        } else {
            let mut x = lane(air.flip_torque, row.axis);
            if step.dpad_dir == 0 {
                let input = along(row.gesture, step);
                let rate = if input > 0.0 { t.rate_increase } else { t.rate_decrease };
                // The rate is scaled by the stick first and by the substep second, which is the
                // original's order and so the one that gives the last bit.
                let d = step.dt * (input * rate);
                x = if row.negative { x - d } else { x + d };
            }
            x = if row.negative { x.max(-t.max_torque) } else { x.min(t.max_torque) };
            let v = with_lane(air.flip_torque, row.axis, x);
            air.flip_torque = Vf::new(v.x(), v.y(), v.z(), F::ONE.0);
            chassis.moment = chassis.moment.merge(Dest::XYZ, chassis.moment.add(air.flip_torque));
            air.flip_torque_applied = true;
            air.flipping = true;
        }
    }
    // The rotation is over once a wheel is down, or once the row says the bike is the right way up.
    // Either way the stick gets its authority back over the row's ramp.
    let row = &rows[*active];
    if air.flipping && (air.on_ground || (row.over)(p, air, chassis)) {
        air.flipping = false;
        air.authority = (row.authority)(p);
    }
}

// --- 3b: MX vs. ATV's air moves (MD16) ---------------------------------------------------------------
//
// Nothing here runs on a ride on the original's tricks: the ride only calls [`unleashed_rotate`] and
// [`three_sixty`] when the bike carries a trick pack's numbers, and with it off [`backflip`] alone is
// the substep's rotation, as it always was.

/// **The port's own (MD16): the front flip, in neither game.** The backflip mirrored: under R1 alone,
/// stick back on the lip, then forward, and a positive pitch torque — nose over. The same machinery
/// ([`rotate`]), the same clocks (the backflip's six `.phy` times and torques, its floor, its 0.6 s
/// grace, its ending and its authority ramp) and the same landing test.
///
/// Its torque is the backflip's times [`FRONT_FLIP_TORQUE_SCALE`], 1.0: the backflip's torque turns
/// the bike a full pitch turn within a backflip's air, and the front flip turns about the same axis
/// with the same inertia, so the same numbers give it the same turn (a test proves it fits).
pub const FRONT_FLIP: Rotation = Rotation {
    name: "front flip",
    gesture: Gesture { stick: StickAxis::Pitch, fire_positive: false },
    axis: Axis::Pitch,
    negative: false,
    tuning: |p| {
        let t = (BACKFLIP.tuning)(p);
        let k = F(FRONT_FLIP_TORQUE_SCALE);
        Tuning { min_torque: t.min_torque * k, max_torque: t.max_torque * k, ..t }
    },
    modifier: true,
    ..BACKFLIP
};

/// **The port's own (MD16):** the front flip's torque as a multiple of the backflip's. No game has a
/// number for it; 1.0 because the turn it has to make is the backflip's turn mirrored.
pub const FRONT_FLIP_TORQUE_SCALE: f32 = 1.0;

/// The rotation set a ride on MX vs. ATV's tricks runs: the disc's backflip first, so with R1 up it is
/// exactly [`backflip`] (the front flip only arms or fires under [`Step::modifier`]), then the front flip.
pub const UNLEASHED_ROTATIONS: &[Rotation] = &[BACKFLIP, FRONT_FLIP];

/// One substep of [`UNLEASHED_ROTATIONS`]. `active` is which row the flip state belongs to, kept by
/// the ride between substeps.
pub fn unleashed_rotate(p: &Params, active: &mut usize, air: &mut Air, chassis: &mut Chassis, step: &Step) {
    rotate(UNLEASHED_ROTATIONS, active, p, air, chassis, step);
}

/// MX vs. ATV's `[Three Sixty]` keys, as its `.phy` files name them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinTuning {
    pub min_time: F,
    pub max_time: F,
    pub min_torque_x: F,
    pub max_torque_x: F,
    pub min_torque_y: F,
    pub max_torque_y: F,
    pub rate_increase: F,
    pub rate_decrease: F,
}

impl SpinTuning {
    /// The 360's tuning as [`three_sixty`] reads it: through [`Params::rotation_speed`] when the
    /// rotation-speed modifier is on (P96), the pack's keys as they are when it is off.
    pub fn tuned(&self, p: &Params) -> SpinTuning {
        match p.rotation_speed {
            None => *self,
            Some(s) => s.three_sixty(*self),
        }
    }
}

/// MX vs. ATV's sideways stick past this arms and fires the 360. Its own threshold, not [`HELD`].
const SPIN_HELD: f32 = 0.7;
/// Seconds a fired 360 has to get the wheels off the ground.
const SPIN_GRACE: f32 = 0.9;
/// 35°: past this from the heading it took off on, the 360 is part way round; back inside it, round.
pub const SPIN_CLIP: f32 = 0.6108652;
const PI: f32 = 3.1415927;
const TWO_PI: f32 = 6.2831855;

/// MX vs. ATV's 360 state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Spin {
    /// The torque went into the chassis this substep.
    pub applied: bool,
    /// A 360 turned, and the control-authority ramp starts when the wheels are down.
    pub ramp: bool,
    /// Triggered, and (`armed`) either waiting for the second flick on the lip or turning in the air.
    pub active: bool,
    pub armed: bool,
    /// Counts down from [`SPIN_GRACE`] while a fired 360 is still on the ground.
    pub grace: F,
    /// Which way: 1 for the stick flicked left then right, −1 the other way.
    pub dir: i32,
    /// The flattened heading the bike left the ground on, 0..2π.
    pub start: F,
    /// The torque: `x` pitch, `y` yaw.
    pub torque: Vf,
    /// The clip state: 0 none, 1 started, 2 playing past [`SPIN_CLIP`].
    pub clip: u8,
    /// **The port's own:** latches the rider reads and clears — the bike came back inside
    /// [`SPIN_CLIP`] with the clip playing (scored), and it landed with the clip still playing (a
    /// wreck).
    pub came_round: bool,
    pub wrecked: bool,
}

/// The heading flattened into the horizontal plane, as an angle 0..2π from `+z`.
fn spin_heading(heading: Vf, table: &AcosTable) -> F {
    let h = normalize(Vf::new(heading.x(), 0.0, heading.z(), 1.0));
    let a = table.at(F(h.z()).clamp(-1.0, 1.0));
    if h.x() < 0.0 { (F(PI) - a) + F(PI) } else { a }
}

/// The sign MX vs. ATV reads a stick axis with: −1 below zero, 1 otherwise.
fn sign(x: F) -> i32 {
    if x < F::ZERO { -1 } else { 1 }
}

/// **MX vs. ATV's 360** (SLUS_211.04, the step after its backflip in the same substep), ported as it
/// reads. Under R1 alone on a lip, the stick past 0.7 one way arms it and past 0.7 the other way fires
/// it, with a yaw torque and a pitch torque each mapped from `flick` (the sideways flick speed) through
/// the `[Three Sixty]` times. The bike has 0.9 s to leave the ground. In the air the torque goes in
/// every substep, decaying at `ThreeSixtyTorqueRateDecrease` while no trick button is held and the
/// stick is pushed against the turn; the 360 ends when a wheel touches, the bike hits something or
/// the yaw torque is under its minimum. Landing while the clip is part way round — more than
/// [`SPIN_CLIP`] off the take-off heading — is a wreck.
#[allow(clippy::too_many_arguments)]
pub fn three_sixty(t: &SpinTuning, p: &Params, air: &mut Air, spin: &mut Spin, chassis: &mut Chassis, step: &Step, flick: F, table: &AcosTable) {
    let t = &t.tuned(p);
    spin.applied = false;
    let x = step.stick.0;
    if !spin.active {
        let lip = on_a_lip(air, chassis, step) && !air.flip_torque_applied && !air.flip_active;
        if !lip {
            spin.active = false;
            spin.armed = false;
        } else if spin.armed && x.abs() > SPIN_HELD && sign(x) == spin.dir {
            let map = |lo: F, hi: F| {
                if t.min_time != t.max_time { map_clamp(flick.abs(), t.min_time, t.max_time, lo, hi) } else { lo }
            };
            let d = F(fpu::from_i32(spin.dir));
            let yaw = d * map(t.min_torque_y, t.max_torque_y);
            let pitch = -map(t.min_torque_x, t.max_torque_x);
            spin.torque = Vf::new(pitch.0, yaw.0, spin.torque.z(), spin.torque.w());
            spin.armed = false;
            spin.active = true;
            spin.grace = F(SPIN_GRACE);
        } else if step.modifier {
            if x < -SPIN_HELD {
                spin.armed = true;
                spin.dir = 1;
            } else if x > SPIN_HELD {
                spin.armed = true;
                spin.dir = -1;
            }
        }
    } else if !spin.armed {
        if !air.on_ground {
            spin.armed = true;
            spin.start = spin_heading(step.heading, table);
        } else {
            spin.grace = spin.grace - step.dt;
            if spin.grace <= 0.0 {
                spin.active = false;
                spin.armed = false;
            }
        }
    } else {
        let h = spin_heading(step.heading, table);
        let turned = (spin.start - h).abs().min((spin.start + F(TWO_PI) - h).abs());
        if air.on_ground || step.touching || F(spin.torque.y()).abs() < t.min_torque_y {
            spin.active = false;
            spin.armed = false;
            if spin.clip != 0 {
                spin.clip = 0;
                spin.wrecked = true;
            }
        } else {
            if step.dpad_dir == 0 && !step.modifier && sign(x) != spin.dir {
                let d = F(fpu::from_i32(spin.dir));
                let yaw = F(spin.torque.y()) - step.dt * t.rate_decrease * d * x.abs();
                let pitch = F(spin.torque.x()) - step.dt * x * t.rate_decrease;
                spin.torque = Vf::new(pitch.0, yaw.0, spin.torque.z(), spin.torque.w());
            }
            let m = Vf::new(spin.torque.x(), spin.torque.y(), 0.0, 1.0);
            chassis.moment = chassis.moment.merge(Dest::XYZ, chassis.moment.add(m));
            spin.applied = true;
            spin.ramp = true;
            let past = turned > SPIN_CLIP;
            if spin.clip == 0 {
                if past {
                    spin.clip = 1;
                }
            } else if !past {
                if spin.clip == 2 {
                    spin.came_round = true;
                }
                spin.clip = 0;
            } else if spin.clip == 1 {
                spin.clip = 2;
            }
        }
    }
    if spin.ramp && air.on_ground {
        spin.ramp = false;
        air.authority = p.control_authority_time;
    }
}

// --- 4: what the stick can do in the air ------------------------------------------------------------

/// The airborne control step: the moment the stick commands, what damps it, and the lean force.
///
/// The moment starts as the stick against [`Params::airborne_control_moment`] — pitch from `y`, yaw
/// and roll from `x`, halved and more while a trick is held, and with yaw and roll silenced entirely
/// while a backflip is turning. All of it is scaled by the control-authority ramp, so the substeps
/// after a backflip give the rider progressively more of their input back.
///
/// Then one of two things happens. If the bike is pointing more than [`STEEP`] up or down, or the
/// rider is leaning, or a backflip is turning, or authority is still ramping, the moment is *damped*:
/// the angular velocity times [`Params::airborne_damping`] comes straight off it, and the bike keeps
/// whatever rotation it has. Otherwise the bike *auto-levels*: yaw is pulled towards the crossup angle
/// and roll towards level, each as a gain against the angular velocity, and each axis damped on its
/// own once it is turning faster than [`DEAD`]. That is the single biggest reason the bike feels like
/// it lands on its wheels.
///
/// Last, with no trick held and no backflip torque this substep, the rider's lean pushes the bike
/// sideways: a force along `up x velocity`, normalised, scaled by the lean and
/// [`Params::airborne_control_force`]. That is how a rider crosses up in the air.
pub fn airborne_control(p: &Params, air: &mut Air, chassis: &mut Chassis, step: &Step) {
    let mut ramp = F::ONE;
    if air.authority > 0.0 {
        ramp = if p.control_authority_time == F::ZERO {
            F::ZERO
        } else {
            map_clamp(air.authority, p.control_authority_time, F::ZERO, F::ZERO, F::ONE)
        };
        air.authority = air.authority - step.dt;
    }
    if air.on_ground {
        return;
    }
    let acm = p.airborne_control_moment;
    let mut m = NOTHING;
    // MX vs. ATV's 360 (MD16) silences the stick and damps as a backflip does; false otherwise.
    let flipping = air.flipping || step.spinning;
    let torqued = air.flip_torque_applied || step.spinning;
    if !torqued {
        m = if step.stunt {
            Vf::new(
                (step.stick.1 * (F(0.5) * F(acm.x()))).0,
                (step.stick.0 * (F(0.4) * F(acm.y()))).0,
                (step.stick.0 * (F(0.4) * F(acm.z()))).0,
                m.w(),
            )
        } else {
            Vf::new(
                (F(acm.x()) * step.stick.1).0,
                (F(acm.y()) * step.stick.0).0,
                (F(acm.z()) * step.stick.0).0,
                m.w(),
            )
        };
        if flipping {
            m = Vf::new(m.x(), 0.0, 0.0, m.w());
        }
        m = m.merge(Dest::XYZ, m.mul_q(ramp.0));
    }
    let steep = F(step.heading.y()).abs() >= STEEP_SINE;
    if steep || step.lean.abs() >= DEAD || flipping || air.authority > 0.0 {
        let damped = chassis.ang_vel.merge(Dest::XYZ, chassis.ang_vel.mul_q(p.airborne_damping.0));
        m = m.merge(Dest::XYZ, m.sub(damped));
    } else {
        let y = F(m.y()) + p.unyaw_torque * (p.unyaw_gain * air.crossup - F(chassis.ang_vel.y()));
        let z = F(m.z()) + p.unroll_torque * (p.unroll_gain * chassis.roll - F(chassis.ang_vel.z()));
        m = Vf::new(m.x(), y.0, z.0, m.w());
        if F(chassis.ang_vel.x()).abs() > DEAD {
            m = Vf::new((F(m.x()) - p.airborne_damping * F(chassis.ang_vel.x())).0, m.y(), m.z(), m.w());
        }
        if F(chassis.ang_vel.y()).abs() > DEAD {
            m = Vf::new(m.x(), (F(m.y()) - p.airborne_damping * F(chassis.ang_vel.y())).0, m.z(), m.w());
        }
    }
    // Braking in the air spins the bike up, the way a real one's wheel does.
    if !step.stunt && (step.front_brake != 0.0 || step.rear_brake != 0.0) {
        m = Vf::new((F(m.x()) + p.brake_gyro_torque).0, m.y(), m.z(), m.w());
    }
    chassis.moment = chassis.moment.merge(Dest::XYZ, chassis.moment.add(m));

    if !step.stunt && !torqued && chassis.speed > SLOW {
        let side = normalize(cross(UP, chassis.vel));
        let push = side.mul_q((p.airborne_control_force * step.lean).0);
        chassis.force = chassis.force.merge(Dest::XYZ, chassis.force.add(push));
    }
}

// --- 5: the landing test ----------------------------------------------------------------------------

/// The wreck test: everything that throws the rider off, in the order the original asks it, which is
/// also the order of priority — the first question to answer yes wins and the rest are not asked.
///
/// Fore-and-aft and sideways g loads first, because a crash into something beats anything a landing
/// can do; then the vertical g load against three different limits (one for a no-hander, one for
/// racing, one for freestyle, and freestyle's is the generous one); then the pitch and roll limits,
/// which are a different pair on the ground and in the air and are not asked at all while a backflip
/// is turning. A backflip that is still applying torque when the bike touches something wrecks.
///
/// Then the landing test proper: on the substep the wheels come back down, after a jump long enough to
/// count ([`Params::crossup_wreck_time`]), the rider has to be square with the bike. How square is
/// [`Loads::landing_crossup`], which the rider's pose gives as 0 to 1 with 0.5 straight ahead; outside
/// [`Params::crossup_wreck`] either way it is a wreck, to whichever side they were crossed up.
///
/// Last the two trick endings: a trick held to its end but landed anyway is a joust, a trick that is a
/// no-hander with the bike too far rolled is a no-hander wreck.
///
/// Returns `None` when the rider survives. The caller asks only while the bike is not already wrecked
/// and only while the session has wrecks turned on, which is what the original's two early returns do;
/// the out-of-bounds test the original runs first, and the bike-against-bike loop it runs last, belong
/// to the track and to collision rather than here.
pub fn wreck_test(p: &Params, air: &mut Air, chassis: &Chassis, step: &Step, loads: &Loads) -> Option<Wreck> {
    if loads.fore_aft > p.back_wreck_speed_limit {
        return Some(Wreck::Back);
    }
    if loads.fore_aft < -p.front_wreck_speed_limit {
        return Some(Wreck::Front);
    }
    if loads.side > p.side_wreck_speed_limit {
        return Some(Wreck::Left);
    }
    if loads.side < -p.side_wreck_speed_limit {
        return Some(Wreck::Right);
    }
    let v = loads.vertical;
    if (v > p.no_hander_wreck_speed_limit && loads.no_hander)
        || (v > p.racing_down_wreck_speed_limit && loads.racing)
        || (v > p.freestyle_down_wreck_speed_limit && !loads.racing)
    {
        return Some(Wreck::Flat);
    }
    if air.on_ground && chassis.pitch < -p.ground_pitch_down_wreck_angle {
        return Some(Wreck::Front);
    }
    if air.on_ground && chassis.pitch > p.ground_pitch_up_wreck_angle {
        return Some(Wreck::Tail);
    }
    let rolled = chassis.roll.abs() > p.airborne_roll_wreck_angle;
    if air.airborne && !air.flipping && !step.spinning {
        if chassis.pitch < -p.airborne_pitch_down_wreck_angle || rolled {
            return Some(Wreck::NoseDive);
        }
        if chassis.pitch > p.airborne_pitch_up_wreck_angle || rolled {
            return Some(Wreck::Tail);
        }
    }
    if air.flip_torque_applied && step.touching {
        return Some(Wreck::Tail);
    }
    // The landing.
    if air.was_airborne && !air.airborne && air.hang_time > p.crossup_wreck_time {
        let c = loads.landing_crossup;
        if c > p.crossup_wreck || c < F::ONE - p.crossup_wreck {
            return Some(if c > 0.5 { Wreck::Left } else { Wreck::Right });
        }
    }
    // The trick endings. The first test is the original's, bracketing and all: a trick only lands
    // cleanly if the bike is on the ground or touching something, nothing else has gone wrong with it,
    // and the rider still has their hands on the bars.
    if (!air.on_ground && !step.touching) || !step.stunt || loads.no_hander || loads.trick_wrecked {
        if loads.pose != loads.pose_wanted || loads.pose_blend <= 0.95 {
            if loads.no_hander && F(step.up.y()) < p.no_hander_max_roll_angle {
                return Some(Wreck::NoHander);
            }
            return None;
        }
        // Landed out of a trick onto someone: thrown at the speed it was going, less ten units'
        // worth of its own direction.
        let v = chassis.vel;
        air.wreck_impact = v.merge(Dest::XYZ, v.sub(normalize(v).mul_q(SLOW)));
        return Some(Wreck::Joust);
    }
    Some(Wreck::NoHander)
}

// --- 5b: the hits ------------------------------------------------------------------------------------

/// How far up the bike a body ray has to reach before something it hits throws the rider off: above
/// this, and only the head ray does ([`HEAD`]).
pub const HIT_REACH: F = F(6.0);
/// The same on a 50cc bike, whose two shoulder rays reach above it too.
pub const HIT_REACH_FIFTY: F = F(4.0);

/// The bike's head ray, in the bike's model space: from the hub all eleven of its body rays start at,
/// up through the rider's helmet. The only one of the eleven that ends above [`HIT_REACH`].
///
/// **Measured, not loaded.** The rays come out of the bike's model, which the port does not load; these
/// two points are the ones the running game's collision object held at the reason-6 wreck of
/// recording2 (`research/journal/2026-10-20-s37-s-reason-six`), to the float.
pub const HEAD: (Vf, Vf) = (
    Vf([-1.7404556e-05, 4.063743, -0.09676063, 1.0]),
    Vf([2.9802322e-08, 6.6743364, 0.17120028, 1.0]),
);

/// What a body ray hit, as far as the wreck test asks about it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Other {
    /// Nothing that is an actor: the track, a prop, a wall.
    Scenery,
    /// Another vehicle, by its chassis' height and velocity.
    Rider { height: F, vel: Vf },
    /// An actor with a rigid body and no chassis — a barrel, a cone — by that body's mass and velocity.
    Object { mass: F, vel: Vf },
}

/// One entry of the collision record's contact list, the three things the wreck test reads off it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The entry was written by another record's sweep and mirrored into this one, rather than found
    /// by the bike's own rays. The wreck test skips these.
    pub mirrored: bool,
    /// How high the ray that hit reaches: its end's height in the bike's own model space — the ray's,
    /// not the hit point's, so it says *which* ray hit and not where.
    pub reach: F,
    pub other: Other,
}

/// The wreck test's last clause: the bike's body rays hitting something hard. The first contact the
/// bike's own rays found ([`Contact::mirrored`] clear) with a ray reaching above [`HIT_REACH`] — or
/// [`HIT_REACH_FIFTY`] on a 50cc bike — is the only one looked at, and it decides by what it hit:
///
/// - scenery is a [`Wreck::Scenery`], always;
/// - another rider is a [`Wreck::Joust`] if their chassis is higher than this one's by more than
///   [`Params::joust_wreck_height`] — landed on from above — and their velocity is the throw;
/// - any other actor is a [`Wreck::Joust`] if it is heavier than the bike's chassis, its velocity the
///   throw.
///
/// An actor that does not pass its test still ends the search, so a rider brushed by a lighter object
/// first is not wrecked by scenery further down the list. `touched` is the record's own "anything at
/// all", without which the list is not read. `mass` and `height` are the bike's chassis'.
///
/// The original runs this after the rest of [`wreck_test`] and not instead of it: both can enter a
/// wreck on one substep, and the entry stores its reason unconditionally, so when both answer, this
/// one's is the reason that stays. [`wreck_test`]'s caller composes them that way.
pub fn hit_wreck(p: &Params, air: &mut Air, touched: bool, fifty: bool, mass: F, height: F,
                 contacts: &[Contact]) -> Option<Wreck> {
    if !touched {
        return None;
    }
    let limit = if fifty { HIT_REACH_FIFTY } else { HIT_REACH };
    let c = contacts.iter().find(|c| !c.mirrored && c.reach > limit)?;
    match c.other {
        Other::Scenery => Some(Wreck::Scenery),
        Other::Object { mass: theirs, vel } if mass < theirs => {
            air.wreck_impact = vel;
            Some(Wreck::Joust)
        }
        Other::Rider { height: theirs, vel } if p.joust_wreck_height < theirs - height => {
            air.wreck_impact = vel;
            Some(Wreck::Joust)
        }
        _ => None,
    }
}
