//! **The freestyle event in the shell** (P155): free09's own stunt zone and target, read off the disc
//! the way the shell reads them ([`portal::stunt_event`]), put on a ride; the stock bike stopped in
//! the zone is offered the event, a second's Circle starts it, the countdown refuses combos, the
//! clock runs out 120 s on with the last-seconds warning up, and the HUD shows the countdown, the
//! clock, the target and the verdict. Skips without the disc.
//!
//! The ground is a flat at the zone's height, not free09's terrain: the zone's trigger reads only
//! where the rider is.

#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/chase.rs"]
#[allow(dead_code)]
mod chase;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;
#[path = "../src/overlay.rs"]
#[allow(dead_code)]
mod overlay;
#[path = "../src/ride_hud.rs"]
#[allow(dead_code)]
mod ride_hud;
#[path = "../src/hud.rs"]
#[allow(dead_code)]
mod hud;
#[path = "../src/portal.rs"]
#[allow(dead_code)]
mod portal;

use game_render::Disc;
use game_sim::collide::{Bundle, Record};
use game_sim::event::{self, Verdict};
use game_sim::input::Control;
use game_sim::score::Sense;
use game_sim::world::World;
use game_sim::{Input, Sim, phy::Phy, rider::Tables, stock};
use menu::Speedometer;
use overlay::{Activity, Gauges};
use ps2_float::Vf;
use ride_hud::{EventShown, Shown, Tracker};
use std::sync::Arc;
use std::time::Instant;

/// Flat ground at `0`'s height, nothing solid on it.
struct Flat(f32);

impl World for Flat {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        (Vf::new(at.x(), self.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// free09's `hit.dat`, or `None` without the disc.
fn hit_dat() -> Option<String> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped, no disc: {e}");
            return None;
        }
    };
    let res = disc.res("Tracks\\free09_2.res").expect("free09's archive");
    let text = res.read_suffix("free09\\hit.dat").expect("free09's hit.dat");
    Some(String::from_utf8_lossy(&text).into_owned())
}

/// The stock bike's wreck limits, `tests/menu.rs`'s: the bare stock bike never gets going without them.
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

/// The stock bike stood at `at`, facing the stock way, with the track's event.
fn ride(at: [f32; 3], event: &event::Event) -> Sim {
    let tables = Arc::new(Tables::build());
    let phy = Phy::read_bytes(format!("{}{LIMITS}", stock::BIKE).as_bytes(), &tables.ride.curve);
    let mut sim = Sim::ride(0x155, tables, phy, Vf::new(at[0], at[1], at[2], 1.0), stock::FACING, Default::default());
    sim.event = event.clone();
    sim
}

fn circle() -> Input {
    Input { buttons: Control::Circle.mask(), ..Input::default() }
}

/// Whether a trick starting in the air now would open a combo on the rider's card.
fn opens(sim: &Sim) -> bool {
    let mut card = sim.rider.score.clone();
    card.combo = false;
    card.start(0, &Sense { airborne: true, ..Sense::default() });
    card.combo
}

/// The HUD's event lines for the ride as it stands.
fn lines(sim: &Sim, tracker: &mut Tracker, now: Instant) -> Vec<String> {
    let gauges = Gauges { feet: 0.0, bike: true, preload: None, score: 0, modified: false, speedometer: Speedometer::Digital };
    let e = EventShown::of(&sim.event, 0, &sim.rider.score);
    tracker.note_event(now, e.as_ref());
    let shown = Shown {
        speed: 0.0,
        gauges: &gauges,
        own: [0.0, 0.0],
        forward: [0.0, 1.0],
        riders: &[],
        blips: &[],
        span: 384.0,
        tag: None,
        offer: None,
        challenge: None,
        markers: None,
        scale: 1.0,
        minimap: false,
        map: None,
        around: [None; 4],
        join: false,
        event: e,
    };
    let mut out = Vec::new();
    ride_hud::sprites(&mut out, 0, [0.0, 0.0, 1920.0, 1080.0], &shown, tracker, now).event.into_iter().map(|l| l.text).collect()
}

#[test]
fn a_ride_into_free09s_stunt_zone_runs_the_event_to_its_verdict() {
    let Some(text) = hit_dat() else { return };
    let spots = portal::spots(&text);
    let zone = spots.iter().find(|s| s.activity == Activity::Stunt).expect("free09 has a stunt zone");
    let ev = portal::stunt_event(&spots, &text);
    assert_eq!(ev.target, 100_000, "free09's target");
    let world = &mut Flat(zone.at[1]);

    // Well outside the zone a held Circle starts nothing.
    let far = [zone.at[0] + zone.size[0] + zone.size[1] + 50.0, zone.at[1], zone.at[2]];
    let mut sim = ride(far, &ev);
    (0..60).for_each(|_| {
        sim.tick_in(circle(), world);
    });
    assert!(sim.event.on.is_none() && sim.event.fresh(), "no event outside the zone");

    // In it, stopped: offered, and a second's hold starts it.
    let mut sim = ride(zone.at, &ev);
    sim.rider.score.total = ps2_float::F(2500.0);
    sim.tick_in(Input::default(), world);
    assert!(sim.event.asks(&sim.event_rider(0)), "the zone offers the event to a stopped bike");
    let mut ticks = 0;
    while sim.event.on.is_none() {
        sim.tick_in(circle(), world);
        ticks += 1;
        assert!(ticks < 100, "the hold never started the event");
    }
    assert_eq!(ticks, 31, "a second's hold at 30 Hz");

    let mut tracker = Tracker::default();
    let mut now = Instant::now();
    let c = sim.event.on.as_ref().unwrap();
    assert!(c.counting && !c.running);
    assert_eq!(c.seats[0].saved, 2500, "the freeride total put aside");
    assert!(!opens(&sim), "the countdown refuses combos");
    let l = lines(&sim, &mut tracker, now);
    assert!(l.contains(&"TARGET 100,000".to_string()) && l.contains(&"3".to_string()), "countdown HUD: {l:?}");

    // The countdown, then the clock from 2:00.
    let mut ticks = 1;
    while !sim.event.on.as_ref().unwrap().running {
        sim.tick_in(Input::default(), world);
        ticks += 1;
        assert!(ticks < 200);
    }
    assert_eq!(ticks, 101, "3.33 s of countdown");
    assert!(opens(&sim), "combos open on the clock");
    let l = lines(&sim, &mut tracker, now);
    assert!(l.contains(&"2:00".to_string()) && l.contains(&"GO!".to_string()), "clock HUD: {l:?}");

    // Run out with nothing scored: the warning in the last seconds, out at 120 s, a loss.
    let mut warned = false;
    while sim.event.verdicts.first().is_none_or(Option::is_none) {
        sim.tick_in(Input::default(), world);
        ticks += 1;
        warned |= sim.event.on.as_ref().unwrap().seats[0].warning;
        assert!(ticks < 4000, "the clock never ran out");
    }
    assert!(warned, "the last-seconds warning");
    // 3600 of the 30 Hz clamp's 0.03333 s leave 0.012 s on the clock: the 3601st runs it out.
    assert_eq!(ticks, 101 + 3601, "out 120 s after the clock started");
    assert_eq!(sim.event.verdicts[0], Some((Verdict::Lose, 2500)), "nothing scored: the profile gets the saved total back");
    assert_eq!(sim.rider.score.total, ps2_float::F(2500.0), "the freeride total put back");
    assert!(!opens(&sim), "no combo out of time");
    now += std::time::Duration::from_secs(1);
    let l = lines(&sim, &mut tracker, now);
    assert!(l.contains(&"YOU LOSE".to_string()) && l.contains(&"EVENT 0".to_string()) && l.contains(&"FREERIDE TOTAL 2,500".to_string()), "verdict HUD: {l:?}");

    // The results come down and the ride is free again.
    while sim.event.on.is_some() {
        sim.tick_in(Input::default(), world);
        ticks += 1;
        assert!(ticks < 4100, "the results never came down");
    }
    assert!(opens(&sim));
    assert!(lines(&sim, &mut tracker, now).is_empty());
}
