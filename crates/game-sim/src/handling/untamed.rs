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
