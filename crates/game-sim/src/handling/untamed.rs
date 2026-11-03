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
//!   the stick before the limiter, and a flip's end clears a stunt id over 48 (SLUS-21104: 40).
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
    /// The stick's fore-aft as the lip reads it: the reader's copy taken before the d-pad and the
    /// limiter have had their say on [`AirStep::stick`] (which the flight's decay reads).
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
/// Untamed's input. The port's controls have three trick buttons and no backflip binding, so the arming
/// button is R1 (the one SLUS-21104 arms on), a trick is held when any of the three is, and the lip
/// reads the step's stick and flick ([`FlipIn::lip_y`], [`FlipIn::hold`]) with no bound control.
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
