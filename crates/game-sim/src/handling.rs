//! **Handling (MD40): the original's, or MX vs. ATV's.** MX vs. ATV Unleashed (SLUS-21104) rides on
//! MX Unleashed's engine with the bike's steps retuned and a few rewritten. This module holds the
//! setting and the steps ported from MX vs. ATV's ELF as they read there, each proved against that
//! ELF's own code (`tests/mxvatv_air.rs`, a RAM image of a Free Ride in SLUS-21104).
//!
//! MD62 adds its airborne control ([`airborne_control`]): the gyro options and `GasGyroTorque`.
//!
//! [`Handling::Original`] is the default, and on it nothing here runs: the ride calls
//! [`crate::air::backflip`] as it always did, so every replay hash is unchanged.
//!
//! What MX vs. ATV's backflip does differently from MX Unleashed's ([`crate::air::backflip`]):
//!
//! - it arms only with R1 held alone (the trick buttons' nibble reads 4), so the stick's pull on a
//!   lip with no button is a preload and nothing else;
//! - its lip refuses while the 360 put torque in or is triggered;
//! - in the air its torque decays only with no trick button held and the stick at or behind centre,
//!   at `BackflipTorqueRateDecrease`, and it is not clamped to the maximum afterwards;
//! - it ends once the torque is *under* its floor (MX Unleashed's: at or under);
//! - it runs a clip state for the rider's backflip stunt (the 360 has one too, [`crate::air::spin_clip`])
//!   from the bike's frame, and a flip that ends with the clip still running is a wreck.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::air::{Air, Chassis, Step};
use ps2_float::vu::Dest;
use ps2_float::{F, Vf, fpu};

pub mod step;

/// How the bikes handle (MD40), as `settings.txt`'s `handling` and `--handling` spell it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Handling {
    /// MX Unleashed's: its bikes' `.phy` and its steps. Every replay hash is this.
    #[default]
    Original,
    /// MX vs. ATV Unleashed's: its own `mx_*.phy` and the steps it changed, ported from SLUS-21104.
    MxVsAtv,
}

impl Handling {
    pub const ALL: [Handling; 2] = [Handling::Original, Handling::MxVsAtv];

    pub fn word(self) -> &'static str {
        match self {
            Handling::Original => "original",
            Handling::MxVsAtv => "mxvatv",
        }
    }

    pub fn from_word(w: &str) -> Option<Handling> {
        Handling::ALL.into_iter().find(|h| h.word() == w)
    }

    pub fn label(self) -> &'static str {
        match self {
            Handling::Original => "Original",
            Handling::MxVsAtv => "MX vs. ATV",
        }
    }
}

/// The trick buttons' nibble MX vs. ATV's steps test: bit 0..2 the three trick buttons as
/// [`crate::input::Controls::trick`] orders them, R1 the third. 4 is R1 alone, 0 none.
pub fn trick_nibble(trick: [bool; 3]) -> u8 {
    u8::from(trick[0]) | u8::from(trick[1]) << 1 | u8::from(trick[2]) << 2
}

/// The trick buttons read R1 alone: what arms MX vs. ATV's backflip.
pub const R1_ALONE: u8 = 4;

/// MX vs. ATV's backflip: what it reads beyond MX Unleashed's [`Step`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlipIn {
    /// The trick buttons' nibble ([`trick_nibble`]).
    pub buttons: u8,
    /// The bike frame's `[0].y`, its right axis' height (the node's world rows are x, y, z, and the
    /// original reads `+0x4` of the first): the clip starts and plays on with it at or under zero, and
    /// stops with it over.
    pub right_y: F,
    /// The bike frame's `[1].y`, its up axis' height.
    pub up_y: F,
    /// The rider's current stunt id: a flip that ends clears one over 40 (to 255).
    pub stunt_id: i32,
}

/// The backflip's clip, as MX vs. ATV keeps it for the rider's backflip stunt: 0 none, 1 started (the
/// bike over on its back), 2 playing (past upside down).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FlipClip(pub i32);

/// What one call asked of the rest of the bike: the rider's stunt calls and the wreck.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FlipOut {
    /// The backflip stunt was asked to start (and the clip went to 1 only if it did).
    pub stunt_started: bool,
    /// The backflip stunt was asked to end (the clip came round: 2 to 0).
    pub stunt_ended: bool,
    /// The rider's current stunt id was cleared (over 40, set to 255) as the flip ended.
    pub stunt_cleared: bool,
    /// The flip ended with its clip running: the wreck, `0xb`, or `0xf` when [`backflip`]'s
    /// `steep` answered yes.
    pub wreck: Option<u8>,
}

/// One substep of MX vs. ATV's backflip (SLUS-21104's, called first of the rotations, before its
/// 360), over MX Unleashed's backflip state on [`Air`].
///
/// `start_stunt` asks the rider to start the backflip stunt and says whether it did; `steep` is the
/// wreck's own test of which reason a mid-clip landing is (true: `0xf`). Both are called only where
/// the original calls them.
pub fn backflip(
    p: &crate::air::Params,
    air: &mut Air,
    chassis: &mut Chassis,
    step: &Step,
    inp: &FlipIn,
    clip: &mut FlipClip,
    start_stunt: impl FnOnce() -> bool,
    steep: impl FnOnce() -> bool,
) -> FlipOut {
    let mut out = FlipOut::default();
    air.flip_torque_applied = false;
    let y = step.stick.1;
    if !air.flip_active {
        let lip = air.on_ground
            && !step.spinning
            && !step.spin_active
            && chassis.vel.y() > 5.0
            && air.contact_vel.z() > 30.0
            && step.brake == 0.0;
        if !lip {
            air.flip_active = false;
            air.flip_armed = false;
        } else if y < F(-0.8) && inp.buttons & 0xf == R1_ALONE {
            air.flip_armed = true;
        } else if y > F(0.8) && air.flip_armed {
            let lo = p.min_backflip_torque * F(1.25);
            let hold = step.stick_hold.abs();
            let torque = if p.min_backflip_time == p.max_backflip_time {
                // The compiler's branch delay slot leaves the map's numerator here, not the floor: the
                // torque on a bike whose two times are equal is the hold less the minimum time.
                hold - p.min_backflip_time
            } else {
                let u = (hold - p.min_backflip_time) / (p.max_backflip_time - p.min_backflip_time);
                let hi = p.max_backflip_torque;
                let v = F(fpu::madd(fpu::adda(0.0, lo.0), u.0, (hi - lo).0));
                v.max(lo.min(hi)).min(lo.max(hi))
            };
            air.flip_torque = Vf::new((-torque).0, air.flip_torque.y(), air.flip_torque.z(), air.flip_torque.w());
            air.flip_armed = false;
            air.flip_active = true;
            air.flip_grace = F::from_bits(0x3f19_999a);
        }
    } else if !air.flip_armed {
        if !air.on_ground {
            air.flip_armed = true;
        } else {
            air.flip_grace = air.flip_grace - step.dt;
            if air.flip_grace <= 0.0 {
                air.flip_active = false;
                air.flip_armed = false;
            }
        }
    } else if air.on_ground || step.touching || F(air.flip_torque.x()).abs() < p.min_backflip_torque {
        air.flip_active = false;
        air.flip_armed = false;
        if inp.stunt_id > 40 {
            out.stunt_cleared = true;
        }
        if clip.0 != 0 {
            clip.0 = 0;
            out.wreck = Some(if steep() { 0xf } else { 0xb });
        }
    } else {
        if inp.buttons & 0xf == 0 && y <= 0.0 {
            let t = F(fpu::msub(fpu::adda(0.0, air.flip_torque.x()), step.dt.0, (y * p.backflip_torque_rate_decrease).0));
            air.flip_torque = Vf::new(t.0, air.flip_torque.y(), air.flip_torque.z(), air.flip_torque.w());
        }
        chassis.moment = chassis.moment.merge(Dest::XYZ, chassis.moment.add(air.flip_torque));
        air.flip_torque_applied = true;
        air.flipping = true;
        if inp.right_y <= 0.0 {
            if inp.up_y > 0.0 && clip.0 == 0 {
                out.stunt_started = start_stunt();
                if out.stunt_started {
                    clip.0 = 1;
                }
            } else if inp.up_y < 0.0 && clip.0 == 1 {
                clip.0 = 2;
            }
        } else if inp.up_y > F(-0.79) && clip.0 == 2 {
            out.stunt_ended = true;
            clip.0 = 0;
        }
    }
    if air.flipping {
        let back = air.on_ground
            || (!air.flip_torque_applied
                && chassis.roll.abs() < F::from_bits(0x3fc9_0fdb)
                && chassis.pitch > -p.airborne_pitch_down_wreck_angle
                && chassis.pitch < p.airborne_pitch_up_wreck_angle);
        if back {
            air.flipping = false;
            air.authority = p.control_authority_time;
        }
    }
    out
}

/// **The port's own:** the ride's state for MX vs. ATV's steps, on [`crate::ride::Ride::mxvatv`] —
/// made on the first frame of a bike on [`Handling::MxVsAtv`], `None` on every other, so their hashes
/// and snapshots do not move.
#[derive(Clone, Copy, Debug, Default)]
pub struct Flip {
    /// The backflip's clip ([`FlipClip`]).
    pub clip: i32,
    /// Latches the rider reads and clears, as [`crate::air::Spin`]'s: the flip ended with its clip
    /// running (the wreck), and that was in the air rather than on a touch.
    pub wrecked: bool,
    pub cut: bool,
    /// The backflip stunt came round (its clip 2 to 0).
    pub came_round: bool,
    /// The preload's cue latch ([`step::preload`]), MD63.
    pub preload_cue: bool,
}

impl crate::hash::State for Flip {
    fn walk(&self, w: &mut crate::hash::Walk<'_>) {
        w.i32("clip", self.clip);
        w.bool("wrecked", self.wrecked);
        w.bool("cut", self.cut);
        w.bool("came_round", self.came_round);
        w.bool("preload_cue", self.preload_cue);
    }
}

/// [`backflip`] on a ride: the trick buttons and the bike's frame read off the ride, the rider's
/// stunt taken as started whenever it is asked (the port's rider has no stunt to refuse it with), and
/// the wreck latched on `flip` for the rider, as the 360's is.
pub fn ride_backflip(p: &crate::air::Params, air: &mut Air, chassis: &mut Chassis, step: &Step, trick: [bool; 3],
                     frame: &crate::chassis::Mat3, flip: &mut Flip) {
    let inp = FlipIn { buttons: trick_nibble(trick), right_y: F(frame.rows[0].y()), up_y: F(frame.rows[1].y()), stunt_id: 0 };
    let mut clip = FlipClip(flip.clip);
    let aloft = !air.on_ground && !step.touching;
    let out = backflip(p, air, chassis, step, &inp, &mut clip, || true, || false);
    flip.clip = clip.0;
    if out.wreck.is_some() {
        flip.wrecked = true;
        flip.cut = aloft;
    }
    if out.stunt_ended {
        flip.came_round = true;
    }
}

/// MX vs. ATV's two gyro options, "Brake Gyro" and "Gas Gyro" in its options (the bike's `+0x1290` /
/// `+0x1294`, copied in from the player's settings when the bike is made). With brake gyro on, a brake
/// held in the air spins the bike nose-up by `BrakeGyroTorque`; with gas gyro on, the throttle spins
/// it nose-down by `GasGyroTorque`. The game ships with brake on and gas off, which is what
/// [`Gyro::default`] is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gyro {
    pub brake: bool,
    pub gas: bool,
}

impl Default for Gyro {
    fn default() -> Gyro {
        Gyro { brake: true, gas: false }
    }
}

/// The rider's stunt id at or past which MX vs. ATV's air control takes the stick unscaled: under it a
/// trick is held (the stick's moment is halved and less).
pub const FIRST_FLIP_STUNT: i32 = 41;
/// The rider's stunt id from which the bike is idle (`0x2e` in the air, `0xff` on the ground): only
/// then do the gyros and the lean force act. The five between are the backflip's and the 360's
/// stunts (the backflip's is 41): no scaling, and no gyro or lean force.
pub const IDLE_STUNT: i32 = 46;

/// What MX vs. ATV's air control reads beyond MX Unleashed's [`Step`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirIn {
    /// The rider's current stunt id (`+0x1524`; see [`FIRST_FLIP_STUNT`] and [`IDLE_STUNT`]).
    pub stunt_id: i32,
    /// The rate-limited throttle (`+0x318`): the gas gyro acts while it is not zero.
    pub gas: F,
    pub gyro: Gyro,
}

/// One substep of MX vs. ATV's airborne control (SLUS-21104's, after its air drag), over MX
/// Unleashed's state: the moment the stick commands, damped or auto-levelled, the two gyros, and the
/// lean force. `gas_gyro_torque` is the bike file's `[Airborne Control] GasGyroTorque`.
///
/// What it does differently from MX Unleashed's ([`crate::air::airborne_control`]):
///
/// - the stick is left out only while the backflip's and the 360's torques both went in; under a
///   backflip turning alone the stick still pitches (yaw and roll silenced, as MX Unleashed's);
/// - a held trick is the rider's stunt id under 41, not the trick clocks' flag, and the gyros and lean
///   force wait for an idle id (46 or more), so a backflip's stunt between has neither;
/// - "steep" is the literal cos 20° (0.9396926), not the sine table's entry MX Unleashed reads;
/// - the auto-level's sums are the FPU's `adda`/`madd` and `msub`, not a multiply then an add;
/// - the brake gyro only on [`Gyro::brake`]; otherwise, on [`Gyro::gas`] with the throttle open, the
///   gas gyro takes `GasGyroTorque` off the pitch.
pub fn airborne_control(p: &crate::air::Params, gas_gyro_torque: F, air: &mut Air, chassis: &mut Chassis, step: &Step,
                        inp: &AirIn) {
    use crate::air::{DEAD, NOTHING, SLOW, UP, cross, map_clamp, normalize};
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
    if !(air.flip_torque_applied && step.spinning) {
        m = if inp.stunt_id < FIRST_FLIP_STUNT {
            Vf::new(
                (step.stick.1 * (F(0.5) * F(acm.x()))).0,
                (step.stick.0 * (F(0.4) * F(acm.y()))).0,
                (step.stick.0 * (F(0.4) * F(acm.z()))).0,
                m.w(),
            )
        } else {
            Vf::new((F(acm.x()) * step.stick.1).0, (F(acm.y()) * step.stick.0).0, (F(acm.z()) * step.stick.0).0, m.w())
        };
        if air.flipping {
            m = Vf::new(m.x(), 0.0, 0.0, m.w());
        }
        m = m.merge(Dest::XYZ, m.mul_q(ramp.0));
    }
    let level = F(step.heading.y()).abs() < F::from_bits(0x3f70_8fb2)
        && step.lean.abs() < DEAD
        && !air.flipping
        && !step.spin_ramp
        && air.authority <= 0.0;
    if !level {
        let damped = chassis.ang_vel.merge(Dest::XYZ, chassis.ang_vel.mul_q(p.airborne_damping.0));
        m = m.merge(Dest::XYZ, m.sub(damped));
    } else {
        let w = chassis.ang_vel;
        let z = fpu::madd(fpu::adda(0.0, m.z()), p.unroll_torque.0, (p.unroll_gain * chassis.roll - F(w.z())).0);
        let y = fpu::madd(fpu::adda(0.0, m.y()), p.unyaw_torque.0, (p.unyaw_gain * air.crossup - F(w.y())).0);
        m = Vf::new(m.x(), y, z, m.w());
        if F(w.x()).abs() > DEAD {
            m = Vf::new(fpu::msub(fpu::adda(0.0, m.x()), p.airborne_damping.0, w.x()), m.y(), m.z(), m.w());
        }
        if F(w.y()).abs() > DEAD {
            m = Vf::new(m.x(), fpu::msub(fpu::adda(0.0, m.y()), p.airborne_damping.0, w.y()), m.z(), m.w());
        }
    }
    let idle = inp.stunt_id >= IDLE_STUNT;
    if idle && inp.gyro.brake && (step.front_brake != 0.0 || step.rear_brake != 0.0) {
        m = Vf::new((F(m.x()) + p.brake_gyro_torque).0, m.y(), m.z(), m.w());
    } else if idle && inp.gyro.gas && inp.gas != 0.0 {
        m = Vf::new((F(m.x()) - gas_gyro_torque).0, m.y(), m.z(), m.w());
    }
    chassis.moment = chassis.moment.merge(Dest::XYZ, chassis.moment.add(m));

    if idle && !air.flip_torque_applied && !step.spinning && chassis.speed > SLOW {
        let side = normalize(cross(UP, chassis.vel));
        let push = side.mul_q((p.airborne_control_force * step.lean).0);
        chassis.force = chassis.force.merge(Dest::XYZ, chassis.force.add(push));
    }
}

/// [`airborne_control`] on a ride. The port's rider keeps no stunt id, so one stands in for it by
/// range, which is all the step reads: a trick held ([`crate::air::Step::stunt`]) is a trick's id, a
/// backflip's clip running ([`Flip::clip`]) or the 360's (`spin_clip`) is the flip's stunt (41, the
/// backflip's in the image), and anything else is the idle air id `0x2e`.
pub fn ride_airborne_control(phy: &crate::phy::Phy, air: &mut Air, chassis: &mut Chassis, step: &Step, gas: F,
                             flip: &Flip, spin_clip: bool) {
    let stunt_id = if step.stunt {
        0
    } else if flip.clip != 0 || spin_clip {
        FIRST_FLIP_STUNT
    } else {
        0x2e
    };
    let inp = AirIn { stunt_id, gas, gyro: phy.gyro };
    airborne_control(&phy.air, phy.gas_gyro_torque, air, chassis, step, &inp);
}

/// MX vs. ATV's `[Mass Properties]` keys that MX Unleashed reads in `[Collisions]`, as
/// `(MX vs. ATV's name, MX Unleashed's)`: the collision springs renamed, the rest moved as they are.
const MOVED_TO_COLLISIONS: [(&str, &str); 17] = [
    ("SpringStiffness", "Spring"),
    ("SpringDamping", "Damping"),
    ("MaxPenetration", "MaxCompression"),
    ("BodySpringStiffness", "BodySpring"),
    ("BodySpringDamping", "BodyDamping"),
    ("BodyMaxPenetration", "BodyMaxCompression"),
    ("Friction", "Friction"),
    ("FrictionMomentScale", "FrictionMomentScale"),
    ("SpringMomentScale", "SpringMomentScale"),
    ("Restitution", "Restitution"),
    ("BodyFriction", "BodyFriction"),
    ("BodyFrictionMomentScale", "BodyFrictionMomentScale"),
    ("BodyRestitution", "BodyRestitution"),
    ("BodySpringMomentScale", "BodySpringMomentScale"),
    ("CheckForHighSpeedCollisions", "CheckForHighSpeedCollisions"),
    ("HighSpeedMinCollisionAngle", "HighSpeedMinCollisionAngle"),
    ("HighSpeedRayOffset", "HighSpeedRayOffset"),
];

/// The rest of the moves: `(MX vs. ATV's section, key, MX Unleashed's section, key)`.
const MOVED: [(&str, &str, &str, &str); 6] = [
    ("Mass Properties", "HighSpeedRaySpeedScale", "Collisions", "HighSpeedRaySpeedScale"),
    ("Mass Properties", "HighSpeedUnpenetrationDistance", "Collisions", "HighSpeedUnpenetrationDistance"),
    // Its sixteen friction zones are one value each on every bike it ships; the first stands for the
    // terrain friction MX Unleashed has one of.
    ("Mass Properties", "FrictionZone1", "Collisions", "TerrainFriction"),
    ("Mass Properties", "GravityScale", "Mass Properties", "Gravity"),
    ("Camera", "Left1stPerShoulderOffset", "FirstPersonCamera", "Left1stPerShoulderOffset"),
    ("Camera", "Right1stPerShoulderOffset", "FirstPersonCamera", "Right1stPerShoulderOffset"),
];

/// An MX vs. ATV bike file (`mx_*.phy`) with its keys where MX Unleashed's reader looks for them
/// ([`crate::phy::Phy::read`]): the collision keys it moved into `[Mass Properties]` back in
/// `[Collisions]` under their old names, `GravityScale` as `Gravity`, the first-person shoulders in
/// `[FirstPersonCamera]`. Every other key is kept where it is — its new ones (`GasGyroTorque`, which
/// [`crate::phy::Phy::gas_gyro_torque`] reads, `[Joystick]`'s input ramp, `ResetTime`, …) stay where
/// they are; only the first is read yet.
pub fn mxvatv_ini(ini: &crate::phy::Ini) -> crate::phy::Ini {
    let eq = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
    let mut sections: Vec<(String, String)> = Vec::new();
    for (sec, key, value) in ini.entries() {
        let to_collisions = eq(sec, "Mass Properties").then(|| MOVED_TO_COLLISIONS.iter().find(|(k, _)| eq(k, key))).flatten();
        let (sec, key) = match to_collisions {
            Some((_, old)) => ("Collisions", *old),
            None => match MOVED.iter().find(|(s, k, _, _)| eq(s, sec) && eq(k, key)) {
                Some((_, _, s, k)) => (*s, *k),
                None => (sec, key),
            },
        };
        let line = format!("{key} = {value}\n");
        match sections.iter_mut().find(|(s, _)| eq(s, sec)) {
            Some((_, text)) => text.push_str(&line),
            None => sections.push((sec.to_string(), line)),
        }
    }
    let text: String = sections.iter().map(|(s, body)| format!("[{s}]\n{body}")).collect();
    crate::phy::Ini::parse(&text)
}

/// An MX vs. ATV bike file read for the ride: [`mxvatv_ini`] through MX Unleashed's reader, the bike
/// set on [`Handling::MxVsAtv`].
pub fn mxvatv_phy(bytes: &[u8], curve: &crate::camera::CurveTable) -> crate::phy::Phy {
    let mut phy = crate::phy::Phy::read(&mxvatv_ini(&crate::phy::Ini::parse_bytes(bytes)), curve);
    phy.handling = Handling::MxVsAtv;
    phy
}
