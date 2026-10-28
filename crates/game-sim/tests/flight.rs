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
use game_sim::biplane::Pad;
use game_sim::world::Flat;
use ps2_float::{F, Vf};
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
    // The frame steps on the pad the last input step stored, the gas held, and only then stores this
    // one, hands off (S94: the original's order): the plane was stepped, its throttle opened by one
    // frame's increment, and the pad it holds now is the idle one.
    let after = flying.plane.as_ref().map(|p| p.surfaces.throttle);
    assert_eq!(after, before.map(|t| t + F(0.35) * F(game_sim::tick::DeltaTime::CAP_30)), "the stored press did not open the throttle");
    assert_eq!(flying.plane.as_ref().map(|p| p.pad.forward), Some(F::ZERO));
}

/// The plane's body (the plane's wrapper round it, `+0x160`) and the wrapper's gravity force.
const BODY: u32 = 0x160;
const GRAVITY: u32 = 0x50 + 0xf0;
/// The three tires' pointers.
const TIRES: [u32; 3] = [0x408, 0x40c, 0x410];

fn quad(ee: &Ee, at: u32) -> [u32; 4] {
    [0, 4, 8, 12].map(|k| ee.u32_at(at + k))
}

/// **The body and the wheels are the ones the original built**: the body's enable word, inverse
/// mass, the two dampings, the inverse inertia (carried, see `flight.rs`), the centre of mass and the
/// gravity force it is pushed with, and each tire's sixteen parameters and its brake and steer
/// hookups — word for word against the save state, the plane off the disc's own file.
#[test]
fn the_body_and_the_wheels_are_built_as_the_original_builds_them() {
    let Some(ee) = image() else { return };
    let Some(file) = plane_file() else { return };
    let f = Flight::new(&Plane::read_bytes(&file));
    let b = ee.u32_at(PLANE + BODY);
    assert_eq!(ee.u32_at(b) != 0, f.body.enabled, "enabled");
    same("inv_mass", f.body.inv_mass, F(ee.f32_at(b + 0x10)));
    for (what, port, at) in [
        ("damping", f.body.damping, b + 0x20),
        ("inv_inertia", f.body.inv_inertia, b + 0x120),
        ("ang_damping", f.body.ang_damping, b + 0x130),
        ("cofg", f.body.cofg, b + 0x180),
        ("gravity", f.gravity, PLANE + GRAVITY),
    ] {
        assert_eq!(port.to_bits(), quad(&ee, at), "{what}: the port {port:?}");
    }
    for (k, (t, port)) in TIRES.iter().zip(&f.tires).enumerate() {
        let t = ee.u32_at(PLANE + t);
        for (what, v, o) in [
            ("StrutCompression", port.max_compression, 0x3c),
            ("MaxDecompressionRate", port.max_decompression_rate, 0x40),
            ("spring", port.spring, 0x44),
            ("bounce", port.bounce, 0x48),
            ("rebound", port.rebound, 0x4c),
            ("radius", port.radius, 0x150),
            ("inv_radius", port.inv_radius, 0x154),
            ("max_braking", port.max_braking, 0x158),
            ("side_force", port.side_force, 0x15c),
            ("brake_friction", port.brake_friction, 0x160),
            ("steering_moment", port.steering_moment, 0x164),
        ] {
            same(&format!("tire {k}: {what}"), v, F(ee.f32_at(t + o)));
        }
        assert_eq!(&port.center.to_bits()[..3], &quad(&ee, t + 0x60)[..3], "tire {k}: center");
        assert_eq!((ee.u32_at(t + 0x30) != 0, ee.u32_at(t + 0x34) != 0), (port.braked, port.steered), "tire {k}: hookups");
    }
}

/// **The plane takes gas** (P31): stood on level ground off the disc's own file, it sits on its three
/// wheels with the throttle closed; with the throttle held open it gathers airspeed and leaves the
/// ground, climbing; closed again, it comes down.
///
/// Over a [`Flat`] floor: the scene's own ground is S45/S46/S48's to search. The numbers asked are
/// what any plane that flies does, not the original's: nothing of the original's was flown on flat
/// ground to compare with.
#[test]
fn the_plane_takes_gas_off_flat_ground() {
    let Some(file) = plane_file() else { return };
    let tables = game_sim::ride::Tables::build();
    let mut f = Flight::new(&Plane::read_bytes(&file));
    f.attitude.flown = true;
    let lowest = f.tires.iter().map(|t| F(t.center.y()) - t.radius).fold(F(f32::MAX), |a, b| a.min(b));
    f.place(Vf::new(0.0, (-lowest).to_f32(), 0.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0), &tables.acos);
    let mut ground = Flat::default();
    let dt = F(1.0 / 30.0);
    let mut fly = |f: &mut Flight, pad: Pad, seconds: u32| {
        for _ in 0..seconds * 30 {
            f.frame(dt, pad, &mut ground, &tables);
        }
        let (_, at) = f.pose();
        println!(
            "after {seconds} s: origin {:?} airspeed {} throttle {} grounded {} crash {}",
            at, f.measure.airspeed.to_f32(), f.surfaces.throttle.to_f32(), f.measure.grounded, f.crash.reason,
        );
        at
    };
    let rest = fly(&mut f, Pad::default(), 3);
    assert!(f.measure.grounded, "a plane at rest is not on its wheels");
    assert!(f.measure.airspeed.to_f32() < 1.0, "a plane with the throttle shut is rolling");
    let gas = Pad { forward: F::ONE, ..Pad::default() };
    let mut top = rest.y();
    let mut left = None;
    for s in 1..=40 {
        let at = fly(&mut f, gas, 1);
        if left.is_none() && !f.measure.grounded && at.y() > rest.y() + 10.0 {
            left = Some(s);
        }
        top = top.max(at.y());
    }
    let left = left.expect("forty seconds of full throttle and the plane never left the ground");
    println!("off the ground after {left} s, as high as {top}");
    assert!(top > rest.y() + 100.0, "the plane left the ground but did not climb");
    let shut = Pad { back: F::ONE, ..Pad::default() };
    let high = fly(&mut f, shut, 5).y();
    // Shut, not zero: held back, the control law puts a negative throttle back to zero and closes it
    // one step again, so it alternates between zero and one step under.
    assert!(f.surfaces.throttle <= F::ZERO, "five seconds of closing did not shut the throttle");
    let low = fly(&mut f, Pad::default(), 10).y();
    assert!(low < high - 50.0, "the throttle is shut and the plane is still not coming down");
}

/// **The reset chord on the flown plane** (S57): [`Flight::reset_chord`] is the original's L1+R1
/// (`biplane::Plane::reset_chord`, S36's model, which `tests/biplane.rs` holds to the recording's
/// chord exit). In the air it does nothing; on the wheels or wrecked it is the unwreck, and the plane
/// is back at its spawn, at rest, the throttle shut, the wreck cleared and its spin, damping, inertia
/// and gravity given back — on the same call, which is the press's own frame.
///
/// The wreck is the map edge's throw (reason 13) over a [`Flat`] floor: the crash into the ground
/// (reason 1) needs the world's contact, which is S48's. Bit for bit, the chord's plane is the plane
/// a fresh [`Flight::reset`] at the same spawn gives — the same unwreck — in every field the frame
/// reads.
#[test]
fn the_reset_chord_puts_the_flown_plane_back() {
    let Some(file) = plane_file() else { return };
    let tables = game_sim::ride::Tables::build();
    let acos = &tables.acos;
    let mut f = Flight::new(&Plane::read_bytes(&file));
    f.attitude.flown = true;
    let lowest = f.tires.iter().map(|t| F(t.center.y()) - t.radius).fold(F(f32::MAX), |a, b| a.min(b));
    let spawn = (Vf::new(2048.0, (-lowest).to_f32(), 2048.0, 1.0), Vf::new(0.0, 0.0, 1.0, 0.0));
    f.area = Some(game_sim::biplane::Area::new(F(16.0), F(16.0), F(1.0), F(1.0)));
    f.place(spawn.0, spawn.1, acos);
    // The spawn transform it keeps: the body's spot, the original's (S164).
    let put = f.spawn;
    let built = (f.gravity, f.body.damping, f.body.ang_damping, f.body.inv_inertia);
    let mut ground = Flat::default();
    let dt = F(1.0 / 30.0);
    for _ in 0..30 {
        f.frame(dt, Pad::default(), &mut ground, &tables);
    }
    assert!(f.measure.grounded, "the parked plane is not on its wheels");

    // On the wheels: back, at rest.
    let gas = Pad { forward: F::ONE, ..Pad::default() };
    for _ in 0..60 {
        f.frame(dt, gas, &mut ground, &tables);
    }
    assert!(f.measure.grounded && f.surfaces.throttle > F::ZERO);
    assert!(!f.reset_chord(false, acos), "no chord, no reset");
    assert!(f.reset_chord(true, acos), "the chord on the wheels did nothing");
    let (_, at) = f.pose();
    assert_eq!((at.x(), at.z()), (spawn.0.x(), spawn.0.z()));
    assert_eq!(f.surfaces.throttle, F::ZERO);
    assert_eq!((f.body.velocity.x(), f.body.velocity.y(), f.body.velocity.z()), (0.0, 0.0, 0.0));

    // In the air: nothing. Held 2000 ft up and let fall for a frame, it is not on its wheels.
    let high = Vf::new(spawn.0.x(), spawn.0.y() + 2000.0, spawn.0.z(), 1.0);
    f.place(high, spawn.1, acos);
    f.frame(dt, Pad::default(), &mut ground, &tables);
    assert!(!f.measure.grounded && f.crash.reason == 0);
    let before = f.body.position;
    assert!(!f.reset_chord(true, acos), "the chord in the air put the plane back");
    assert_eq!(f.body.position, before);

    // Wrecked: thrown back off the map's west edge, then the chord, still in the air.
    f.place(Vf::new(50.0, spawn.0.y() + 2000.0, 2048.0, 1.0), spawn.1, acos);
    f.spawn = put;
    f.frame(dt, Pad::default(), &mut ground, &tables);
    assert_eq!(f.crash.reason, 13, "the map's edge did not throw the plane");
    assert_ne!(f.gravity, built.0, "a wreck's gravity");
    f.frame(dt, Pad::default(), &mut ground, &tables);
    assert!(!f.measure.grounded);
    let mut fresh = f.clone();
    assert!(f.reset_chord(true, acos), "the chord in a wreck did nothing");
    fresh.reset(spawn.0, spawn.1, acos);
    assert_eq!(f.crash.reason, 0);
    assert_eq!((f.gravity, f.body.damping, f.body.ang_damping, f.body.inv_inertia), built, "the wreck given back");
    assert!(f == fresh, "the chord's plane is not the reset's");
    let (_, at) = f.pose();
    assert_eq!((at.x(), at.z()), (spawn.0.x(), spawn.0.z()));
}
