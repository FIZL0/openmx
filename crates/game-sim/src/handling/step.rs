//! **MX vs. ATV's ground-step clauses (MD63)**: the ones SLUS-21104 changed from MX Unleashed's,
//! ported as they read there and proved against its own code (`tests/mxvatv_step.rs`).
//!
//! - The clutch's launch ramp ([`clutch`]): rearming it also clears the multiplier it started at and
//!   the launch's share of the centre-of-gravity travel, and the travel itself is gone — MX vs. ATV
//!   never moves the centre of gravity back on a launch (MX Unleashed's [`crate::r#move::clutch`]
//!   does). The multiplier is still computed and written to the engine as before.
//! - The preload ([`preload`]): a file whose min and max preload times are equal fires with
//!   `−min` (the branch's delay slot subtracts the minimum from the zero it loaded; MX Unleashed's
//!   gives zero), and a cue: the first substep the timer stands above zero the bike posts the
//!   preload's event (category 8, id 14) and holds a latch until the timer is back at or under zero.
//! - The input limiter ([`input_limiter`], MD63a): no squaring of the stick, a gas cut for the first
//!   frames the throttle is back on, and the gas held at 0 on the rider's balance stunt.
//! - The steering command ([`steer_command`], MD63a): `[Joystick]`'s input ramp brings part of the
//!   lock in over a time while the stick is held hard over, when its option is on.
//! - The rider's controllers ([`stand_upright`], [`balance`], [`roll_pitch`], MD63b): MX Unleashed's
//!   arithmetic, with the frames off the body's quaternion and the yaw moments on every bike.
//! - The steering hinge ([`steer_integrate`], MD63c): each sum starts from `0 + x`, so a `−0` rate or
//!   angle comes out `+0`.
//! - The wreck test ([`wreck_test`], [`hit_wreck`], MD63c): reason 15 first, the 360's torque beside the
//!   backflip's, no no-hander term in the trick endings' gate, and the hits' own reach (2.9 / 4.5),
//!   rider-only throws and a scenery hit that needs the g loads past their limits.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::{Air, Chassis, Contact, Loads, Other, Params as AirParams, Step as AirStep, Wreck};
use crate::phy::Joystick;
use crate::math::quat_axes;
use crate::r#move::{Bike, Engine, Frame, Outside, Params, Ride, Step, Tables};
use crate::ride::{Controls, Held, limit, rise, toward};
use crate::trick::Stick;
use ps2_float::{Acc, Dest, F, Vf};

/// `x` from `a..b` onto `out0..out1`, clamped to that range: `(x − a) / (b − a)` on the FPU's
/// accumulator lerp (`adda`, `madd`), then `max` with the low end and `min` with the high end.
fn map_clamp(x: F, a: F, b: F, out0: F, out1: F) -> F {
    let u = (x - a) / (b - a);
    out0.lerp(out1, u).max(out0.min(out1)).min(out0.max(out1))
}

/// One frame of MX vs. ATV's clutch ramp (SLUS-21104's, once a frame in the ground step's head, after
/// the gear choice). Its three cases are MX Unleashed's ([`crate::r#move::clutch`]): clutch in and
/// throttle pinned rearms it, clutch out runs it, anything else leaves the engine's multiplier where
/// it was. What differs is above (the module's doc). An empty range is skipped as `start − 0`, the
/// branch's delay slot, which is the start itself.
pub fn clutch(p: &Params, r: &mut Ride, eng: &mut Engine, s: &Step) {
    if s.clutch == F::ONE && s.gas == F::ONE {
        r.clutch_timer = F::ZERO;
        r.clutch_scale_start = F::ZERO;
        r.clutch_cofg_weight = F::ZERO;
        return;
    }
    if !(s.clutch == F::ZERO && r.clutch_timer <= p.clutch_torque_scale_time) {
        return;
    }
    if r.clutch_timer == F::ZERO {
        let k = p.max_clutch_torque_scale_speed;
        r.clutch_scale_start = if k == F::ZERO {
            p.max_clutch_torque_scale - F::ZERO
        } else {
            map_clamp(s.speed, F::ZERO, k, p.max_clutch_torque_scale, F::ONE)
        };
        // Still made, and read by nothing: the travel it weighed is gone.
        r.clutch_cofg_weight = if k == F::ZERO { F::ONE } else { map_clamp(s.speed, F::ZERO, k, F::ONE, F::ZERO) };
    }
    r.clutch_timer += s.dt;
    let t = p.clutch_torque_scale_time;
    eng.clutch_torque_scale = if t == F::ZERO {
        r.clutch_scale_start - F::ZERO
    } else {
        map_clamp(r.clutch_timer, F::ZERO, t, r.clutch_scale_start, F::ONE)
    };
}

/// The stick past which the preload arms (pulled back) and fires (pushed forward again).
const HELD: f32 = 0.8;

/// One substep of MX vs. ATV's preload (SLUS-21104's, between the roll/pitch control and the
/// backflip). The lip, the arming and the firing are MX Unleashed's ([`crate::air::preload`]) in the
/// original's own comparisons; what differs is the equal-times force and the cue (the module's doc).
/// `cue` is the latch; the answer is whether this substep posted the cue's event.
pub fn preload(p: &AirParams, air: &mut Air, chassis: &mut Chassis, step: &AirStep, cue: &mut bool) -> bool {
    if air.preload_firing {
        air.preload_timer = air.preload_timer - step.dt;
        if F::ZERO < air.preload_timer {
            chassis.force = chassis.force.merge(Dest::XYZ, chassis.force.add(air.preload_force));
        } else {
            air.preload_firing = false;
        }
    } else if !air.on_ground
        || chassis.vel.y() <= 5.0
        || air.contact_vel.z() <= 30.0
        || step.brake != F::ZERO
    {
        air.preload_ready = false;
    } else {
        air.preload_ready = true;
        let y = step.stick.1;
        if !(y <= F(HELD)) {
            air.preload_armed = true;
            air.preload_timer = F::ZERO;
        } else if y < F(-HELD) && air.preload_armed {
            let (from, to) = if step.dpad_y {
                (p.min_dpad_preload_time, p.max_dpad_preload_time)
            } else {
                (p.min_preload_time, p.max_preload_time)
            };
            let force = if from == to { F::ZERO - from } else { map_clamp(step.stick_hold.abs(), from, to, F::ZERO, p.max_preload) };
            air.preload_force = air.preload_force.merge(Dest::Y, ps2_float::Vf::splat(force.0));
            air.preload_timer = p.preload_time;
            air.preload_armed = false;
            air.preload_firing = true;
        } else if air.preload_armed {
            air.preload_timer += step.dt;
        }
    }
    if *cue {
        if air.preload_timer <= 0.0 {
            *cue = false;
        }
        false
    } else if air.preload_timer <= 0.0 {
        false
    } else {
        *cue = true;
        true
    }
}

// --- the input limiter (MD63a) ------------------------------------------------------------------------

/// MX vs. ATV's gas cut after the throttle comes on ([`input_limiter`]): the latch is set while the
/// throttle is off, and once it is back on the gas lever is held at 0 until it is let go or the timer
/// has run past [`GAS_CUT_TIME`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GasCut {
    pub latched: bool,
    pub timer: F,
}

/// How long the gas cut holds. The bike's reset zeroes the word and nothing in SLUS-21104 writes it
/// again, so a cut always lasts two frames: the one the throttle comes on, which counts the timer
/// from 0, and the next, which finds it past 0 and lets go.
pub const GAS_CUT_TIME: F = F::ZERO;

/// What MX vs. ATV's limiter reads beyond MX Unleashed's: the bike is in the air (its gas gate's second
/// way through with a reverse asked for), and how long the gas cut holds ([`GAS_CUT_TIME`] on a ride).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LimiterIn {
    pub airborne: bool,
    pub cut_time: F,
}

/// One frame of MX vs. ATV's input limiter (SLUS-21104's, the ground head's first call), over the
/// port's [`Held`]: what MX Unleashed's rate limit ([`Held::step`]) does, with these differences, in
/// the original's order:
///
/// - the start-gate penalty's cut ([`Controls::gate_cut`]) is the limiter's own first call here;
/// - then the gas cut ([`GasCut`]) runs on the gas lever, so the start-line latch below sees it cut;
/// - the stick is **not squared**: [`Held::stick`] is `stick` as it comes, and the lean walks to it;
/// - the gas's zero gate is the rider's current stunt being the bike's balance stunt (the two words
///   the balance and the stand-upright compare) in place of the no-hander, which the port reads off
///   [`Controls::hands_off`] as before;
/// - a reverse asked for holds the gas at 0 unless the bike is in the air (MX Unleashed's has the same
///   gate; its port leaves it to the pad read's [`Controls::reverse_rule`], which already cut the
///   lever).
///
/// The answer is the controls as the original leaves its lever words: the gas lever cut, the brake
/// lever full under the start-line latch. The flick ([`Held::flick`]) is the port's, as on MX
/// Unleashed's handling.
pub fn input_limiter(held: &mut Held, cut: &mut GasCut, c: &Controls, stick: Stick, j: &Joystick, dt: F, inp: &LimiterIn) -> Controls {
    let mut c = c.gate_cut();
    if held.lock.throttle {
        cut.latched = true;
        cut.timer = F::ZERO;
    } else if cut.latched {
        if c.gas == F::ZERO || inp.cut_time < cut.timer {
            cut.latched = false;
        }
        cut.timer += dt;
        c.gas = F::ZERO;
    }

    held.flick = held.ring.sample(stick.y, dt);
    held.digital = stick.digital;
    held.stick = (stick.x, stick.y);
    let rx = if stick.digital[0] { j.x_dpad_rate_limit } else { j.x_axis_rate_limit } * dt;
    let ry = if stick.digital[1] { j.y_dpad_rate_limit } else { j.y_axis_rate_limit } * dt;
    let lean_x = if held.lock.steer { F::ZERO } else { limit(held.lean.0, stick.x, rx) };
    held.lean = (lean_x, limit(held.lean.1, stick.y, ry));

    let l = &mut held.lock;
    if l.steer && !l.steer_was {
        l.latch = true;
    }
    if l.latch && (!l.steer || (!l.throttle && c.gas > F::ZERO && c.clutch == F::ZERO)) {
        l.latch = false;
    }
    if l.latch {
        c.brake = F::ONE;
        held.reverse = F::ZERO;
    }
    l.steer_was = l.steer;

    held.brake = rise(held.brake, c.brake, j.brake_rate_limit, dt);
    held.front_brake = rise(held.front_brake, c.front_brake.max(c.brake), j.brake_rate_limit, dt);
    held.rear_brake = rise(held.rear_brake, c.rear_brake.max(c.brake), j.brake_rate_limit, dt);
    held.gas = if held.lock.throttle || c.hands_off {
        F::ZERO
    } else if held.reverse == F::ZERO || inp.airborne {
        rise(held.gas, c.gas, j.gas_rate_limit, dt)
    } else {
        F::ZERO
    };
    let clutch_rate = if held.clutch < c.clutch { j.clutch_in_rate_limit } else { j.clutch_out_rate_limit };
    held.clutch = toward(held.clutch, c.clutch, clutch_rate, dt);
    c
}

// --- the steer ramp (MD63a) ---------------------------------------------------------------------------

/// The lean past which the steer ramp's clock runs: the stick held hard over.
const HARD_OVER: f32 = 0.987;

/// MX vs. ATV's steer ramp, `[Joystick]`'s `InputRampAmount` (read in degrees, kept in radians) and
/// `InputRampTime`, and the option that turns it on. The game keeps the option off in every image
/// read (a Free Ride and a quad's spawn) and nothing in SLUS-21104 was found setting it, so a ride
/// starts with it off, which is [`SteerRamp::default`]'s.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SteerRamp {
    pub on: bool,
    pub amount: F,
    pub time: F,
}

/// The degrees `InputRampAmount` is read in, to radians: the reader's own constant.
const DEGREES: f32 = 0.01745329;

impl SteerRamp {
    /// The two keys as the file has them, the option off.
    pub fn of_file(amount_degrees: F, time: F) -> SteerRamp {
        SteerRamp { on: false, amount: amount_degrees * F(DEGREES), time }
    }
}

/// The front wheel's steering command in MX vs. ATV's ground head: with the ramp off, MX Unleashed's
/// ([`crate::r#move::steer_command`]); with it on, `amount` of the lock comes in over `time` while the
/// lean is held at [`HARD_OVER`] or past it (`timer` counts it, and is zeroed under it) and the speed
/// fade scales only the rest of the lock.
pub fn steer_command(ramp: &SteerRamp, lock: F, lean: F, fade: F, timer: &mut F, dt: F) -> F {
    if !ramp.on {
        return lock * (lean * fade);
    }
    *timer = if lean.abs() < F(HARD_OVER) { F::ZERO } else { *timer + dt };
    let t = *timer / ramp.time;
    let t = if F::ONE < t { F::ONE } else { t };
    Acc::mula(lean, ramp.amount * t).madd(lean * fade, lock - ramp.amount)
}

// --- the rider's controllers (MD63b) -----------------------------------------------------------------

/// The frames SLUS-21104's three rider controllers below ask for, answered off the rigid body's own
/// orientation quaternion as its body does: a world vector into the body by the conjugate rotation
/// ([`crate::quad::Body::to_local`], the body's world-moment call and the balance's inline VU0 code
/// alike), and the forward axis out of the quaternion ([`crate::math::quat_axes`]). MX Unleashed asks
/// the chassis' and the node's transforms instead. The noise and the node's axes go to `o`.
struct Oriented<'a> {
    q: Vf,
    o: &'a mut dyn Outside,
}

impl Outside for Oriented<'_> {
    fn unrotate(&mut self, _frame: Frame, v: Vf) -> Vf {
        to_local(self.q, v)
    }
    fn forward(&mut self, _frame: Frame) -> Vf {
        let (f, _) = quat_axes(self.q);
        Vf::new(f[0].0, f[1].0, f[2].0, 0.0)
    }
    fn noise(&mut self, t: F) -> F {
        self.o.noise(t)
    }
    fn set_axes(&mut self, forward: Vf, up: Vf) {
        self.o.set_axes(forward, up);
    }
}

/// `v` out of the world into the frame of the orientation `q` (`q* v q` on VU0, `w` from `v`).
fn to_local(q: Vf, v: Vf) -> Vf {
    crate::quad::Body { orientation: q, ..Default::default() }.to_local(v)
}

/// MX vs. ATV's low-speed upright controller: MX Unleashed's ([`crate::r#move::stand_upright`])
/// instruction for instruction, with the bike's forward axis taken out of the body's quaternion
/// `orientation` and the force handed to the body's add-force (the same `xyz` add). Its "not
/// balancing" is the rider's stunt differing from the balance stunt, which is what calls the balance
/// on this bike: the port's [`Step::balancing`].
pub fn stand_upright(p: &Params, b: &mut Bike<'_>, s: &Step, o: &mut dyn Outside, orientation: Vf) {
    crate::r#move::stand_upright(p, b, s, &mut Oriented { q: orientation, o });
}

/// MX vs. ATV's balance on one wheel: MX Unleashed's ([`crate::r#move::balance`]) clause for clause,
/// with the front wheel's ground normal brought into the bike by the body's quaternion `orientation`
/// (VU0 code inline in the function) where MX Unleashed asks the node's transform.
///
/// The original spells its integrations `0 + x`, then `madd`, sums the flattened normal's squares
/// `y·y` first and weighs the arc cosine's upper entry first; each of those is the same one add of
/// the same rounded product as MX Unleashed's order, so the arithmetic is shared.
pub fn balance(p: &Params, b: &mut Bike<'_>, s: &Step, t: &Tables<'_>, o: &mut dyn Outside, orientation: Vf, dt: F) {
    crate::r#move::balance(p, b, s, t, &mut Oriented { q: orientation, o }, dt);
}

/// MX vs. ATV's wheelie and stoppie: MX Unleashed's ([`crate::r#move::roll_pitch`]) clause for
/// clause, with the same conditions, targets, fade, latch and arithmetic, and one change: the yaw
/// moments (the wheelie's zeroed one and the stoppie's) go through the body's world-moment call,
/// into the body by its quaternion `orientation`, on every bike: there is no "has a transform"
/// test. The noise and the node's axes are not asked for.
pub fn roll_pitch(p: &Params, b: &mut Bike<'_>, s: &Step, orientation: Vf, dt: F) {
    let had = b.chassis.has_transform;
    b.chassis.has_transform = true;
    crate::r#move::roll_pitch(p, b, s, &mut Oriented { q: orientation, o: &mut Unasked }, dt);
    b.chassis.has_transform = had;
}

/// The outside for a controller that asks only for frames: anything else is a bug.
struct Unasked;

impl Outside for Unasked {
    fn unrotate(&mut self, _frame: Frame, _v: Vf) -> Vf {
        unreachable!("Oriented answers the frames")
    }
    fn forward(&mut self, _frame: Frame) -> Vf {
        unreachable!("Oriented answers the frames")
    }
    fn noise(&mut self, _t: F) -> F {
        unreachable!("the wheelie and stoppie make no noise")
    }
    fn set_axes(&mut self, _forward: Vf, _up: Vf) {
        unreachable!("the wheelie and stoppie do not point the node")
    }
}

/// The tail of MX vs. ATV's wheel step (SLUS-21104's `atv_wheel_integrate`): the surface drag and
/// the spin (the two calls, run by the caller before this as in [`crate::wheel::integrate`]), then
/// the steering hinge on the same three cases — a wheel that does not steer is left alone, a steered
/// wheel that is not balanced on has its rate and torque thrown away, and a balanced one turns under
/// its own torque ([`steer_integrate`]).
pub fn wheel_integrate(w: &mut crate::wheel::Wheel, dt: F) -> Vec<crate::wheel::Call> {
    let calls = vec![crate::wheel::Call::SurfaceDrag(dt), crate::wheel::Call::SpinIntegrate(dt)];
    if w.steered {
        if w.balancing {
            steer_integrate(w, dt);
        } else {
            w.steer_rate = F::ZERO;
            w.steer_torque = F::ZERO;
        }
    }
    calls
}

/// MX vs. ATV's steering hinge (`atv_wheel_steer_integrate`): MX Unleashed's, with its
/// fields where the PC's are, the same clamp order (`c.ole` against the top of the lock, so a NaN
/// angle clamps there, then `c.olt` against the bottom) and the same products. What differs is that
/// each of the three sums starts from the accumulator's `0 + x` (`adda.s $f0`): the damped torque,
/// the rate and the angle are `0 + x ± a·b`, so a `−0` torque, rate or angle comes out `+0` where
/// MX Unleashed keeps its sign.
pub fn steer_integrate(w: &mut crate::wheel::Wheel, dt: F) {
    let torque = (F::ZERO + w.steer_torque) - w.steer_damping * w.steer_rate;
    let mut rate = (F::ZERO + w.steer_rate) + dt * (torque * w.steer_inv_inertia);
    let mut angle = (F::ZERO + w.steer_angle) + rate * dt;
    if !(angle <= w.max_steer_angle) {
        angle = w.max_steer_angle;
        rate = F::ZERO;
    } else if angle < w.min_steer_angle {
        angle = w.min_steer_angle;
        rate = F::ZERO;
    }
    w.steer_angle = angle;
    w.steer_rate = rate;
    w.steer_torque = F::ZERO;
}

/// What MX vs. ATV's wreck test reads that MX Unleashed's did not (SLUS-21104's `atv_bike_wreck_test`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WreckIn {
    /// The 360's torque went into the chassis (`+0x11d0`, [`crate::air::Spin::applied`]): it holds the
    /// airborne attitude limits off as the backflip's does, and a touch with it in is a wreck off the
    /// back.
    pub spin_applied: bool,
    /// The collision record is touching and its object (record `+0x20c`) is of the class the game
    /// keeps at `0x00542db0` (`0x20` in the Free Ride image): the reason-15 test's own gate
    /// ([`Wreck::Struck`]).
    pub struck_class: bool,
}

/// The g loads within their limits, as the reason-15 test and the scenery hit ask it: fore and aft
/// against the back and front limits, then sideways both ways (`<=`, so a load on its limit is in).
fn loads_within(p: &AirParams, l: &Loads) -> bool {
    l.fore_aft <= p.back_wreck_speed_limit
        && -p.front_wreck_speed_limit <= l.fore_aft
        && l.side <= p.side_wreck_speed_limit
        && -p.side_wreck_speed_limit <= l.side
}

/// **`atv_bike_flip_wreck_steep`** (S223): reason 15 ([`Wreck::Struck`]) or not, the test the wreck
/// test asks first and the backflip asks of a flip that ends mid-clip (yes `0xf`, no `0xb`). Yes when
/// the collision record is touching an object of the struck class (`struck_class`, see
/// [`WreckIn::struck_class`]; a missing record or object is no) and the bike is airborne (`+0xde0`)
/// or its g loads (`+0x1250` side, `+0x1258` fore and aft) are past the back, front or side limits.
pub fn flip_wreck_steep(p: &AirParams, struck_class: bool, airborne: bool, loads: &Loads) -> bool {
    struck_class && (airborne || !loads_within(p, loads))
}

/// **MX vs. ATV's wreck test** (SLUS-21104's `atv_bike_wreck_test`, the clauses it runs
/// for a driven bike not yet down; the hits after them are [`hit_wreck`]). It is MX Unleashed's
/// state machine and wreck test in one function, and its clauses are [`crate::air::wreck_test_in`]'s
/// in the same order on the same fields and limits, with these differences:
///
/// - First, before any of them, reason 15 ([`Wreck::Struck`]; `atv_bike_flip_wreck_steep`, the test
///   the backflip's mid-clip wreck also asks): [`WreckIn::struck_class`] and the bike airborne or its
///   g loads past the back, front or side limits.
/// - The airborne pitch and roll limits are held off by the backflip's torque being in
///   ([`Air::flip_torque_applied`], `+0x1194`, where MX Unleashed reads `flipping`) or the 360's
///   ([`WreckIn::spin_applied`], `+0x11d0`); and a touch with either in is the wreck off the back.
/// - The trick endings' gate has no "no-hander" term: aloft, or not in a trick (the rider's stunt id
///   over 40), or the trick already a wreck.
///
/// The fields read as the port's: [`AirStep::stunt`] is the rider's stunt id at most 40 (a trick),
/// [`Loads::no_hander`] the id being the bike's balance stunt (`+0x1524 == +0x152c`, also the
/// vertical load's first limit), [`Loads::pose`] and [`Loads::pose_wanted`] the id and the one
/// wanted (`+0x1530`), [`Loads::pose_blend`] its blend (`+0x14e4`), [`Loads::trick_wrecked`]
/// `+0x14cc`, [`Loads::racing`] `+0x146c`, [`AirStep::touching`] the record's `+0x34`.
/// [`AirStep::spinning`] is not read. `in_air` is [`crate::air::wreck_test_in`]'s switch (P138), on
/// the same clauses; true is the original.
pub fn wreck_test(p: &AirParams, air: &mut Air, chassis: &Chassis, step: &AirStep, loads: &Loads, w: &WreckIn,
                  in_air: bool) -> Option<Wreck> {
    if flip_wreck_steep(p, w.struck_class, air.airborne, loads) {
        return Some(Wreck::Struck);
    }
    if p.back_wreck_speed_limit < loads.fore_aft {
        return Some(Wreck::Back);
    }
    if loads.fore_aft < -p.front_wreck_speed_limit {
        return Some(Wreck::Front);
    }
    if p.side_wreck_speed_limit < loads.side {
        return Some(Wreck::Left);
    }
    if loads.side < -p.side_wreck_speed_limit {
        return Some(Wreck::Right);
    }
    let v = loads.vertical;
    if (p.no_hander_wreck_speed_limit < v && loads.no_hander)
        || (p.racing_down_wreck_speed_limit < v && loads.racing)
        || (p.freestyle_down_wreck_speed_limit < v && !loads.racing)
    {
        return Some(Wreck::Flat);
    }
    if air.on_ground && chassis.pitch < -p.ground_pitch_down_wreck_angle {
        return Some(Wreck::Front);
    }
    if air.on_ground && p.ground_pitch_up_wreck_angle < chassis.pitch {
        return Some(Wreck::Tail);
    }
    let torqued = air.flip_torque_applied || w.spin_applied;
    if in_air && air.airborne && !torqued {
        if chassis.pitch < -p.airborne_pitch_down_wreck_angle || p.airborne_roll_wreck_angle < chassis.roll.abs() {
            return Some(Wreck::NoseDive);
        }
        if p.airborne_pitch_up_wreck_angle < chassis.pitch {
            return Some(Wreck::Tail);
        }
    }
    if torqued && step.touching {
        return Some(Wreck::Tail);
    }
    if air.was_airborne && !air.airborne && p.crossup_wreck_time < air.hang_time {
        let c = loads.landing_crossup;
        if p.crossup_wreck < c || c < F::ONE - p.crossup_wreck {
            return Some(if F(0.5) < c { Wreck::Left } else { Wreck::Right });
        }
    }
    let aloft = !air.on_ground && !step.touching;
    if aloft || !step.stunt || loads.trick_wrecked {
        if loads.pose != loads.pose_wanted || loads.pose_blend <= F(0.95) {
            if loads.no_hander && F(step.up.y()) < p.no_hander_max_roll_angle && (in_air || !aloft) {
                return Some(Wreck::NoHander);
            }
            return None;
        }
        let v = chassis.vel;
        air.wreck_impact = v.merge(Dest::XYZ, v.sub(crate::air::normalize(v).mul_q(crate::air::SLOW)));
        return Some(Wreck::Joust);
    }
    Some(Wreck::NoHander)
}

/// How far up the bike a body ray has to reach for what it hits to count, on MX vs. ATV: a literal in
/// the wreck test, by the bike's size word (`+0xbc0`, set once from the spawn record): 2.9 where it is
/// 0, 4.5 otherwise. MX Unleashed's are [`crate::air::HIT_REACH`] (6) and
/// [`crate::air::HIT_REACH_FIFTY`] (4).
pub const HIT_REACH_SMALL: F = F(2.9);
pub const HIT_REACH: F = F(4.5);

/// **MX vs. ATV's hits** (the wreck test's loop over the collision record's contacts, after
/// [`wreck_test`] and asked by the caller only when that let the bike go, as
/// [`crate::air::hit_wreck`]): the first of the bike's own contacts with a ray reaching above
/// [`HIT_REACH`] ([`HIT_REACH_SMALL`] when `small`, the size word 0) decides, and ends the search:
///
/// - another rider is a [`Wreck::Joust`] if their body stands higher than this one's by more than
///   [`AirParams::joust_wreck_height`], their velocity the throw;
/// - scenery is a [`Wreck::Scenery`] only with the g loads past the back, front or side limits — which
///   the clauses before it have already wrecked on, so it answers nothing in the ride;
/// - there is no heavier-object case: an actor is asked by height only, so an
///   [`Other::Object`] (no height here; the port never makes one) answers nothing.
///
/// `touched` is the record's `+0x34`; `height` this bike's.
pub fn hit_wreck(p: &AirParams, air: &mut Air, touched: bool, small: bool, height: F, loads: &Loads,
                 contacts: &[Contact]) -> Option<Wreck> {
    if !touched {
        return None;
    }
    let limit = if small { HIT_REACH_SMALL } else { HIT_REACH };
    let c = contacts.iter().find(|c| !c.mirrored && limit < c.reach)?;
    match c.other {
        Other::Rider { height: theirs, vel } if p.joust_wreck_height < theirs - height => {
            air.wreck_impact = vel;
            Some(Wreck::Joust)
        }
        Other::Scenery if !loads_within(p, loads) => Some(Wreck::Scenery),
        _ => None,
    }
}
