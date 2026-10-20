//! The whole ride, saved and put back (N1): what online play's rollback copies every tick and puts
//! back when a late input arrives, and the bytes a host sends a peer that has fallen out of step.
//!
//! Taken from opentennis (`crates/hst-play/src/play/snapshot.rs`), with the shape changed where this
//! crate already had the answer:
//!
//! * **`save` and `restore` are a copy.** Everything a tick reads and writes is in [`Sim`] (lib.rs's
//!   rule 3), so a [`Snapshot`] is one and [`restore`] copies it back. opentennis needed a walk over a
//!   Bevy world's resources and entities for the same thing.
//! * **The ride's constants are shared, not copied.** [`Sim::tables`] and [`Sim::phy`] sit behind an
//!   `Arc`: a snapshot points at the same tables and the same bike as the state it came from, and the
//!   hash ([`crate::hash`]) has never walked either. The rider's four disc-read constants (the stunt
//!   set, the clip table, the wreck files and the body's numbers) are plain fields of
//!   [`crate::rider::Rider`] and are copied with it; they are a few hundred bytes.
//! * **The hash is the one the replay harness already uses**, [`Sim::hash`]. It is integer-only and
//!   the same on every machine (`tests/platforms.rs`), which is what a desync check between two
//!   machines needs and what opentennis's `Debug`-text hash could not promise.
//! * **[`encode`] and [`decode`] are this module's own byte codec**, not bincode: this crate takes no
//!   dependency that is not a sibling. Each struct's codec names every one of its fields — encoding
//!   destructures it and decoding builds it with a struct literal — so a field added to the state and
//!   not to its codec is a build failure, and a resync can never quietly drop it. The constants stay
//!   this machine's own, as opentennis's `Const` does: [`decode`] keeps the local ones.
//!
//! What is *not* here, because it is not the simulation's: the world under the wheels is an argument
//! of [`Sim::frame`], read and never written (the terrain and the collision meshes; nothing on a track
//! moves yet), and the follow camera is the shell's — nothing in a tick reads it back.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use std::sync::Arc;

use ps2_float::{F, Vf};

use crate::Sim;

/// Everything a tick reads and writes, copied. The constants inside it are shared with the state it
/// was saved from (see the module docs).
#[derive(Clone, Debug)]
pub struct Snapshot(Sim);

impl Snapshot {
    /// The state as it was saved: what a desync report or a test reads without restoring it.
    pub fn sim(&self) -> &Sim {
        &self.0
    }

    /// The saved state's hash, without restoring it: what rollback hands its session as the
    /// checksum of the frame it saved.
    pub fn hash(&self) -> u64 {
        self.0.hash()
    }
}

/// The state now, for [`restore`] to put back.
pub fn save(sim: &Sim) -> Snapshot {
    Snapshot(sim.clone())
}

/// Put a saved state back.
pub fn restore(sim: &mut Sim, s: &Snapshot) {
    sim.clone_from(&s.0);
}

/// [`save`] into a snapshot that already exists: what a rollback ring that saves every tick wants.
pub fn save_into(sim: &Sim, s: &mut Snapshot) {
    s.0.clone_from(sim);
}

/// What a resync's bytes begin with: the format, so that two builds that would read each other's
/// state differently refuse rather than desync again. Bump it when the codec changes shape in a way
/// the field list does not already catch.
const MAGIC: [u8; 4] = *b"OMX1";

/// The state for another machine: every field a tick reads and writes, as bytes, the constants
/// ([`Sim::tables`], [`Sim::phy`] and the rider's four disc-read fields) left out.
pub fn encode(sim: &Sim) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 * 1024);
    out.extend_from_slice(&MAGIC);
    let Sim { ticks, rng, tables: _, phy: _, input, rider, plane, others } = sim;
    ticks.put(&mut out);
    rng.put(&mut out);
    input.put(&mut out);
    rider.put(&mut out);
    plane.put(&mut out);
    // S54's players after the first, only when there are any, so a one-player state's bytes are
    // what they were before there could be a second. Each one's `.phy` is a constant, kept local.
    if !others.is_empty() {
        let riders: Vec<&Rider> = others.iter().map(|p| &p.rider).collect();
        riders.len().put(&mut out);
        riders.iter().for_each(|r| r.put(&mut out));
    }
    out
}

/// [`encode`]'s bytes into this machine's state. The constants stay this machine's own: the session
/// agreed them before the ride began (the bike, the stunt set, the track), and a peer's are not this
/// one's to overwrite. Nothing is written unless the whole state decodes.
pub fn decode(sim: &mut Sim, bytes: &[u8]) -> Result<(), String> {
    let mut r = Reader { bytes, at: 0 };
    if r.bytes(4)? != MAGIC {
        return Err("not a ride state (or another build's)".into());
    }
    let ticks = Wire::take(&mut r)?;
    let rng = Wire::take(&mut r)?;
    let input = Wire::take(&mut r)?;
    let mut rider: crate::rider::Rider = Wire::take(&mut r)?;
    let plane = Wire::take(&mut r)?;
    let mut others_riders: Vec<Rider> = if r.at < bytes.len() { Wire::take(&mut r)? } else { Vec::new() };
    if r.at != bytes.len() {
        return Err(format!("{} bytes left over after the state", bytes.len() - r.at));
    }
    let here = &sim.rider;
    rider.set = here.set.clone();
    rider.clips = here.clips;
    rider.wreckage = here.wreckage;
    rider.ragdoll_kit = here.ragdoll_kit.clone();
    rider.body_numbers = here.body_numbers;
    if others_riders.len() != sim.others.len() {
        return Err(format!("{} players in the state, {} here", 1 + others_riders.len(), sim.players()));
    }
    let mut others = sim.others.clone();
    for (p, mut r) in others.iter_mut().zip(others_riders.drain(..)) {
        r.set = p.rider.set.clone();
        r.clips = p.rider.clips;
        r.wreckage = p.rider.wreckage;
        r.ragdoll_kit = p.rider.ragdoll_kit.clone();
        r.body_numbers = p.rider.body_numbers;
        p.rider = r;
    }
    *sim = Sim {
        ticks,
        rng,
        tables: Arc::clone(&sim.tables),
        phy: Arc::clone(&sim.phy),
        input,
        rider,
        plane,
        others,
    };
    Ok(())
}

// ---------------------------------------------------------------------------------------------------
// The codec.
// ---------------------------------------------------------------------------------------------------

/// The bytes being decoded, and how far in.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.bytes.len());
        let end = end.ok_or_else(|| format!("the state ends early (byte {} of {})", self.at, self.bytes.len()))?;
        let b = &self.bytes[self.at..end];
        self.at = end;
        Ok(b)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], String> {
        Ok(self.bytes(N)?.try_into().expect("a slice of N bytes"))
    }
}

/// A part of the state that can go to another machine and back. Little-endian, a float as its bit
/// pattern, a length as eight bytes whatever a `usize` is — the same rules as the hash, so the bytes
/// are the same on every machine the port runs on.
pub trait Wire: Sized {
    fn put(&self, out: &mut Vec<u8>);
    fn take(r: &mut Reader<'_>) -> Result<Self, String>;
}

macro_rules! wire_int {
    ($($t:ty),*) => {$(
        impl Wire for $t {
            fn put(&self, out: &mut Vec<u8>) {
                out.extend_from_slice(&self.to_le_bytes());
            }
            fn take(r: &mut Reader<'_>) -> Result<Self, String> {
                Ok(<$t>::from_le_bytes(r.array()?))
            }
        }
    )*};
}

wire_int!(u8, u16, u32, u64, i8, i16, i32, i64);

impl Wire for usize {
    fn put(&self, out: &mut Vec<u8>) {
        (*self as u64).put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        usize::try_from(u64::take(r)?).map_err(|_| "a length too long for this machine".to_string())
    }
}

impl Wire for bool {
    fn put(&self, out: &mut Vec<u8>) {
        out.push(u8::from(*self));
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        match u8::take(r)? {
            0 => Ok(false),
            1 => Ok(true),
            b => Err(format!("a flag of {b}")),
        }
    }
}

impl Wire for f32 {
    fn put(&self, out: &mut Vec<u8>) {
        self.to_bits().put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(f32::from_bits(u32::take(r)?))
    }
}

impl Wire for F {
    fn put(&self, out: &mut Vec<u8>) {
        self.to_bits().put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(F::from_bits(u32::take(r)?))
    }
}

impl Wire for Vf {
    fn put(&self, out: &mut Vec<u8>) {
        self.to_bits().put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(Vf::from_bits(Wire::take(r)?))
    }
}

impl Wire for String {
    fn put(&self, out: &mut Vec<u8>) {
        self.len().put(out);
        out.extend_from_slice(self.as_bytes());
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        let n = usize::take(r)?;
        String::from_utf8(r.bytes(n)?.to_vec()).map_err(|e| e.to_string())
    }
}

impl<T: Wire, const N: usize> Wire for [T; N] {
    fn put(&self, out: &mut Vec<u8>) {
        self.iter().for_each(|x| x.put(out));
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        let v = (0..N).map(|_| T::take(r)).collect::<Result<Vec<_>, _>>()?;
        Ok(v.try_into().unwrap_or_else(|_| unreachable!("N items were taken")))
    }
}

impl<T: Wire> Wire for Vec<T> {
    fn put(&self, out: &mut Vec<u8>) {
        self.len().put(out);
        self.iter().for_each(|x| x.put(out));
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        let n = usize::take(r)?;
        // Each item is at least a byte, so a length past what is left is a corrupt state, not a
        // reason to reserve gigabytes.
        if n > r.bytes.len() - r.at {
            return Err(format!("a list of {n} with {} bytes left", r.bytes.len() - r.at));
        }
        (0..n).map(|_| T::take(r)).collect()
    }
}

impl<T: Wire> Wire for Option<T> {
    fn put(&self, out: &mut Vec<u8>) {
        self.is_some().put(out);
        if let Some(x) = self {
            x.put(out);
        }
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(if bool::take(r)? { Some(T::take(r)?) } else { None })
    }
}

impl<A: Wire, B: Wire> Wire for (A, B) {
    fn put(&self, out: &mut Vec<u8>) {
        self.0.put(out);
        self.1.put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok((A::take(r)?, B::take(r)?))
    }
}

impl<A: Wire, B: Wire, C: Wire> Wire for (A, B, C) {
    fn put(&self, out: &mut Vec<u8>) {
        self.0.put(out);
        self.1.put(out);
        self.2.put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok((A::take(r)?, B::take(r)?, C::take(r)?))
    }
}

/// A struct's codec from the list of its fields. The list must be complete: encoding destructures
/// the struct by it and decoding builds the struct from it, so a missing field does not compile.
macro_rules! wire {
    ($t:ty { $($f:ident),* $(,)? }) => {
        impl Wire for $t {
            fn put(&self, out: &mut Vec<u8>) {
                let Self { $($f),* } = self;
                $( $f.put(out); )*
            }
            fn take(r: &mut Reader<'_>) -> Result<Self, String> {
                Ok(Self { $($f: Wire::take(r)?),* })
            }
        }
    };
    ($t:ty; $($f:ident),* $(,)?) => {
        impl Wire for $t {
            fn put(&self, out: &mut Vec<u8>) {
                let Self ( $($f),* ) = self;
                $( $f.put(out); )*
            }
            fn take(r: &mut Reader<'_>) -> Result<Self, String> {
                Ok(Self ( $(wire!(@take r $f)),* ))
            }
        }
    };
    (@take $r:ident $f:ident) => { Wire::take($r)? };
}

/// An enum whose variants carry nothing, as its variant's position in the list. The list must be
/// complete: the encoding matches on it.
macro_rules! wire_enum {
    ($t:ty { $($v:ident),* $(,)? }) => {
        impl Wire for $t {
            fn put(&self, out: &mut Vec<u8>) {
                let all = [$(<$t>::$v),*];
                let i = match self { $(<$t>::$v => all.iter().position(|x| matches!(x, <$t>::$v)),)* };
                (i.expect("listed") as u8).put(out);
            }
            fn take(r: &mut Reader<'_>) -> Result<Self, String> {
                let all = [$(<$t>::$v),*];
                let i = u8::take(r)?;
                all.into_iter().nth(usize::from(i)).ok_or_else(|| format!("{} has no variant {i}", stringify!($t)))
            }
        }
    };
}

// ---------------------------------------------------------------------------------------------------
// The state's own types, field by field.
// ---------------------------------------------------------------------------------------------------

use crate::rider::Rider;

impl Wire for Rider {
    /// The rider's disc-read constants (the stunt set, the clip table, the wreck files and the body's
    /// numbers) are not sent: [`decode`] puts this machine's own back.
    fn put(&self, out: &mut Vec<u8>) {
        let Rider {
            ride,
            tricks,
            state,
            motion,
            set: _,
            clips: _,
            sense,
            racing,
            wreckage: _,
            crash,
            ragdoll_kit: _,
            ragdoll,
            wreck_sounds,
            animation,
            body,
            body_numbers: _,
            body_rates,
            idles,
            body_clocked,
            body_request,
            flight,
            airborne,
        } = self;
        ride.put(out);
        tricks.put(out);
        state.put(out);
        motion.put(out);
        sense.put(out);
        racing.put(out);
        crash.put(out);
        ragdoll.put(out);
        wreck_sounds.put(out);
        animation.put(out);
        body.put(out);
        body_rates.put(out);
        idles.put(out);
        body_clocked.put(out);
        body_request.put(out);
        flight.put(out);
        airborne.put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(Rider {
            ride: Wire::take(r)?,
            tricks: Wire::take(r)?,
            state: Wire::take(r)?,
            motion: Wire::take(r)?,
            set: Default::default(),
            clips: Default::default(),
            sense: Wire::take(r)?,
            racing: Wire::take(r)?,
            wreckage: None,
            crash: Wire::take(r)?,
            ragdoll_kit: None,
            ragdoll: Wire::take(r)?,
            wreck_sounds: Wire::take(r)?,
            animation: Wire::take(r)?,
            body: Wire::take(r)?,
            body_numbers: Default::default(),
            body_rates: Wire::take(r)?,
            idles: Wire::take(r)?,
            body_clocked: Wire::take(r)?,
            body_request: Wire::take(r)?,
            flight: Wire::take(r)?,
            airborne: Wire::take(r)?,
        })
    }
}

wire!(crate::Input { buttons, stick });
wire!(crate::ragdoll_ride::Down { reason, ragdoll, contacts, held_at, sensed });
wire!(crate::ragdoll::Ragdoll { records, bones, joints, collisions, world, holds, bike_box, throwing, launch, ignores_bike, entered, stiff, riding, off_bike, flung, settled, roll, pins, pull });
wire!(crate::ragdoll::Records { parts, joints, collisions });
wire!(crate::ragdoll::PartRecord { ride_mass, wreck_mass, ride_inertia, wreck_inertia, ride_angular_damping, settled_angular_damping, limp_angular_damping });
wire!(crate::ragdoll::JointRecord { part1, part2, max_angle, ride_spring, ride_damping, wreck_spring, wreck_damping });
wire!(crate::ragdoll::Bone { body, mass, inertia, reference });
wire!(crate::ragdoll::Joint { wrecked, max_angle, spring, damping, critical });
wire!(crate::ragdoll::Collision { bone, enabled, touching });
wire!(crate::ragdoll::Launch { force, force_time, hand_hold, target, world_frame, other_frame });
wire!(crate::ragdoll::Pin { first, second, anchors, target, target_velocity, lower, upper, hit, sticky, gains });
wire!(crate::biplane::World { iterations, rate });
wire!(crate::ragdoll_contact::Contact { touched, ground, scene, swept, point, normal, dv });
wire!(crate::rider_anim::Thrown { throw, touching, vel, clearance, launch });
wire!(crate::wreck_sound::Emitter { crash, fall });
wire!(crate::wreck_sound::List { count, last });
wire!(crate::rider_anim::Clocked { played, snapshot });
wire!(crate::flight::Flight { joystick, autopilot, engine, aero, body, measure, tires, pad, surfaces, attitude, air, crash, gravity, gravity_scale, mass, inv_inertia, edge_scale, wrecks, spawn, area, noise, rng });
wire!(crate::input::Manager { pads, events, time_ms });
wire!(crate::rider_anim::Pool { used, count, rng, seeded });
wire!(crate::state::Sense { ai_route, owner, wrecks_enabled, settles });
wire!(crate::trick::Trick; f0);
wire!(crate::trick::Tricks { gesture, queue, requested, current, restart, seq, prev_seq, elapsed, progress, hold_elapsed, wiggle_time, held, hold_done, in_progress, safe, no_hander, hand_hold });
wire!(crate::motion::Driver { action, ramp, faded, fore_aft, pose, shown_crossup });
wire!(crate::motion::Frame { blend, restart });
wire!(crate::ride::Crash { reason, hit, throw, clock, holding, throwing, body, origin, rider, rider_vel, landed, ground, resting, facing });
wire!(crate::ride::Ride { mv, chassis, engine, air, front, rear, origin, node, held, spawn, spawn_facing, near, touching, stunt });
wire!(crate::rider_anim::Body { veh, player, ready, picked, look, looking, settled, machine });
wire!(crate::state::Ride { wreck, counting, elapsed, kind, pending, fresh, wrecking, wrecks, entries });
wire!(crate::vehicle_anim::Params { rate_stunt, rate_default, rate_pose, rate_hold, rate_ready, cross_cap, start_phase, band });
wire!(crate::vehicle_anim::Request { held, at_base, cut, offset, base, rotation, phase_axis, mix_axis, band, progress, seq });
wire!(crate::biplane::Area { width, depth });
wire!(crate::biplane::Attitude { flown, grounded, nose_y, roll, speed, pitch_rate });
wire!(crate::input::EventQueue { count, events });
wire!(crate::ride::Held { gas, brake, front_brake, rear_brake, clutch, stick, digital, lean, flick, ring });
wire!(crate::rider_anim::Machine { mode, prev, places_itself, thrown, loose, spin, roll, touching, flung, stiff });
wire!(crate::biplane::Measure { grounded, was_grounded, airspeed, alpha, beta, world_rates, velocity, forward, up, euler, matrix });
wire!(crate::state::Owner { human, screen });
wire!(crate::phy::PlaneWrecks { wreck_time, min_wreck_speed, wreck_gravity, wreck_damping, wreck_inertia });
wire!(crate::motion::Played { window, fade, looped, finished });
wire!(crate::motion::Player { clip, last, time, prev, finished, fade });
wire!(crate::noise::SmoothNoise { perm, g1 });
wire!(crate::phy::Throw { force_time, hand_hold_time, force, force_time_delta, hand_hold_time_delta, force_delta });
wire!(crate::air::Air { on_ground, both_wheels, airborne, was_airborne, hang_time, contact_vel, crossup, authority, preload_ready, preload_firing, preload_armed, preload_timer, preload_force, flip_torque_applied, flipping, flip_active, flip_armed, flip_grace, flip_torque, wreck_impact });
wire!(crate::biplane::Aero { true_speed, density, chord, span, area, default_alpha, cm_break_alpha, lift_alpha, lift_round_off, lift_drop_off, lift_scale, lift_angle_scale, lift_q, lift_de, drag0, drag1, drag_de, side_beta, side_da, side_dr, side_p, side_r, cl_beta, cl_p, cl_r, cl_da, cl_dr, cm0, cm_alpha, cm_q, cm_de, cm_stall, cn_beta, cn_p, cn_r, cn_dr, cn_da, lift, drag, side, roll, pitch });
wire!(crate::biplane::Air { alpha, beta, speed, elevator, aileron, rudder, rates });
wire!(crate::biplane::Autopilot { aileron_dead_zone, max_pitch_rate, max_roll, feed_forward, gain, integrator_gain });
wire!(crate::biplane::Body { enabled, inv_mass, damping, last_position, position, velocity, last_force, force, accel, shock_peak, shock, step, inv_inertia, ang_damping, last_quat, quat, cofg, moment, last_moment, rates, ang_accel, twist_peak, twist });
wire!(crate::biplane::Crash { reason, landed, min_wreck_speed, wreck_time, clock });
wire!(crate::biplane::Engine { min_efficiency, full_efficiency_speed, prop_rpm, prop_angle, max_power, fade_altitude, max_altitude, power, rpm, idle_rpm, max_rpm, thrust, note, clock });
wire!(crate::biplane::Joystick { throttle_increment, reverse_throttle, aileron_rate, elevator_rate, rudder_rate, max_elevator, max_aileron, max_rudder, ramp_off_roll, off_roll, min_elevator });
wire!(crate::biplane::Pad { forward, back, elevator, aileron, rudder });
wire!(crate::biplane::Surfaces { throttle, brake, elevator, elevator_command, aileron, rudder, steer, pitch_integral });
wire!(crate::biplane::Tire { max_compression, max_decompression_rate, spring, bounce, rebound, center, radius, inv_radius, max_braking, side_force, brake_friction, steering_moment, braked, steered, offset, below, held, touching, compression, last_compression, spring_force, bottom, low, surface, ground, normal, contact, hub, contact_velocity, angle, spin, last_normal, slip, friction, frame });
wire!(crate::chassis::Chassis { speed, cofg_world, accel, ang_accel, euler, inv_inertia, force, torque, impulse, correction, cofg, quat, awake, contacts, inv_mass, max_speed_sq, max_ang_speed_sq, ang_vel_max, ang_vel_min, vel, local_vel, world_ang_vel, ang_vel, free, ang_free, pos_hist, vel_hist, ang_hist, quat_hist });
wire!(crate::input::Pad { device, index, connected, controls, axis_raw, axis_value, axes_centred });
wire_enum!(crate::phy::Hit { Front, Side, Back, Bottom, FallOffFront, FallOffBack, Stunt });
wire!(crate::r#move::Engine { gear, gears, final_drive, gear_ratios, ratio, inv_ratio, clutch_torque_scale, clutch_locked, rev_limit, idle_throttle, engine_braking, inv_inertia, clutch_static, clutch_kinetic, min_rpm, curve_rpm, curve_torque, curve, torque, wheel_torque, rpm, road_rpm, torque_scale });
wire!(crate::r#move::Ride { g_peak, clutch_timer, clutch_scale_start, clutch_cofg_weight, clutch_in, collision_near, balance_inertia, balance_command, balance_point, balance_noise_time, pitch_integral, stoppie });
wire!(crate::ride::Wheel { mv, w, surface, inv_inertia, steer_axis, slip });
wire!(crate::tire::Slip { spinning, locked, sliding });
wire!(crate::rng::Rng { state });
wire_enum!(crate::state::Kind { Plain, Wrecked });
wire!(crate::trick::Gesture { group, dir, stick, look, armed });
wire!(crate::trick::Wreck; f0);
wire!(crate::vehicle_anim::Driver { want, state, prev, seq, rate, ramp, started, holding, from_table, cross, cross_prev, cross_start, reverse, extra, mix, phase });
wire!(crate::motion::ClipId; f0);
wire!(crate::input::ControlState { held, strength, since_release_ms, held_ms, prev_since_release_ms, prev_held_ms });

wire!(crate::input::Event { control, kind, device, index, pressed, stamp });
wire!(crate::motion::Fade { on, from, clock, delay, length, end });
wire!(crate::ride::Flick { now, head, at, value });
wire!(crate::trick::Look { angle, reach, dir });
wire!(crate::chassis::Mat3 { rows });
wire!(crate::math::Mat4; f0);
wire_enum!(crate::rider_anim::Mode { Held, Riding, Wrecked, Posed, Gate });
wire!(crate::trick::Stick { x, y, digital });
wire!(crate::motion::Cross { weight, from, at });
wire!(crate::r#move::Wheel { balancing, donut_scale, spring, max_steer_angle, steer_command, inv_radius, spin, contact, other_tire_down, drive_torque, radius, spin_from_torque, contact_force_fwd, contact_normal, steered, fresh });
wire_enum!(crate::trick::Dir { None, Up, Down, Left, Right });
wire!(crate::vehicle_anim::Lag { value, tau, ratio });
wire!(crate::wheel::Wheel { has_ground, mount, mount_world, probe, probe_end, probe_end_world, probe_length, ground_normal, ray_steps, ray_hit, swept_contact, terrain_wins, contact_point, contact_normal, max_compression, max_damper_speed, extension_rate, spring_rate, damping_out, damping_in, contact, compression, last_compression, spring_force, load_point, contact_velocity, chassis_velocity, steered, balancing, steer_torque, steer_damping, steer_inv_inertia, max_steer_angle, min_steer_angle, steer_angle, steer_rate });
wire!(crate::r#move::Curve { points, index, frac_bits });
wire!(crate::r#move::Fresh { scalars, vectors });

impl<const N: usize> Wire for crate::chassis::Deriv<N> {
    fn put(&self, out: &mut Vec<u8>) {
        let Self { dx, prev } = self;
        dx.put(out);
        prev.put(out);
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(Self { dx: Wire::take(r)?, prev: Wire::take(r)? })
    }
}

impl<T: Wire> Wire for std::collections::VecDeque<T> {
    fn put(&self, out: &mut Vec<u8>) {
        self.len().put(out);
        self.iter().for_each(|x| x.put(out));
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(Vec::<T>::take(r)?.into())
    }
}

impl Wire for crate::motion::Window {
    fn put(&self, out: &mut Vec<u8>) {
        use crate::motion::Window::*;
        match self {
            None => 0u8.put(out),
            Span(a, b) => {
                1u8.put(out);
                (*a, *b).put(out);
            }
            Wrapped { tail, head } => {
                2u8.put(out);
                (*tail, *head).put(out);
            }
        }
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        use crate::motion::Window::*;
        Ok(match u8::take(r)? {
            0 => None,
            1 => {
                let (a, b) = Wire::take(r)?;
                Span(a, b)
            }
            2 => Wrapped { tail: Wire::take(r)?, head: Wire::take(r)? },
            t => return Err(format!("motion::Window has no variant {t}")),
        })
    }
}

impl Wire for crate::motion::Slot {
    fn put(&self, out: &mut Vec<u8>) {
        use crate::motion::Slot::*;
        match self {
            Stunt(i) => {
                0u8.put(out);
                i.put(out);
            }
            AirFwd => 1u8.put(out),
            AirMid => 2u8.put(out),
            AirBck => 3u8.put(out),
        }
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        use crate::motion::Slot::*;
        Ok(match u8::take(r)? {
            0 => Stunt(Wire::take(r)?),
            1 => AirFwd,
            2 => AirMid,
            3 => AirBck,
            t => return Err(format!("motion::Slot has no variant {t}")),
        })
    }
}

impl Wire for crate::motion::Blend {
    fn put(&self, out: &mut Vec<u8>) {
        use crate::motion::Blend::*;
        match self {
            Free => 0u8.put(out),
            One { phase, ramp, clip } => {
                1u8.put(out);
                (*phase, *ramp, *clip).put(out);
            }
            Two { phase, mix, ramp, a, b } => {
                2u8.put(out);
                (*phase, *mix, *ramp).put(out);
                (*a, *b).put(out);
            }
            Three { phase, mix, cross, ramp, a, b, c, d } => {
                3u8.put(out);
                (*phase, *mix, *cross).put(out);
                ramp.put(out);
                (*a, *b).put(out);
                (*c, *d).put(out);
            }
        }
    }
    fn take(r: &mut Reader<'_>) -> Result<Self, String> {
        use crate::motion::Blend::*;
        let t = u8::take(r)?;
        Ok(match t {
            0 => Free,
            1 => One { phase: Wire::take(r)?, ramp: Wire::take(r)?, clip: Wire::take(r)? },
            2 => Two { phase: Wire::take(r)?, mix: Wire::take(r)?, ramp: Wire::take(r)?, a: Wire::take(r)?, b: Wire::take(r)? },
            3 => Three {
                phase: Wire::take(r)?,
                mix: Wire::take(r)?,
                cross: Wire::take(r)?,
                ramp: Wire::take(r)?,
                a: Wire::take(r)?,
                b: Wire::take(r)?,
                c: Wire::take(r)?,
                d: Wire::take(r)?,
            },
            t => return Err(format!("motion::Blend has no variant {t}")),
        })
    }
}
