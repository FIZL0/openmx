//! The biplane as a ride ([`game_sim::flight`]): the plane is built off its file the way the original
//! builds it, and a state with a plane in it is a lane the harness can name without moving a bike's.
//!
//! The first proof reads the save state beside C7's recording (`biplane.p2m2_SaveState`, the plane
//! the human flies) and the disc's `Biplane\biplane.phy`; both are the user's own data, so it skips
//! with a line where either is missing.

use game_sim::archive::{Psr, Res};
use game_sim::flight::Flight;
use game_sim::hash::Dump;
use game_sim::phy::{self, Plane};
use game_sim::{Input, Sim};
use ps2_float::F;
use ps2_oracle::{Ee, Kit, skip};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

/// The save state beside C7's recording, and the plane in it (MXBiplane, the player's) — the same
/// object `tests/biplane.rs` proves the listeners on.
const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
const PLANE: u32 = 0x00c4_4a40;

/// The plane's parameter blocks, by offset (research/oracle.txt, R8's `biplane_read_phy`).
const JOYSTICK: u32 = 0x368; // twelve words to MinElevator; the ninth is not the controls'
const AUTOPILOT: u32 = 0x398; // six words
const ENGINE: u32 = 0x400; // the engine object's pointer; its keys from +0x3c, fifteen words
const AERO: u32 = 0x404; // the aero model's pointer; the true-speed flag at +0x30, then 37 words

/// The RAM image: this worktree's `context/` first, because a slot's kit.toml points `Kit::ctx` at
/// the main checkout's, which need not hold it.
fn image() -> Option<Ee> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE);
    let path = if here.exists() { here } else { kit.ctx("states").join(STATE) };
    match Ee::from_ram(&path) {
        Ok(ee) => Some(ee),
        Err(why) => {
            skip(&format!("{why} — `./kit state ram context/recordings/biplane.p2m2_SaveState.p2s` writes it"));
            None
        }
    }
}

/// `Biplane\biplane.phy` out of `machine.res`, the archive the plane's own loader reads it from.
fn plane_file() -> Option<Vec<u8>> {
    let kit = Kit::load().ok()?;
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/iso/MX.PSR");
    let path = if here.exists() { here } else { kit.ctx("iso").join("MX.PSR") };
    let Ok(mut f) = File::open(&path) else {
        skip(&format!("no {} — `./kit extract MX.PSR` writes it", path.display()));
        return None;
    };
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find("machine.res").expect("machine.res is in MX.PSR");
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut bytes = vec![0u8; e.size as usize];
    f.read_exact(&mut bytes).expect("read");
    let res = Res::parse(&bytes).expect("machine.res");
    let e = res
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(phy::BIPLANE_PHY))
        .expect("machine.res carries the plane's file");
    Some(res.read(&bytes, e).expect("its bytes"))
}

fn words(ee: &Ee, at: u32, n: usize) -> Vec<F> {
    (0..n).map(|k| F(ee.f32_at(at + 4 * k as u32))).collect()
}

fn same(what: &str, port: F, game: F) {
    assert_eq!(
        port.to_bits(),
        game.to_bits(),
        "{what}: the original built {:e}, the port {:e}",
        game.to_f32(),
        port.to_f32(),
    );
}

/// **[`Flight::new`] is the plane the original built**: every parameter the four listeners read —
/// the joystick's rates and limits, the autopilot's gains, the engine's keys and the constructor's
/// revolutions, the aero model's density, geometry and 31 coefficients, and its true-speed flag —
/// word for word against the live plane in the save state, built off the disc's own file.
#[test]
fn the_plane_is_built_off_its_file_as_the_original_builds_it() {
    let Some(ee) = image() else { return };
    let Some(file) = plane_file() else { return };
    let f = Flight::new(&Plane::read_bytes(&file));

    let j = words(&ee, PLANE + JOYSTICK, 12);
    let jp = &f.joystick;
    for (what, port, k) in [
        ("ThrottleIncrement", jp.throttle_increment, 0),
        ("ReverseThrottle", jp.reverse_throttle, 1),
        ("AileronControlRateLimit", jp.aileron_rate, 2),
        ("ElevatorControlRateLimit", jp.elevator_rate, 3),
        ("RudderControlRateLimit", jp.rudder_rate, 4),
        ("MaxElevator", jp.max_elevator, 5),
        ("MaxAileron", jp.max_aileron, 6),
        ("MaxRudder", jp.max_rudder, 7),
        ("ElevatorRampOffRollAngle", jp.ramp_off_roll, 9),
        ("ElevatorOffRollAngle", jp.off_roll, 10),
        ("MinElevator", jp.min_elevator, 11),
    ] {
        same(what, port, j[k]);
    }

    let a = words(&ee, PLANE + AUTOPILOT, 6);
    let ap = &f.autopilot;
    for (what, port, k) in [
        ("AileronDeadZone", ap.aileron_dead_zone, 0),
        ("MaxPitchRate", ap.max_pitch_rate, 1),
        ("MaxRollAngle", ap.max_roll, 2),
        ("PitchRateFeedForwardscale", ap.feed_forward, 3),
        ("PitchRateGain", ap.gain, 4),
        ("PitchRateIntegratorGain", ap.integrator_gain, 5),
    ] {
        same(what, port, a[k]);
    }

    // The engine's state words (the prop's angle, the power, rpm, thrust, note and clock) are the
    // moment's, not the build's; these are the ones nothing but the loader and the constructor write.
    let e = words(&ee, ee.u32_at(PLANE + ENGINE) + 0x3c, 15);
    let en = &f.engine;
    for (what, port, k) in [
        ("MinPropEfficiency", en.min_efficiency, 0),
        ("MinPropEfficiencySpeed", en.full_efficiency_speed, 1),
        ("the drawn prop's rpm", en.prop_rpm, 2),
        ("MaxPower", en.max_power, 4),
        ("the fade altitude", en.fade_altitude, 5),
        ("MaxAltitude", en.max_altitude, 6),
        ("the idle rpm", en.idle_rpm, 10),
        ("the top rpm", en.max_rpm, 11),
    ] {
        same(what, port, e[k]);
    }

    let aero = ee.u32_at(PLANE + AERO);
    assert_eq!(f.aero.true_speed, ee.u32_at(aero + 0x30) != 0, "the true-speed flag");
    let c = words(&ee, aero + 0x34, 37);
    let ar = &f.aero;
    for (what, port, k) in [
        ("the density", ar.density, 0),
        ("Chord", ar.chord, 1),
        ("Span", ar.span, 2),
        ("SurfaceArea", ar.area, 3),
        ("DefaultAngleOfAttack", ar.default_alpha, 5),
        ("CmBreakAngleOfAttack", ar.cm_break_alpha, 6),
        ("the lift slope", ar.lift_alpha, 7),
        ("CLRoundOff", ar.lift_round_off, 8),
        ("CLDropOff", ar.lift_drop_off, 9),
        ("CLScale", ar.lift_scale, 10),
        ("CLAngleScale", ar.lift_angle_scale, 11),
        ("CLq", ar.lift_q, 12),
        ("CLde", ar.lift_de, 13),
        ("CD0", ar.drag0, 14),
        ("CD1", ar.drag1, 15),
        ("CDde", ar.drag_de, 16),
        ("CYbeta", ar.side_beta, 17),
        ("CYda", ar.side_da, 18),
        ("CYdr", ar.side_dr, 19),
        ("CYp", ar.side_p, 20),
        ("CYr", ar.side_r, 21),
        ("Clbeta", ar.cl_beta, 22),
        ("Clp", ar.cl_p, 23),
        ("Clr", ar.cl_r, 24),
        ("Clda", ar.cl_da, 25),
        ("Cldr", ar.cl_dr, 26),
        ("CM0", ar.cm0, 27),
        ("CMalpha", ar.cm_alpha, 28),
        ("CMq", ar.cm_q, 29),
        ("CMde", ar.cm_de, 30),
        ("CMstall", ar.cm_stall, 31),
        ("Cnbeta", ar.cn_beta, 32),
        ("Cnp", ar.cn_p, 33),
        ("Cnr", ar.cn_r, 34),
        ("Cndr", ar.cn_dr, 35),
        ("Cnda", ar.cn_da, 36),
    ] {
        same(what, port, c[k]);
    }
    println!("the plane off {}: 61 parameter words and the true-speed flag, bit-exact", phy::BIPLANE_PHY);
}

/// **A plane ride is a lane, and a bike ride is untouched by it.** The same state with and without a
/// plane: without one the walk has no `plane.*` leaf at all and its hash is the bike ride's own; with
/// one the walk names the plane's fields, a tick steps the plane and not the bike, and the bike's two
/// lanes stay exactly where they were.
#[test]
fn a_plane_ride_is_a_lane_and_a_bike_ride_is_not_moved_by_it() {
    let bike = Sim::new(7);
    assert!(
        Dump::of(&bike).get("plane.crash").is_none(),
        "a bike ride's walk has grown a plane: every hash pinned on a bike ride would move",
    );

    let mut flying = bike.clone();
    let mut plane = Flight::default();
    plane.attitude.flown = true;
    plane.joystick.throttle_increment = F(0.35);
    plane.joystick.reverse_throttle = F(-0.2);
    plane.pad.forward = F(1.0);
    flying.plane = Some(plane);
    assert_ne!(flying.hash(), bike.hash(), "the plane is in the hash");
    for path in ["plane.body.pos[0]", "plane.surfaces.throttle", "plane.engine.prop_angle", "plane.crash"] {
        assert!(Dump::of(&flying).get(path).is_some(), "the walk has no '{path}'");
    }

    let (pos, vel) = (flying.bike_pos(), flying.bike_vel());
    let before = flying.plane.as_ref().map(|p| p.surfaces.throttle);
    flying.tick(Input::default());
    assert_eq!(flying.ticks, 1);
    assert_eq!(flying.bike_pos().to_bits(), pos.to_bits(), "the parked bike moved");
    assert_eq!(flying.bike_vel().to_bits(), vel.to_bits(), "the parked bike's velocity changed");
    // Hands off the pad (I6's mapping is not in yet), so the controls zero the pad and hold the
    // throttle: the plane was stepped, the frame ran its preforce half, and nothing else moved it.
    let after = flying.plane.as_ref().map(|p| p.surfaces.throttle);
    assert_eq!(after, before, "a hands-off frame moved the throttle");
    assert_eq!(flying.plane.as_ref().map(|p| p.pad.forward), Some(F::ZERO));
}
