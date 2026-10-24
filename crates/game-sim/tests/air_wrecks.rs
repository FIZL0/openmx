//! P138 — the Air Wrecks switch ([`game_sim::state::Sense::air_wrecks`]): off, the bike's attitude in
//! the air never throws the rider off, and the landing is judged as it always was.
//!
//! The stand-in bike carries `tests/ride.rs`' `[Wrecks]` limits (it has none of its own, and with
//! every limit at zero the airborne clause answers on any jump), and is put upside down 40 ft over a
//! flat floor, moving forward and up: a long jump held upside down. Its landing on the roof is not
//! asserted: on the flat floor no landing clause judges a roof (the ground pitch limits read the
//! nose's pitch, which a roll over leaves level, and the vertical g load is the chassis' own up,
//! which a roof landing loads the other way), so that landing is ridden too, and the journal says
//! so. A landing nosed over past the ground's pitch limit is what proves the landing still judged. That the switch on is the
//! original's own test, bit for bit, is `tests/air.rs`' `the_wreck_test_is_the_originals`, which asks
//! both settings of it beside the original's answer over its generated states.

use std::sync::Arc;

use game_sim::air;
use game_sim::trick::Wreck;
use game_sim::phy::Phy;
use game_sim::replay::{self, PadFrame, PadTape, Recorder};
use game_sim::ride::Controls;
use game_sim::trick::Set;
use game_sim::world::Flat;
use game_sim::rider::Tables;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};

const NOSE_DIVE: Wreck = Wreck(air::Wreck::NoseDive as u8);
const TAIL: Wreck = Wreck(air::Wreck::Tail as u8);

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);

/// `tests/ride.rs`' `RIDE_LIMITS`: a shipped bike's `[Wrecks]` numbers.
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

/// The stand-in bike with limits, wrecks on, and the Air Wrecks switch as `air`.
fn session(air: bool) -> Sim {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(format!("{}{LIMITS}", stock::BIKE).as_bytes(), &tables.ride.curve);
    let mut sim = Sim::ride(0x138, tables, phy, stock::SPAWN, stock::FACING, Set::default());
    sim.rider.sense.wrecks_enabled = true;
    sim.rider.sense.air_wrecks = air;
    sim
}

/// Upside down (`up` straight down) 40 ft up, going forward at 30 and up at 10: the top of a jump.
fn thrown(sim: &mut Sim, up: f32) {
    let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + 40.0, stock::SPAWN.z(), 1.0);
    sim.rider.ride.place_on(at, stock::FACING, Vf::new(0.0, up, 0.0, 0.0));
    sim.rider.ride.chassis.vel = Vf::new(0.0, 10.0, 30.0, 0.0);
}

/// Each tick until the first wreck or `most` ticks: whether the bike was airborne after it, and
/// the wreck it entered, if any. `ups` gets the bike's up axis' height after each tick.
fn ride_up(sim: &mut Sim, most: usize, ups: &mut Vec<f32>) -> Vec<(bool, Option<Wreck>)> {
    let mut out = Vec::new();
    for _ in 0..most {
        let f = sim.step(DT, &Controls::default(), &mut Flat::default());
        out.push((sim.rider.ride.air.airborne, f.wreck));
        ups.push(sim.rider.ride.node[1].y());
        let wreck = f.wreck;
        if wreck.is_some() {
            break;
        }
    }
    out
}

fn ride(sim: &mut Sim, most: usize) -> Vec<(bool, Option<Wreck>)> {
    ride_up(sim, most, &mut Vec::new())
}

/// On (the original): upside down in the air is a wreck in the air, and the airborne clause's —
/// a nose dive or off the back — on the first tick. Upright, the same jump is not.
#[test]
fn upside_down_in_the_air_wrecks_with_the_switch_on() {
    let mut sim = session(true);
    thrown(&mut sim, -1.0);
    let seen = ride(&mut sim, 200);
    let &(airborne, wreck) = seen.last().unwrap();
    assert!(airborne, "the wreck came on the ground, after {} ticks", seen.len());
    assert!([NOSE_DIVE, TAIL].map(Some).contains(&wreck), "upside down in the air: {wreck:?} after {} ticks", seen.len());
    assert_eq!(seen.len(), 1, "upside down from the first tick, and not wrecked until tick {}", seen.len());

    let mut upright = session(true);
    thrown(&mut upright, 1.0);
    let seen = ride(&mut upright, 200);
    let airborne_wrecks = seen.iter().filter(|(a, w)| *a && w.is_some()).count();
    assert_eq!(airborne_wrecks, 0, "the same jump upright wrecked in the air: {:?}", seen.last());
    assert!(seen.iter().any(|(a, _)| !a), "the upright jump never came down");
}

/// Off: the same jump is ridden out — airborne for the whole of it, upside down from its first tick
/// to its last, never thrown off.
#[test]
fn off_rides_the_upside_down_jump_out() {
    let mut sim = session(false);
    thrown(&mut sim, -1.0);
    let mut ups = Vec::new();
    let seen = ride_up(&mut sim, 600, &mut ups);
    let in_air = seen.iter().take_while(|(a, _)| *a).count();
    assert!(in_air >= 30, "the bike was in the air only {in_air} ticks");
    assert!(seen[..in_air].iter().all(|(_, w)| w.is_none()), "thrown off in the air with the switch off: {seen:?}");
    assert!(ups[..in_air].iter().all(|&y| y < -0.5), "the bike turned back over in the air: up {:?}", &ups[..in_air]);
    assert!(in_air < seen.len(), "the bike never came down");
}

/// Off, the landing is judged as ever: nosed over past the ground's pitch limit (60 degrees) but
/// short of the air's (120), the bike rides the air with the switch either way, and the landing
/// throws the rider off the front on the same tick for both.
#[test]
fn off_still_wrecks_a_nosed_over_landing() {
    let landed = |air: bool| {
        let mut sim = session(air);
        let at = Vf::new(stock::SPAWN.x(), stock::SPAWN.y() + 40.0, stock::SPAWN.z(), 1.0);
        sim.rider.ride.place_on(at, Vf::new(0.0, -0.966, 0.259, 0.0), Vf::new(0.0, 0.259, 0.966, 0.0));
        sim.rider.ride.chassis.vel = Vf::new(0.0, 10.0, 30.0, 0.0);
        ride(&mut sim, 600)
    };
    let (on, off) = (landed(true), landed(false));
    assert_eq!(on, off, "the switch changed a jump that never passed the air's limits");
    let &(airborne, wreck) = off.last().unwrap();
    assert!(off.len() >= 30, "down after {} ticks", off.len());
    assert!(!airborne && wreck == Some(Wreck(air::Wreck::Front as u8)), "landed nose first: {wreck:?}, airborne {airborne}");
}

/// The switch rides on a tape (P138): a recorder writes it where it was set, the file round-trips,
/// a tape that never touched it is the bytes it always was, and [`replay::play`] rides it — the
/// switched-off tape rides past the tick where the switched-on one is down.
#[test]
fn the_switch_rides_on_a_tape() {
    let mut on = Recorder::new("air wrecks");
    let mut off = Recorder::new("air wrecks");
    off.air_wrecks(true);
    off.air_wrecks(false);
    for _ in 0..4 {
        let f = PadFrame::of_buttons(DT, 0);
        on.frame(f.dt(), &f.reports);
        off.frame(f.dt(), &f.reports);
    }
    off.air_wrecks(true);
    assert_eq!(off.tape.air_wrecks, vec![(0, false), (4, true)], "two settings before one frame are the last");
    assert!(!on.to_text().contains("air_wrecks"), "a tape with the switch untouched says nothing of it");
    let text = off.to_text();
    assert!(text.contains("air_wrecks 0 off\n") && text.contains("air_wrecks 4 on\n"), "{text}");
    let back = PadTape::parse(&text).unwrap();
    assert_eq!(back, off.tape);
    assert_eq!(back.air_wrecks_at(0), Some(false));
    assert_eq!(back.air_wrecks_at(2), None);
    assert!(PadTape::parse("air_wrecks 0 maybe").is_err());

    // Played: the head hands the switch out before the frame it was set before.
    let mut head = replay::Head::new(back.clone());
    assert_eq!(head.air_wrecks(), Some(false));
    head.next();
    assert_eq!(head.air_wrecks(), None);

    // And `play` takes it: one frame of the off tape, upside down in the air, rides on.
    let one = PadTape { frames: back.frames[..1].to_vec(), ..back.clone() };
    let mut sim = session(true);
    thrown(&mut sim, -1.0);
    let run = replay::play_from(&one, sim.clone());
    assert!(!run.sim.rider.sense.air_wrecks);
    assert_eq!(run.sim.rider.state.wreck, None, "the tape's Air Wrecks off was not ridden");
    let run = replay::play_from(&PadTape { air_wrecks: Vec::new(), ..one }, sim);
    assert!(run.sim.rider.state.wreck.is_some(), "the same frame with the switch on rode on");
}

