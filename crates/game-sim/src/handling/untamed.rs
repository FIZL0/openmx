//! **MX vs. ATV Untamed's reworked bike functions (MD93)**: the ones SLUS-21701 rewrote from
//! SLUS-21104's (research/untamed-machines.md, "SLUS-21701's bike step against SLUS-21104's"), ported
//! as they read there and proved against an Untamed Free Ride image (`tests/untamed_reworked.rs`).
//! They run under [`super::Handling::Untamed`]; on every other handling nothing here runs.
//!
//! - The preload ([`preload`]): SLUS-21104's ([`super::step::preload`]) with an analogue preload of
//!   its own. With an axis bound to it, a push past a thousandth fires on the lip straight away,
//!   its force the axis mapped onto `0..MaxPreload`. The stick's down-and-up is still there after
//!   it, in the same call (so a stick held up arms again and zeroes the timer). The cue is posted
//!   while the preload fires (SLUS-21104: while its timer stands above zero).
//! - The wreck entry ([`wreck_entry`]): SLUS-21104's, with its three random spins stored together
//!   as one quadword whose `w` is zero.
//! - The put-back ([`put_back`]): the ground snap lifts the bike 0.3 ft along the ground's normal and
//!   keeps that normal. The driver's respawn words go over only when the first two are set. The two
//!   SLUS-21104 restores of `+0x60`/`+0x70` are gone. An attachment, when there is one, is put back
//!   too, and the put-back posts its own event (category 8, id 0x15).
//! - The backflip ([`backflip`], MD93a): SLUS-21104's on Untamed's input. The fourth trick button
//!   arms it whatever else is held, a bound backflip control fires it on the lip at once, the lip reads
//!   a stick word of its own (on the shipped bindings the same axis as the limited stick's, R30), and a
//!   flip's end clears a stunt id over 48 (SLUS-21104: 40).
//! - The 360 ([`spin`], MD93b): SLUS-21104's on the same input. The fourth trick button arms it, a
//!   bound 360 control fires it on the lip at once with torques of its own, the torque decays only with
//!   no trick button held and the stick off centre, and its end clears a stunt id over 48.
//! - The vehicle update ([`vehicle_update`], MD93c): SLUS-21104's, whose calls it reports as
//!   [`UpdateCall`]s, but a remote bike's cleared stick has its angle and length worked out again.
//! - The air state ([`air_state`], MD93d): SLUS-21104's, whose calls it reports as [`AirCall`]s, but a
//!   bike out of the world is unwrecked (no ground snap) where SLUS-21104 put it back with the snap. The
//!   no-hander it calls is SLUS-21104's with registers swapped, so it has no port here.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::{Air, Chassis, Params as AirParams, Step as AirStep};
use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu};

/// How far the stick has to go before it counts as held.
const HELD: f32 = 0.8;
/// How far the analogue preload has to be pushed before it fires.
const AXIS_DEAD: f32 = 0.001;

/// What the preload's event is posted as (category 8, id 14), as on SLUS-21104.
pub const PRELOAD_EVENT: u32 = 14;

/// One substep of Untamed's preload, over MX Unleashed's preload state on [`Air`].
///
/// `axis` is the analogue preload's reading when an action is bound to it, `None` when not (the
/// stick's down-and-up is then all there is). `cue` is the latch the event is posted under. The answer
/// is whether the event was posted this call.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares: a NaN goes the far way
pub fn preload(p: &AirParams, air: &mut Air, chassis: &mut Chassis, step: &AirStep, axis: Option<F>, cue: &mut bool) -> bool {
    let was = air.preload_firing;
    if was {
        air.preload_timer = air.preload_timer - step.dt;
        if !(air.preload_timer <= F::ZERO) {
            chassis.force = chassis.force.merge(Dest::XYZ, chassis.force.add(air.preload_force));
        } else {
            air.preload_firing = false;
        }
    }
    let lip = air.on_ground
        && !(chassis.vel.y() <= 5.0)
        && !(air.contact_vel.z() <= 30.0)
        && step.brake == F::ZERO;
    if let Some(axis) = axis
        && !was
        && lip
        && !(axis <= F(AXIS_DEAD))
    {
        air.preload_firing = true;
        let force = crate::air::map_clamp(axis, F::ZERO, F::ONE, F::ZERO, p.max_preload);
        air.preload_force = air.preload_force.merge(Dest::Y, Vf::splat(force.0));
        air.preload_timer = p.preload_time;
        air.preload_armed = false;
    }
    if !was {
        if !lip {
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
                // As on SLUS-21104: equal times leave the branch's delay slot, `0 − from`.
                let force = if from == to {
                    F::ZERO - from
                } else {
                    crate::air::map_clamp(step.stick_hold.abs(), from, to, F::ZERO, p.max_preload)
                };
                air.preload_force = air.preload_force.merge(Dest::Y, Vf::splat(force.0));
                air.preload_timer = p.preload_time;
                air.preload_armed = false;
                air.preload_firing = true;
            } else if air.preload_armed {
                air.preload_timer = air.preload_timer + step.dt;
            }
        }
    }
    if !*cue && air.preload_firing {
        *cue = true;
        true
    } else {
        if air.preload_timer <= F::ZERO {
            *cue = false;
        }
        false
    }
}

/// What the wreck entry and the put-back call, in the order they call it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// The rider's stunts ended (vt `+0x4a0`).
    EndStunts,
    /// The rider's clip stopped (vt `+0x4a8`, handed −1).
    StopClip,
    /// The bike's physics node switched off (vt `+0x398`, handed 0).
    PhysicsOff,
    /// The collision swapped onto the wreck rays (vt `+0x484`).
    WreckRays,
    /// The vehicle's own wreck entry (the base class's), handed the reason.
    BaseWreck(u32),
    /// The put-back's: the ground asked where the bike stands (see [`put_back`]'s `ground`); the
    /// vehicle settled (its world entry refreshed and its settle word set); the ride rays back, or
    /// the kept ones (vt `+0x39c`, `+0x3a0`); the attachment put back; the event posted (category 8).
    Ground,
    Settle,
    RideRays,
    KeptRays,
    Attachment,
    Post(u32),
}

/// The words the wreck entry writes on the bike beyond [`Air`]'s preload.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WreckWords {
    /// A word the put-back restores from its saved copy, or zeroes (`+0xdd8`; what reads it was not
    /// traced).
    pub hold: u32,
    /// Set on every entry (`+0x1714`).
    pub entered: bool,
    /// Whether the reason was not 0 (`+0x17c4`).
    pub reasoned: bool,
    /// Entries counted (`+0x708`).
    pub count: u32,
}

/// The reason whose entry spins the body at random.
pub const SPIN_REASON: u32 = 13;

/// **Untamed's wreck entry**: end the rider's stunts and clip, and for reason 13 set the body's
/// angular velocity to three draws of `rand` each times 2⁻³¹ (`w` zero). Then switch the physics node
/// off and swap to the wreck rays. Then stop the preload (`firing` and `ready` both cleared), zero
/// [`WreckWords::hold`], mark the entry, and run the vehicle's own entry. Last, note whether there was
/// a reason, and count the entry.
pub fn wreck_entry(
    air: &mut Air,
    w: &mut WreckWords,
    angular_velocity: &mut Vf,
    reason: u32,
    mut rand: impl FnMut() -> i32,
    mut call: impl FnMut(Call),
) {
    call(Call::EndStunts);
    call(Call::StopClip);
    if reason == SPIN_REASON {
        let scale = F::from_bits(0x3000_0000); // 2^-31
        let mut spin = [0.0f32; 4];
        for lane in spin.iter_mut().take(3) {
            *lane = (scale * F(fpu::from_i32(rand()))).0;
        }
        *angular_velocity = Vf(spin);
    }
    call(Call::PhysicsOff);
    call(Call::WreckRays);
    air.preload_firing = false;
    air.preload_ready = false;
    w.hold = 0;
    w.entered = true;
    call(Call::BaseWreck(reason));
    w.reasoned = reason != 0;
    w.count = w.count.wrapping_add(1);
}

/// The event the put-back posts.
pub const PUT_BACK_EVENT: u32 = 0x15;
/// How far the ground snap lifts the bike along the ground's normal.
const SNAP_LIFT: u32 = 0x3e99_999a; // 0.3

/// What the put-back reads and writes on the bike.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PutBack {
    /// The vehicle's position, orientation and up (`+0x80`, `+0x90`, `+0xa0`).
    pub position: Vf,
    pub orientation: Vf,
    pub up: Vf,
    /// The saved place they are put back to (`+0xd30`, `+0xd40`, `+0xd50`).
    pub saved_position: Vf,
    pub saved_orientation: Vf,
    pub saved_up: Vf,
    /// The respawn words handed to the driver (`+0xd60..`): only when the first two are both set.
    pub respawn: [u32; 3],
    /// The driver's copy (its `+0x94..`); `None`, no driver.
    pub driver_respawn: Option<[u32; 3]>,
    /// [`WreckWords::hold`] and its saved copy (`+0xddc`).
    pub hold: u32,
    pub saved_hold: u32,
    /// The bike has a ground to snap to (`+0x3e4`), and an attachment (`+0x10cc`).
    pub has_ground: bool,
    pub has_attachment: bool,
}

/// **Untamed's put-back** (the vehicle's, as the bike's override calls it): the bike back at its saved
/// place. With `snap` and a ground, `ground` is asked where the position stands; it moves the position
/// and answers the normal there. The bike is then lifted 0.3 ft along that normal (`x y z`), and the
/// normal is kept as its up. With `keep` the saved hold word comes back and the kept rays are used,
/// otherwise the hold is zeroed and the ride rays come back.
pub fn put_back(
    pb: &mut PutBack,
    snap: bool,
    keep: bool,
    ground: impl FnOnce(&mut Vf) -> Vf,
    mut call: impl FnMut(Call),
) {
    pb.position = pb.saved_position;
    if let Some(d) = pb.driver_respawn.as_mut()
        && pb.respawn[0] != 0
        && pb.respawn[1] != 0
    {
        *d = pb.respawn;
    }
    if snap && pb.has_ground {
        call(Call::Ground);
        let normal = ground(&mut pb.position);
        let lift = normal.mul(Vf::splat(f32::from_bits(SNAP_LIFT)));
        pb.position = pb.position.merge(Dest::XYZ, pb.position.add(lift));
        pb.orientation = pb.saved_orientation;
        pb.up = normal;
    } else {
        pb.orientation = pb.saved_orientation;
        pb.up = pb.saved_up;
    }
    call(Call::Settle);
    if keep {
        pb.hold = pb.saved_hold;
        call(Call::KeptRays);
    } else {
        pb.hold = 0;
        call(Call::RideRays);
    }
    if pb.has_attachment {
        call(Call::Attachment);
    }
    call(Call::Post(PUT_BACK_EVENT));
}

/// The rider's stunt id at and past which Untamed's stunt is a flip's rather than a trick's: every trick id
/// SLUS-21104 had ([`super::FIRST_FLIP_STUNT`]) moved up by 8, so a trick is an id up to 48.
pub const FIRST_FLIP_STUNT: i32 = super::FIRST_FLIP_STUNT + 8;
/// The idle id in the air, moved up by 8 as the rest ([`super::IDLE_STUNT`]).
pub const IDLE_STUNT: i32 = super::IDLE_STUNT + 8;
/// The stunts the backflip and the 360 start, as the bike keeps them (`+0x13c8`, `+0x13cc`, `+0x13d0`):
/// the stunt table's "backflip", "threesixtyL" and "threesixtyR" (MD93g, off the Free Ride image).
pub const BACKFLIP_STUNT: i32 = 49;
pub const THREESIXTY_L_STUNT: i32 = 52;
pub const THREESIXTY_R_STUNT: i32 = 53;

/// The stick hold a bound backflip button fires with: as if the stick had been flicked at 0.8.
const BUTTON_HOLD: u32 = 0x3f4c_cccd; // 0.8

/// Untamed's backflip: what it reads beyond MX Unleashed's [`AirStep`]. Where SLUS-21104 reads the trick
/// buttons as one nibble ([`super::FlipIn`]), Untamed's input reader keeps the four trick buttons apart,
/// and the stick the lip reads apart from the one the flight's decay reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlipIn {
    /// The fourth trick button is held: what arms the flip on the lip. Unlike SLUS-21104's, the others
    /// may be held with it.
    pub arm_button: bool,
    /// Any of the four trick buttons is held: the flight's torque then does not decay.
    pub trick_held: bool,
    /// The stick's fore-aft as the lip reads it: the reader copies it from a pad word of its own, apart
    /// from [`AirStep::stick`]'s (which the flight's decay reads); the d-pad overrides both alike. The
    /// input object fills both words from the axis bound to each one's action, and the shipped bindings
    /// put both on the same axis (R30).
    pub lip_y: F,
    /// The flick the triggered torque is mapped from (a control of its own, apart from the preload's
    /// [`AirStep::stick_hold`]).
    pub hold: F,
    /// The backflip as a bound control: `None` when the controls have no binding for it on this
    /// device (the stick's down-and-up is the flip), else whether the binding is pressed. Pressed on the
    /// lip, the flip fires at once with the torque of a 0.8 hold; armed or not, it is not disarmed.
    pub button: Option<bool>,
    /// The bike frame's `[0].y` and `[1].y`, as [`super::FlipIn`]'s.
    pub right_y: F,
    pub up_y: F,
    /// The rider's current stunt id: a flip that ends clears one at or past [`FIRST_FLIP_STUNT`] (to 255).
    pub stunt_id: i32,
}

/// The triggered torque's size for a hold, between the floor (1.25 × the least torque) and the most.
fn flip_torque(p: &AirParams, hold: F) -> F {
    let lo = p.min_backflip_torque * F(1.25);
    if p.min_backflip_time == p.max_backflip_time {
        // As SLUS-21104's: the branch's delay slot leaves the map's numerator, not the floor.
        hold - p.min_backflip_time
    } else {
        let u = (hold - p.min_backflip_time) / (p.max_backflip_time - p.min_backflip_time);
        let hi = p.max_backflip_torque;
        let v = F(fpu::madd(fpu::adda(0.0, lo.0), u.0, (hi - lo).0));
        v.max(lo.min(hi)).min(lo.max(hi))
    }
}

/// One substep of Untamed's backflip, over MX Unleashed's backflip state on [`Air`]: SLUS-21104's
/// ([`super::backflip`]) on Untamed's input. On the lip a bound backflip control, when there is one,
/// takes the place of the stick (see [`FlipIn::button`]); otherwise the stick pulled back past 0.8 with
/// the fourth trick button arms it, and pushed past 0.8 triggers it. In the air the torque decays with
/// no trick button held and the stick at or behind centre. A flip that ends clears a stunt id over 48
/// (SLUS-21104: 40).
///
/// `start_stunt` and `steep` are as [`super::backflip`]'s.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares: a NaN goes the far way
pub fn backflip(
    p: &AirParams,
    air: &mut Air,
    chassis: &mut Chassis,
    step: &AirStep,
    inp: &FlipIn,
    clip: &mut super::FlipClip,
    start_stunt: impl FnOnce() -> bool,
    steep: impl FnOnce() -> bool,
) -> super::FlipOut {
    let mut out = super::FlipOut::default();
    air.flip_torque_applied = false;
    let fire = |air: &mut Air, hold: F| {
        let torque = flip_torque(p, hold.abs());
        air.flip_torque = air.flip_torque.merge(Dest::X, Vf::splat((-torque).0));
        air.flip_active = true;
        air.flip_grace = F::from_bits(0x3f19_999a);
    };
    if !air.flip_active {
        let lip = air.on_ground
            && !step.spinning
            && !step.spin_active
            && !(chassis.vel.y() <= 5.0)
            && !(air.contact_vel.z() <= 30.0)
            && step.brake == F::ZERO;
        if !lip {
            air.flip_active = false;
            air.flip_armed = false;
        } else if let Some(pressed) = inp.button {
            if pressed {
                fire(air, F::from_bits(BUTTON_HOLD));
            }
        } else if inp.lip_y < F(-HELD) && inp.arm_button {
            air.flip_armed = true;
        } else if !(inp.lip_y <= F(HELD)) && air.flip_armed {
            fire(air, inp.hold);
            air.flip_armed = false;
        }
    } else if !air.flip_armed {
        if !air.on_ground {
            air.flip_armed = true;
        } else {
            air.flip_grace = air.flip_grace - step.dt;
            if air.flip_grace <= F::ZERO {
                air.flip_active = false;
                air.flip_armed = false;
            }
        }
    } else if air.on_ground || step.touching || F(air.flip_torque.x()).abs() < p.min_backflip_torque {
        air.flip_active = false;
        air.flip_armed = false;
        if inp.stunt_id >= FIRST_FLIP_STUNT {
            out.stunt_cleared = true;
        }
        if clip.0 != 0 {
            clip.0 = 0;
            out.wreck = Some(if steep() { 0xf } else { 0xb });
        }
    } else {
        let y = step.stick.1;
        if !inp.trick_held && y <= F::ZERO {
            let t = F(fpu::msub(fpu::adda(0.0, air.flip_torque.x()), step.dt.0, (y * p.backflip_torque_rate_decrease).0));
            air.flip_torque = air.flip_torque.merge(Dest::X, Vf::splat(t.0));
        }
        chassis.moment = chassis.moment.merge(Dest::XYZ, chassis.moment.add(air.flip_torque));
        air.flip_torque_applied = true;
        air.flipping = true;
        if inp.right_y <= F::ZERO {
            if !(inp.up_y <= F::ZERO) && clip.0 == 0 {
                out.stunt_started = start_stunt();
                if out.stunt_started {
                    clip.0 = 1;
                }
            } else if inp.up_y < F::ZERO && clip.0 == 1 {
                clip.0 = 2;
            }
        } else if !(inp.up_y <= F(-0.79)) && clip.0 == 2 {
            out.stunt_ended = true;
            clip.0 = 0;
        }
    }
    if air.flipping {
        let back = air.on_ground
            || (!air.flip_torque_applied
                && chassis.roll.abs() < F::from_bits(0x3fc9_0fdb)
                && !(chassis.pitch <= -p.airborne_pitch_down_wreck_angle)
                && chassis.pitch < p.airborne_pitch_up_wreck_angle);
        if back {
            air.flipping = false;
            air.authority = p.control_authority_time;
        }
    }
    out
}

/// **The port's own:** the rider's stunt id by its range, as [`super::stunt_stand_in`] gives it, on
/// Untamed's ids: the flip's and the idle one in the air moved up by 8, a trick's (0) and the ground's
/// (`0xff`) as they were.
pub fn stunt_stand_in(trick_held: bool, backflip: bool, spin_clip: bool, on_ground: bool) -> i32 {
    match super::stunt_stand_in(trick_held, backflip, spin_clip, on_ground) {
        id @ (0 | 0xff) => id,
        id => id + 8,
    }
}

/// [`backflip`] on a ride, as [`super::ride_backflip`] runs SLUS-21104's: the same rider's side, on
/// Untamed's input as its shipped controls give it (MD93f):
///
/// - Untamed's four trick actions are bound by default to Circle, Triangle, Select and R1, in that
///   order. The first, second and fourth are the port's `trick` (TRICK_1..3 on MX Unleashed's first
///   preset: Circle, Triangle, R1), so the arming button is `trick[2]`. Select is the port's camera
///   button, with no trick command, so the third reads unpressed. A trick is held when any of the
///   three is.
/// - The flick is the step's ([`AirStep::stick_hold`]): Untamed binds the hold to the same analogue
///   control as SLUS-21104's hold, which is what the port's flick reads.
/// - There is no bound control ([`FlipIn::button`] `None`). The binding asked for is the classic
///   style's, the only one the game picks, and none of the shipped stunts map one to the backflip. So
///   the stick's down-and-up is always the flip.
/// - The lip reads the step's stick. In the reader the lip's copy differs from the limited one only by
///   the pad word it is copied from (the d-pad overrides both alike). The input object fills those
///   words from the device axis bound to each word's action, and the shipped bindings put the lip's
///   and the limited stick's actions on the same axis. Neither word is limited, so the step's
///   unlimited stick with the d-pad applied is right (R30). They differ only where a player's
///   second device pushes further than the pad: then it replaces the limited words and not the
///   lip's. No shipped setup has a second device.
pub fn ride_backflip(p: &AirParams, air: &mut Air, chassis: &mut Chassis, step: &AirStep, trick: [bool; 3],
                     frame: &crate::chassis::Mat3, flip: &mut super::Flip, steep: impl FnOnce() -> bool) {
    if step.stunt {
        flip.stunt = false;
    }
    let stunt_id = stunt_stand_in(step.stunt, flip.stunt, false, air.on_ground);
    let inp = FlipIn {
        arm_button: trick[2],
        trick_held: trick.iter().any(|&b| b),
        lip_y: step.stick.1,
        hold: step.stick_hold,
        button: None,
        right_y: F(frame.rows[0].y()),
        up_y: F(frame.rows[1].y()),
        stunt_id,
    };
    let mut clip = super::FlipClip(flip.clip);
    let aloft = !air.on_ground && !step.touching;
    let mut stunt = flip.stunt;
    let out = backflip(p, air, chassis, step, &inp, &mut clip, || {
        if stunt_id >= FIRST_FLIP_STUNT {
            stunt = true;
        }
        true
    }, steep);
    flip.stunt = stunt && !out.stunt_cleared;
    flip.clip = clip.0;
    if let Some(reason) = out.wreck {
        flip.wrecked = true;
        flip.struck = reason == 0xf;
        flip.cut = aloft;
    }
    if out.stunt_ended {
        flip.came_round = true;
    }
}

// --- the 360 (MD93b) ----------------------------------------------------------------------------------

/// How far over the stick has to go to arm and fire the 360, as on SLUS-21104.
const SPIN_HELD: f32 = 0.7;
/// The seconds a fired 360 has to get the wheels off the ground.
const SPIN_GRACE: u32 = 0x3f66_6666; // 0.9
/// 35°: the turn from the take-off heading past which the clip starts, and back under which it stops.
const SPIN_CLIP: u32 = 0x3f1c_61a9;
const PI: u32 = 0x4049_0fdb;
const TWO_PI: u32 = 0x40c9_0fdb;
/// The arc cosine table's lookup scale (the ELF's `0x003ff550`).
const ACOS_SCALE: f32 = 14999.5;
/// What a bound 360 control fires with: constants in the ELF (`0x0053d3e8`, `0x0053d3f0`), not the
/// bike's `[Three Sixty]` keys. The yaw is times the direction (see [`spin`]); the pitch is +200 where
/// the stick's is the negated map.
const BUTTON_YAW: u32 = 0x4489_8000; // 1100
const BUTTON_PITCH: u32 = 0x4348_0000; // 200

/// Untamed's 360: what it reads beyond MX Unleashed's [`AirStep`]. The buttons and the bound control are
/// read off the input reader's pad record as [`FlipIn`]'s are.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpinIn {
    /// The fourth trick button is held: what arms the 360 on the lip with the stick over (the others
    /// may be held with it; SLUS-21104 wants R1 alone).
    pub arm_button: bool,
    /// Any of the four trick buttons is held: the torque in the air then does not decay.
    pub trick_held: bool,
    /// The stick's side-to-side as the lip reads it (`+0x12bc`, the reader's copy of a pad word of its
    /// own, apart from [`SpinIn::x`]'s; the d-pad overrides both alike). On the shipped bindings both
    /// words are the same axis (R30).
    pub lip_x: F,
    /// The limited stick's side-to-side (`+0x1298`), which the decay in the air reads.
    pub x: F,
    /// The sideways flick the fired torque is mapped from (`+0x12b8`).
    pub hold: F,
    /// The 360 as a bound control: `None` when this device has no binding for it (the stick is the
    /// 360), else which way the binding is pressed: −1 left (asked first), 1 right, 0 neither.
    pub button: Option<i32>,
    /// The bike frame's forward axis (its node's third row), which the heading is read off.
    pub forward: Vf,
    /// The rider's current stunt id: a 360 that ends clears one at or past [`FIRST_FLIP_STUNT`] (to 255).
    pub stunt_id: i32,
}

/// What one call of [`spin`] asked of the rest of the bike, as [`super::FlipOut`] for the backflip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SpinOut {
    /// The 360's stunt was asked to start (the clip went to 1 only if it did).
    pub stunt_started: bool,
    /// The 360's stunt was asked to end (the clip came round: 2 to 0).
    pub stunt_ended: bool,
    /// The rider's stunt id was cleared (at or past [`FIRST_FLIP_STUNT`], set to 255) as the 360 ended.
    pub stunt_cleared: bool,
    /// The 360 ended with its clip running: the wreck, `0xb`, or `0xf` when `steep` answered yes.
    pub wreck: Option<u8>,
}

/// `[-1, 1]` as two compares and two moves: anything unordered comes back as `1.0`.
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

/// The heading of the forward axis flattened (`y` 0, normalised through `vrsqrt`), off the arc cosine
/// table: `acos(z)` (clamped twice; `mula(frac, hi)`, `madd(1 - frac, lo)`), and `(π − a) + π` for an
/// `x` below 0.
fn spin_heading(forward: Vf, table: &crate::air::AcosTable) -> F {
    let v = crate::air::normalize(forward.merge(Dest::Y, Vf::ZERO));
    let z = clamp_unit(clamp_unit(F(v.z())));
    let t = table.entries();
    let s = (F::ONE + z) * F(ACOS_SCALE);
    let i = s.to_i32();
    let frac = s - F(fpu::from_i32(i));
    let a = F(fpu::madd(fpu::mula(frac.0, t[i as usize + 1]), (F::ONE - frac).0, t[i as usize]));
    if F(v.x()) < F::ZERO { (F::from_bits(PI) - a) + F::from_bits(PI) } else { a }
}

/// The fired 360's torque off the flick: the low end when the two times are equal, else the flick mapped
/// from the times onto the two torques and clamped between them.
fn spin_torque(t: &crate::air::SpinTuning, hold: F, lo: F, hi: F) -> F {
    if t.min_time == t.max_time { lo } else { crate::air::map_clamp(hold.abs(), t.min_time, t.max_time, lo, hi) }
}

/// One substep of Untamed's 360 (SLUS-21701's spin in the air, called after its backflip), over
/// MX vs. ATV's 360 state ([`crate::air::Spin`]; only its original's words: the port's latches are left
/// alone). It is SLUS-21104's on Untamed's input:
///
/// - On a lip (on the ground, no backflip torque in or triggered, rising faster than 5 and faster than
///   30 forward, the brake off) a bound 360 control, when this device has one, takes the stick's place.
///   Pressed either way it fires at once, a yaw of 1100 times the direction the 360 had *before* (the
///   word is read before the press's direction is stored) and a pitch of +200, with the grace 0.9.
/// - Otherwise an armed 360 whose lip stick is over past 0.7 its own way fires with the torques mapped
///   off the flick, and the fourth trick button (others may be held) with the lip stick past 0.7 arms it,
///   to the left (direction 1) for a stick left. Off a lip it is disarmed.
/// - In the air it is SLUS-21104's, but the torque decays only with none of the four trick buttons held
///   and the limited stick off centre (SLUS-21104: at centre too, which reads as pushed right) and
///   against the turn. A 360 that ends clears a stunt id over 48 (SLUS-21104: 40).
///
/// `start_stunt` is handed the direction (the stunt it starts is the bike's left or right 360 by it)
/// and answers whether the stunt started; `steep` is the mid-clip wreck's reason test. Both are called
/// only where the original calls them.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares: a NaN goes the far way
#[allow(clippy::too_many_arguments)]
pub fn spin(
    t: &crate::air::SpinTuning,
    p: &AirParams,
    air: &mut Air,
    s: &mut crate::air::Spin,
    chassis: &mut Chassis,
    step: &AirStep,
    inp: &SpinIn,
    table: &crate::air::AcosTable,
    start_stunt: impl FnOnce(i32) -> bool,
    steep: impl FnOnce() -> bool,
) -> SpinOut {
    let mut out = SpinOut::default();
    let sign = |x: F| if x < F::ZERO { -1 } else { 1 };
    s.applied = false;
    if !s.active {
        let lip = air.on_ground
            && !air.flip_torque_applied
            && !air.flip_active
            && !(chassis.vel.y() <= 5.0)
            && !(air.contact_vel.z() <= 30.0)
            && step.brake == F::ZERO;
        if !lip {
            s.active = false;
            s.armed = false;
        } else if let Some(way) = inp.button {
            if way != 0 {
                let yaw = F(fpu::from_i32(s.dir)) * F::from_bits(BUTTON_YAW);
                s.torque = s.torque.merge(Dest::Y, Vf::splat(yaw.0)).merge(Dest::X, Vf::splat(f32::from_bits(BUTTON_PITCH)));
                s.grace = F::from_bits(SPIN_GRACE);
                s.active = true;
                s.armed = false;
                s.dir = way;
            }
        } else if s.armed && !(inp.lip_x.abs() <= F(SPIN_HELD)) && sign(inp.lip_x) == s.dir {
            let yaw = F(fpu::from_i32(s.dir)) * spin_torque(t, inp.hold, t.min_torque_y, t.max_torque_y);
            let pitch = -spin_torque(t, inp.hold, t.min_torque_x, t.max_torque_x);
            s.torque = s.torque.merge(Dest::Y, Vf::splat(yaw.0)).merge(Dest::X, Vf::splat(pitch.0));
            s.armed = false;
            s.active = true;
            s.grace = F::from_bits(SPIN_GRACE);
        } else if inp.arm_button {
            if inp.lip_x < F(-SPIN_HELD) {
                (s.armed, s.dir) = (true, 1);
            } else if !(inp.lip_x <= F(SPIN_HELD)) {
                (s.armed, s.dir) = (true, -1);
            }
        }
    } else if !s.armed {
        if air.on_ground {
            s.grace = s.grace - step.dt;
            if s.grace <= F::ZERO {
                s.active = false;
                s.armed = false;
            }
        } else {
            s.armed = true;
            s.start = spin_heading(inp.forward, table);
        }
    } else {
        let now = spin_heading(inp.forward, table);
        let off = (s.start - now).abs().min(((F::from_bits(TWO_PI) + s.start) - now).abs());
        if air.on_ground || step.touching || F(s.torque.y()).abs() < t.min_torque_y {
            s.active = false;
            s.armed = false;
            if inp.stunt_id >= FIRST_FLIP_STUNT {
                out.stunt_cleared = true;
            }
            if s.clip != 0 {
                s.clip = 0;
                out.wreck = Some(if steep() { 0xf } else { 0xb });
            }
        } else {
            let x = inp.x;
            if !inp.trick_held && x != F::ZERO && s.dir != sign(x) {
                let d = F(fpu::from_i32(s.dir));
                let by = t.rate_decrease * (d * x.abs());
                let yaw = F(fpu::msub(fpu::adda(0.0, s.torque.y()), step.dt.0, by.0));
                let by = x * t.rate_decrease;
                let pitch = F(fpu::msub(fpu::adda(0.0, s.torque.x()), step.dt.0, by.0));
                s.torque = s.torque.merge(Dest::Y, Vf::splat(yaw.0)).merge(Dest::X, Vf::splat(pitch.0));
            }
            chassis.moment = chassis.moment.merge(Dest::XYZ, chassis.moment.add(s.torque));
            s.applied = true;
            s.ramp = true;
            let past = F::from_bits(SPIN_CLIP);
            if s.clip == 0 {
                if !(off <= past) {
                    out.stunt_started = start_stunt(s.dir);
                    if out.stunt_started {
                        s.clip = 1;
                    }
                }
            } else if off < past {
                if s.clip == 2 {
                    out.stunt_ended = true;
                    s.clip = 0;
                }
            } else if s.clip == 1 {
                s.clip = 2;
            }
        }
    }
    if s.ramp && air.on_ground {
        s.ramp = false;
        air.authority = p.control_authority_time;
    }
    out
}

/// The stunt a 360 turning `dir` starts: the bike's `+0x13cc` ([`THREESIXTY_L_STUNT`]) for a direction
/// under 1, else `+0x13d0` ([`THREESIXTY_R_STUNT`]). A stick left arms direction 1, so it is "R"; a bound
/// control's left (direction −1) is "L".
pub fn spin_stunt(dir: i32) -> i32 {
    if dir < 1 { THREESIXTY_L_STUNT } else { THREESIXTY_R_STUNT }
}

/// [`spin`] on a ride on Untamed's handling, in the place [`crate::air::three_sixty`] runs on a ride on
/// a trick pack, over the same state and with the pack's `[Three Sixty]` keys (through the rotation-speed
/// modifier, as there). The rider's side is as there: the stunt start answers yes (MD59, the clip state
/// runs whether or not the rider has the clip), the clip coming round latches [`crate::air::Spin::came_round`],
/// and a mid-clip end latches [`crate::air::Spin::wrecked`] (and [`crate::air::Spin::cut`] off the ground),
/// which the rider takes as the 360's wreck whatever `steep` would say. None of the port's own additions
/// to the 360 (MD20's both at once and the pitch that follows a front flip) run here.
///
/// The input is Untamed's shipped controls, as [`ride_backflip`]'s (MD93f, MD93g):
///
/// - The arming button (pad record `+24`) is `trick[2]`, R1, the fourth trick action's default. A trick
///   is held when any of the three is.
/// - Both sticks are the step's: the lip's x (`+0x12bc`) and the decay's (`+0x1298`) are plain copies
///   of two pad words. The d-pad overrides both alike. The words are the axes bound to two actions,
///   which the shipped bindings put on the same axis (R30, as in [`ride_backflip`]).
/// - The flick is `flick`, the side flick. The spin hold (`+0x12b8`) is bound by default to the
///   analogue control SLUS-21104's spin hold reads.
/// - There is no bound control ([`SpinIn::button`] `None`). The 360's "bound" (`0x00252630`) is the
///   backflip's byte: the stunts loader sets it only from the backflip's own key mapping, and that mapping
///   is empty in every style. So "bound left/right" (`0x00252690`/`0x002526e0`, matching the pad record
///   against threesixtyL's/R's classic combo) is never asked. Those two combos are empty too.
///
/// The rider's stunt id is the real one while the 360's clip runs: the stunt start sets it to
/// [`spin_stunt`] unless a trick holds it. Otherwise it is [`stunt_stand_in`]'s. Its clearing is not
/// kept (the port's rider has no id).
#[allow(clippy::too_many_arguments)]
pub fn ride_spin(t: &crate::air::SpinTuning, p: &AirParams, air: &mut Air, s: &mut crate::air::Spin, chassis: &mut Chassis,
                 step: &AirStep, trick: [bool; 3], flick: F, table: &crate::air::AcosTable) {
    let t = t.tuned(p);
    let stunt_id = match stunt_stand_in(step.stunt, false, s.clip != 0, air.on_ground) {
        FIRST_FLIP_STUNT => spin_stunt(s.dir),
        id => id,
    };
    let inp = SpinIn {
        arm_button: trick[2],
        trick_held: trick.iter().any(|&b| b),
        lip_x: step.stick.0,
        x: step.stick.0,
        hold: flick,
        button: None,
        forward: step.heading,
        stunt_id,
    };
    let aloft = !air.on_ground && !step.touching;
    let out = spin(&t, p, air, s, chassis, step, &inp, table, |_| true, || false);
    if out.wreck.is_some() {
        s.wrecked = true;
        s.cut = aloft;
    }
    if out.stunt_ended {
        s.came_round = true;
    }
}

/// What Untamed's vehicle update ([`vehicle_update`], MD93c) calls, in the order it calls them. Each is
/// handed the bike, and those with a `dt` the frame's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpdateCall {
    /// The ghost's step (vt `+0x3a4`), first, whenever the game-wide skip bit is clear.
    Ghost(F),
    /// A local bike's: the wreck test (vt `+0x494`, handed 0) and the stunt idle (vt `+0x4e4`).
    WreckTest(F),
    StuntIdle(F),
    /// A remote bike's (vehicle states 3 and 4), with its inputs cleared: the physics listeners stepped
    /// (the ones at the bike's `+0x30`), the node put where the body is (vt `+0x4ec`, no `dt`), the stunt
    /// clocks (vt `+0x4e0`), and the wreck reason handed to the remote's setter (vt `+0x498`).
    Listeners(F),
    NodeFromBody,
    StuntClocks(F),
    RemoteWreck { dt: F, reason: u32 },
    /// The timer's object told the timer ran out (its vt `+0x90`; only when there is one).
    TimerOut,
    /// The vehicle's own update (the base class's), last; with no vehicle state it is all that runs.
    VehicleUpdate(F),
}

/// What Untamed's vehicle update reads and writes on the bike.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Update {
    /// The vehicle state's kind (its `+0x28`; the state is the bike's `+0xbb4`). `None`: no state.
    pub vehicle_state: Option<u32>,
    /// A game-wide bit that skips the ghost, the wreck test and the rest of the rider's half (bit 3 of a
    /// global flag byte; what sets it was not traced).
    pub skip: bool,
    /// The limited stick (`+0x1298`, `+0x129c`), its angle and length (`+0x12a8`, `+0x12ac`), the brake
    /// (`+0x340`) and one more input word (`+0x12d0`): a remote bike's are cleared.
    pub stick: (F, F),
    pub stick_angle: F,
    pub stick_length: F,
    pub brake: F,
    pub input: u32,
    /// The wreck reason (`+0xc70`), handed to [`UpdateCall::RemoteWreck`].
    pub wreck_reason: u32,
    /// Frames until the one-frame request (`+0x34c`) is raised (`+0x17b0`).
    pub request_countdown: i32,
    pub request: u32,
    /// A timer (`+0x17bc`) and whether its object is there (the `+0x30` of what `+0x17b8` points at).
    pub timer: F,
    pub timed: bool,
    /// A word the update zeroes last (`+0x1690`).
    pub cleared: u32,
}

/// The vehicle state kinds a remote bike rides on.
const REMOTE: [u32; 2] = [3, 4];

/// **Untamed's vehicle update** (SLUS-21701's bike override of the vehicle's), once a frame. It is
/// SLUS-21104's ([`crate::quad::Call::BikeVehicleUpdate`]), on Untamed's fields and slots, with one
/// addition: a remote bike's stick angle and length are worked out again after its stick is cleared.
///
/// With no vehicle state only the vehicle's own update runs. Else, unless the game-wide skip bit is set:
/// the ghost steps; then, with the vehicle state (read again: the ghost may have changed it) of kind 3
/// or 4, the stick, the brake and the input word are cleared, the stick's angle (`acos` of `x` over the
/// length off the arc cosine table, negated for a `y` below 0) and length are set (for the cleared stick,
/// the table's middle and 0), and the listeners, node, stunt clocks and remote wreck run; any other
/// bike runs the wreck test and the stunt idle. Then, skipped or not: the request countdown runs, and
/// raises the request on reaching 0; a running timer counts down, and on running out tells its object
/// and is zeroed; the vehicle's own update runs; and [`Update::cleared`] is zeroed.
///
/// `call` is handed each call with the state, which it may change: the update reads it again after.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn vehicle_update(u: &mut Update, dt: F, table: &crate::air::AcosTable, mut call: impl FnMut(UpdateCall, &mut Update)) {
    if u.vehicle_state.is_none() {
        call(UpdateCall::VehicleUpdate(dt), u);
        return;
    }
    if !u.skip {
        call(UpdateCall::Ghost(dt), u);
        if u.vehicle_state.is_some_and(|k| REMOTE.contains(&k)) {
            u.stick = (F::ZERO, F::ZERO);
            let (x, y) = u.stick;
            let len2 = ps2_float::Acc::mula(x, x).madd(y, y);
            let c = clamp_unit(clamp_unit(x * F::ONE.rsqrt(len2)));
            let t = table.entries();
            let s = (F::ONE + c) * F(ACOS_SCALE);
            let i = s.to_i32();
            let frac = s - F(fpu::from_i32(i));
            let a = F(fpu::madd(fpu::mula(frac.0, t[i as usize + 1]), (F::ONE - frac).0, t[i as usize]));
            u.stick_angle = if y < F::ZERO { -a } else { a };
            u.stick_length = len2.sqrt();
            u.brake = F::ZERO;
            u.input = 0;
            call(UpdateCall::Listeners(dt), u);
            call(UpdateCall::NodeFromBody, u);
            call(UpdateCall::StuntClocks(dt), u);
            let reason = u.wreck_reason;
            call(UpdateCall::RemoteWreck { dt, reason }, u);
        } else {
            call(UpdateCall::WreckTest(dt), u);
            call(UpdateCall::StuntIdle(dt), u);
        }
    }
    if u.request_countdown > 0 {
        u.request_countdown -= 1;
        if u.request_countdown == 0 {
            u.request = 1;
        }
    }
    if !(u.timer <= F::ZERO) {
        u.timer = u.timer - dt;
        if u.timer <= F::ZERO {
            if u.timed {
                call(UpdateCall::TimerOut, u);
            }
            u.timer = F::ZERO;
        }
    }
    call(UpdateCall::VehicleUpdate(dt), u);
    u.cleared = 0;
}

/// What Untamed's air state ([`air_state`], MD93d) calls, in the order it calls them. Each is handed
/// the bike, and those with a `dt` the frame's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AirCall {
    /// Not balancing (no wreck reason and a stunt id not the balance's): the balance's four words
    /// cleared, then the no-hander. Both are SLUS-21104's code on moved fields.
    BalanceCleared,
    NoHander(F),
    /// The vehicle's in-world test (its vt `+0x120`), on the bike's origin: the body's position less its
    /// centre of gravity turned into the world. The answer is the callback's.
    InWorld(Vf),
    /// Out of the world: the event the clock posts (category 8, id 13) under the bike's event tag, then
    /// the bike's unwreck (vt `+0x36c`, handed 1). SLUS-21104 here clears three words and calls the
    /// put-back (vt `+0x3a8`, handed 1 and 1) instead.
    Post { tag: u32 },
    Unwreck,
    /// The vehicle's own post step (the base class's PostProcess).
    VehiclePost(F),
    /// The scene node's position asked for ([`AirState::node_pos`]); it is kept in a second copy after.
    NodePosition,
}

/// What the out-of-world event is posted as (category 8, id 13), as on SLUS-21104.
pub const OUT_OF_WORLD_EVENT: u32 = 13;

/// What Untamed's air state reads and writes on the bike and its body.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirState {
    /// The vehicle state's kind (its `+0x28`; the state is the bike's `+0xbb4`). `None`: no state.
    pub vehicle_state: Option<u32>,
    /// The wreck reason (`+0xc70`), the rider's stunt id and the balance's (`+0x1544`, `+0x154c`).
    pub wreck_reason: u32,
    pub stunt_id: u32,
    pub balance_id: u32,
    /// The rigid body (the bike's `+0xe8`): its velocity, orientation, angular velocity and position,
    /// and its centre of gravity in its own frame (body `+0x1a0`).
    pub body: crate::quad::Body,
    pub cofg: Vf,
    /// The two wheels' (`+0x17f0`, `+0x17f4`) touching words (their `+0xdc`) and contact velocities
    /// (their `+0x190`).
    pub wheel_touching: [u32; 2],
    pub wheel_contact_vel: [Vf; 2],
    /// The body's speed (`+0x35c`), velocity in its own frame (`+0x380`), forward and up axes (`+0xbd0`,
    /// `+0xbe0`; `w` untouched), euler angles (`+0x390`) and angular velocity in the world (`+0x370`).
    pub speed: F,
    pub local_vel: Vf,
    pub heading: Vf,
    pub up: Vf,
    pub euler: Vf,
    pub world_ang_vel: Vf,
    /// Either wheel down (`+0x354`), both down (`+0x1260`), airborne this frame and last (`+0xdd0`,
    /// `+0xdd4`), the hang time (`+0x1180`) and the contact velocity (`+0xc20`).
    pub on_ground: u32,
    pub both_down: u32,
    pub airborne: u32,
    pub was_airborne: u32,
    pub hang_time: F,
    pub contact_vel: Vf,
    /// The backflip's flipping word (`+0x1188`) and the 360's ramp (`+0x11c4`): either set, no crossup.
    pub flipping: u32,
    pub spin_ramp: u32,
    /// The control authority after a flip (`+0x1178`), its time (`+0x117c`), and the crossup (`+0x360`).
    pub authority: F,
    pub authority_time: F,
    pub crossup: F,
    /// The collision record's (`+0xf8`) tipped word (its `+0x24`).
    pub tipped: u32,
    /// The bike's event tag (`+0xd10`).
    pub event_tag: u32,
    /// The scene node's position (`+0xbc0`) and its kept copy (`+0xc30`).
    pub node_pos: Vf,
    pub node_pos_kept: Vf,
}

/// Above this speed (ft/s) the crossup is measured.
const CROSSUP_SPEED: f32 = 10.0;
/// An up axis whose `y` is at most this is tipped.
const TIPPED_UP: f32 = 0.25;
const HALF_PI: f32 = 1.570_796_4;

/// **Untamed's air state** (SLUS-21701's bike PostProcess), once a frame after the ground step. It is
/// SLUS-21104's on Untamed's fields, with one change: a bike out of the world is unwrecked (vt
/// `+0x36c`, which ends its stunts and puts it back without the ground snap) where SLUS-21104 clears
/// the wreck reason and two more words and calls the put-back with the snap.
///
/// With no vehicle state, or a remote one (kinds 3 and 4), nothing. Else: when not balancing, the
/// balance cleared and the no-hander; the body's speed, its velocity into its frame, its axes, euler
/// angles and angular velocity into the world; on the ground with either wheel down, both down with
/// both; airborne the other way round, the hang time restarted on leaving the ground and counted while
/// off it; the contact velocity the first touching wheel's (front, then rear), else the body's own. The
/// crossup: neither flipping nor spinning and above 10 ft/s, the flattened heading against the
/// flattened velocity through the arc cosine table, ramped with the authority after a flip. The
/// collision record told the bike is tipped when its up axis's `y` is at most 0.25. Then the origin's
/// in-world test, the out-of-world event and unwreck, the vehicle's post step and the node's position,
/// kept in a second copy.
///
/// `call` is handed each call with the state, which it may change; its answer is read only for
/// [`AirCall::InWorld`] (true: inside).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn air_state(a: &mut AirState, dt: F, table: &crate::air::AcosTable, mut call: impl FnMut(AirCall, &mut AirState) -> bool) {
    use crate::air::{cross, map_clamp, normalize};
    use ps2_float::fpu::Acc;
    match a.vehicle_state {
        None => return,
        Some(k) if REMOTE.contains(&k) => return,
        Some(_) => {}
    }
    if a.wreck_reason == 0 && a.stunt_id != a.balance_id {
        call(AirCall::BalanceCleared, a);
        call(AirCall::NoHander(dt), a);
    }
    let v = a.body.velocity;
    a.speed = Acc::mula(F(v.x()), F(v.x())).madda(F(v.y()), F(v.y())).madd(F(v.z()), F(v.z())).sqrt();
    a.local_vel = a.body.to_local(v);
    let (fwd, up) = crate::math::quat_axes(a.body.orientation);
    a.heading = Vf::new(fwd[0].0, fwd[1].0, fwd[2].0, a.heading.w());
    a.up = Vf::new(up[0].0, up[1].0, up[2].0, a.up.w());
    a.euler = crate::math::quat_to_euler(a.body.orientation, table);
    a.world_ang_vel = a.body.to_world(a.body.ang_vel);

    a.was_airborne = a.airborne;
    let [front, rear] = a.wheel_touching.map(|t| t != 0);
    a.on_ground = u32::from(front || rear);
    a.both_down = u32::from(front && rear);
    a.airborne = u32::from(a.on_ground == 0);
    if a.was_airborne == 0 && a.airborne != 0 {
        a.hang_time = F::ZERO;
    }
    if a.airborne != 0 {
        a.hang_time = a.hang_time + dt;
    }
    a.contact_vel = if front {
        a.wheel_contact_vel[0]
    } else if rear {
        a.wheel_contact_vel[1]
    } else {
        a.local_vel
    };

    let heading = normalize(Vf::new(a.heading.x(), 0.0, a.heading.z(), 1.0));
    let mut angle = F::ZERO;
    if a.flipping == 0 && a.spin_ramp == 0 && !(a.speed <= F(CROSSUP_SPEED)) {
        let vel = normalize(Vf::new(v.x(), 0.0, v.z(), 1.0));
        let c = cross(heading, vel);
        let sin = Acc::mula(F(c.x()), F(c.x())).madda(F(c.y()), F(c.y())).madd(F(c.z()), F(c.z())).sqrt();
        let sin = clamp_unit(clamp_unit(clamp_unit(sin)));
        let t = table.entries();
        let s = (F::ONE + sin) * F(ACOS_SCALE);
        let i = s.to_i32();
        let frac = s - F(fpu::from_i32(i));
        let lerp = F(fpu::madd(fpu::mula(frac.0, t[i as usize + 1]), (F::ONE - frac).0, t[i as usize]));
        angle = F(HALF_PI) - lerp;
        if c.y() < 0.0 {
            angle = angle * F(-1.0);
        }
    }
    a.crossup = if !(a.authority <= F::ZERO) {
        if a.authority_time == F::ZERO { F::ZERO } else { map_clamp(a.authority, a.authority_time, F::ZERO, F::ZERO, angle) }
    } else {
        angle
    };
    a.tipped = u32::from(a.up.y() <= TIPPED_UP);

    let at = origin(&a.body, a.cofg);
    if !call(AirCall::InWorld(at), a) {
        let tag = a.event_tag;
        call(AirCall::Post { tag }, a);
        call(AirCall::Unwreck, a);
    }
    call(AirCall::VehiclePost(dt), a);
    call(AirCall::NodePosition, a);
    a.node_pos_kept = a.node_pos;
}

/// The bike's origin: the body's position less its centre of gravity (in its own frame) turned into
/// the world by its orientation, on VU0 as the vehicle's post step does it; `w` the position's.
fn origin(body: &crate::quad::Body, cofg: Vf) -> Vf {
    let turned = body.to_world(cofg);
    body.position.merge(Dest::XYZ, body.position.sub(turned))
}

/// **Untamed's hazard clock** (MD81b): [`super::tail::stuck`] with a third argument, `grounded` (some
/// wheel's ray down; the quad's update hands it as its own word, beside `on`, which is it and the
/// hazard surface together). With it clear, the free reset is passed over: the vehicle is not free,
/// stuck unless its run says not, and the clock goes on by `limit` as with `on` (−1 puts it back at
/// once, 0 or under zeroes the time, over 0 counts up and puts it back past it). With it set, it is
/// SLUS-21104's clock.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn stuck(m: &mut impl super::tail::Vehicles, on: bool, grounded: bool, dt: F, limit: F) {
    use super::tail::{NO_STUCK, Out};
    let v = m.vehicle();
    if grounded && (!on || (limit <= F::ZERO && limit != F(-1.0))) {
        (v.free, v.stuck, v.stuck_time) = (1, 0, F::ZERO);
        return;
    }
    v.free = 0;
    if v.run_flags & NO_STUCK == 0 {
        v.stuck = 1;
    }
    if limit == F(-1.0) {
        m.out(Out::PutBack, dt);
        m.vehicle().stuck_time = F::ZERO;
    } else if limit <= F::ZERO {
        v.stuck_time = F::ZERO;
    } else {
        v.stuck_time = v.stuck_time + dt;
        if !(v.stuck_time <= limit) {
            m.out(Out::PutBack, dt);
            m.vehicle().stuck_time = F::ZERO;
        }
    }
}
