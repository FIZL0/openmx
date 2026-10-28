//! S161 — the Backwards Landings Bail switch ([`game_sim::state::Sense::backwards_bail`]), the port's
//! own and off by default: on, a landing with the bike pointing back against the way it is going wreck
//! ([`air::backwards_landing`]), which the original rides out once it is past about 135 degrees,
//! because its crossup is an arcsine (S150).
//!
//! The clause itself is checked against a plain reading of its rule over generated touchdowns; the
//! switch against the stand-in bike dropped onto a flat floor going backwards, forwards and sideways;
//! and its tape line round trip. That S150's own captured landings bail with the switch on, and that
//! one the original already wrecks wrecks the same, is `tests/replay.rs`'
//! `backwards_landings_bail_when_the_switch_is_on`; that off is the original's is S150's
//! `crooked_landings_wreck_or_swing_round_as_the_original_does` and every other replay, which ride
//! with the switch at its default.

use std::sync::Arc;

use game_sim::air;
use game_sim::phy::Phy;
use game_sim::replay::{self, PadFrame, PadTape, Recorder};
use game_sim::ride::Controls;
use game_sim::rider::Tables;
use game_sim::trick::{Set, Wreck};
use game_sim::world::Flat;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// `tests/ride.rs`' `RIDE_LIMITS`, a shipped bike's `[Wrecks]` numbers, but for `CrossupWreck` at 1:
/// the original's own landing clause then never fires, so a wreck on the touch is the switch's.
const LIMITS: &str = "
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 60
BackWreckSpeedLimit = 60
SideWreckSpeedLimit = 40
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 40
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 120
AirbornePitchUpWreckAngle = 120
AirborneRollWreckAngle = 100
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// A small deterministic generator (xorshift), so the sweep is the same every run.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn within(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

/// **The clause is its rule.** Over 20,000 generated touchdowns — the landing's three conditions
/// each sometimes false, the forward axis and velocity anywhere — it wrecks exactly when the wheels
/// came down this substep after more than `CrossupWreckTime` aloft, the bike is not mid-backflip and
/// is faster than 10, and the flattened heading and velocity point more than a quarter turn apart;
/// and then to the side the heading is crossed to (Left when velocity × heading turns up, the sign
/// the crossup takes). Cases within a hair of a quarter turn, where the rounding decides, are left out
/// and counted.
#[test]
fn the_clause_wrecks_a_landing_more_than_a_quarter_turn_round() {
    let tables = Tables::build();
    let p = Phy::read_bytes(format!("{}{LIMITS}", stock::BIKE).as_bytes(), &tables.ride.curve).air;
    let limit = p.crossup_wreck_time.0;
    assert!((limit - 0.3).abs() < 1e-6, "{limit}");
    let mut pick = Pick(0x5161);
    let (mut wrecked, mut ridden, mut close) = (0, 0, 0);
    for n in 0..20_000 {
        let forward = Vf::new(pick.within(-1.0, 1.0), pick.within(-1.0, 1.0), pick.within(-1.0, 1.0), 0.0);
        let vel = Vf::new(pick.within(-60.0, 60.0), pick.within(-60.0, 60.0), pick.within(-60.0, 60.0), 0.0);
        let air = air::Air {
            was_airborne: pick.unit() < 0.8,
            airborne: pick.unit() < 0.2,
            hang_time: F(pick.within(0.0, 1.0)),
            flipping: pick.unit() < 0.1,
            ..air::Air::default()
        };
        let chassis = air::Chassis { speed: F(pick.within(0.0, 30.0)), vel, ..air::Chassis::default() };

        let (fx, fz) = (forward.x() as f64, forward.z() as f64);
        let (vx, vz) = (vel.x() as f64, vel.z() as f64);
        let (fl, vl) = ((fx * fx + fz * fz).sqrt(), (vx * vx + vz * vz).sqrt());
        if fl < 1e-3 || vl < 1e-3 {
            continue;
        }
        let along = (fx * vx + fz * vz) / (fl * vl);
        if along.abs() < 1e-4 {
            close += 1;
            continue;
        }
        let landing = air.was_airborne && !air.airborne && air.hang_time.0 > limit;
        let asked = landing && !air.flipping && chassis.speed.0 > 10.0 && along < 0.0;
        // heading × velocity's y, as the crossup's sign: positive is Left.
        let side = fz * vx - fx * vz;
        let want = asked.then(|| if side < 0.0 { air::Wreck::Right } else { air::Wreck::Left });
        let got = air::backwards_landing(&p, &air, &chassis, forward);
        assert_eq!(got, want, "case {n}: forward {forward:?} vel {vel:?} air {air:?} speed {:?}", chassis.speed);
        if got.is_some() {
            wrecked += 1;
        } else {
            ridden += 1;
        }
    }
    println!("{wrecked} wrecked, {ridden} ridden, {close} within a hair of a quarter turn left out");
    assert!(wrecked > 1000 && ridden > 1000, "the sweep reached both answers");
}

/// The stand-in bike with limits, wrecks on, and the switch as `bail`.
fn session(bail: bool) -> Sim {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(format!("{}{LIMITS}", stock::BIKE).as_bytes(), &tables.ride.curve);
    let mut sim = Sim::ride(0x161, tables, phy, stock::SPAWN, stock::FACING, Set::default());
    sim.rider.sense.wrecks_enabled = true;
    sim.rider.sense.backwards_bail = bail;
    sim
}

/// Upright, 12 ft up, moving at 30 along `way` (flat, unit): it lands about 0.9 s later.
fn dropped(sim: &mut Sim, way: Vf) {
    let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + 12.0, stock::SPAWN.z(), 1.0);
    sim.rider.ride.place_on(at, stock::FACING, Vf::new(0.0, 1.0, 0.0, 0.0));
    sim.rider.ride.chassis.vel = Vf::new(way.x() * 30.0, 0.0, way.z() * 30.0, 0.0);
}

/// Up to `most` ticks: the tick the wheels first came down, and the first wreck with its tick.
fn ride(sim: &mut Sim, most: usize) -> (Option<usize>, Option<(usize, Wreck)>) {
    let (mut down, mut wreck) = (None, None);
    for k in 0..most {
        let f = sim.step(DT, &Controls::default(), &mut Flat::default());
        if down.is_none() && sim.rider.ride.air.on_ground {
            down = Some(k);
        }
        if let Some(w) = f.wreck {
            wreck = Some((k, w));
            break;
        }
    }
    (down, wreck)
}

/// **The switch.** Dropped going straight back, the bike rides the landing out with the switch off,
/// the original's (the arcsine reads 180 degrees as 0), and wrecks on the tick it comes down with it
/// on. Going forwards, or sideways a little under a quarter turn, the switch changes nothing. A new
/// rider has it off.
#[test]
fn on_bails_a_backwards_landing_and_off_rides_it() {
    assert!(!session(false).rider.sense.backwards_bail);
    let rider = game_sim::rider::Rider::new(
        &Phy::read_bytes(stock::BIKE.as_bytes(), &Tables::build().ride.curve),
        Default::default(),
        stock::SPAWN,
        stock::FACING,
        Set::default(),
    );
    assert!(!rider.sense.backwards_bail, "a new rider rides the original's landing");

    let f = stock::FACING;
    let back = Vf::new(-f.x(), 0.0, -f.z(), 0.0);
    let (s, c) = (80f32.to_radians().sin(), 80f32.to_radians().cos());
    let aside = Vf::new(f.x() * c + f.z() * s, 0.0, f.z() * c - f.x() * s, 0.0);
    for (what, way, bails) in [("backwards", back, true), ("forwards", f, false), ("80 degrees aside", aside, false)] {
        let mut off = session(false);
        dropped(&mut off, way);
        let (down, wreck) = ride(&mut off, 60);
        println!("{what}, off: down at {down:?}, wreck {wreck:?}");
        assert!(down.is_some(), "{what}: the bike never came down");
        assert_eq!(wreck, None, "{what}: the original's test wrecked the landing");

        let mut on = session(true);
        dropped(&mut on, way);
        let (down_on, wreck_on) = ride(&mut on, 60);
        println!("{what}, on: down at {down_on:?}, wreck {wreck_on:?}");
        if bails {
            let (k, w) = wreck_on.unwrap_or_else(|| panic!("{what}: the switch on rode it out"));
            assert_eq!(Some(k), down_on, "{what}: the wreck is on the touch");
            assert!(w == Wreck(air::Wreck::Left as u8) || w == Wreck(air::Wreck::Right as u8), "{what}: {w:?}");
        } else {
            assert_eq!(wreck_on, None, "{what}: the switch wrecked a landing it should not judge");
        }
    }
}

/// The switch rides on a tape as Air Wrecks' does: a `backwards_bail FRAME on|off` line, written only
/// where it was set, read back, handed out by the head before its frame, and put on by `play_from`.
#[test]
fn the_switch_rides_on_a_tape() {
    let mut plain = Recorder::new("backwards bail");
    let mut set = Recorder::new("backwards bail");
    set.backwards_bail(false);
    set.backwards_bail(true);
    for _ in 0..4 {
        let f = PadFrame::of_buttons(DT, 0);
        plain.frame(f.dt(), &f.reports);
        set.frame(f.dt(), &f.reports);
    }
    set.backwards_bail(false);
    assert_eq!(set.tape.backwards_bail, vec![(0, true), (4, false)], "two settings before one frame are the last");
    assert!(!plain.to_text().contains("backwards_bail"), "a tape with the switch untouched says nothing of it");
    let text = set.to_text();
    assert!(text.contains("backwards_bail 0 on\n") && text.contains("backwards_bail 4 off\n"), "{text}");
    let back = PadTape::parse(&text).unwrap();
    assert_eq!(back, set.tape);
    assert_eq!(back.backwards_bail_at(0), Some(true));
    assert_eq!(back.backwards_bail_at(2), None);
    assert!(PadTape::parse("backwards_bail 0 maybe").is_err());
    assert!(PadTape::parse("backwards_bail 4 on\nbackwards_bail 2 off").is_err());

    let mut head = replay::Head::new(back.clone());
    assert_eq!(head.backwards_bail(), Some(true));
    head.next();
    assert_eq!(head.backwards_bail(), None);

    let run = replay::play_from(&PadTape { frames: back.frames[..1].to_vec(), ..back.clone() }, session(false));
    assert!(run.sim.rider.sense.backwards_bail, "the tape's switch was not put on");
    let run = replay::play_from(&PadTape { frames: back.frames[..1].to_vec(), backwards_bail: Vec::new(), ..back }, session(false));
    assert!(!run.sim.rider.sense.backwards_bail);
}
