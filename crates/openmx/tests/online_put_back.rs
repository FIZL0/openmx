//! P206 — an online seat's put-back and trick start are told.
//!
//! Online the session runs the riders' own frames and keeps none of what they said, so the shell's
//! `said.respawned` and `said.started` were never set: the biplane pilot's lean was not snapped on a
//! put-back, and the online `put back` and `trick:` lines never printed. [`audio::Said`] reads them
//! off the drawn ride instead, as it reads a wreck (P184). Proved here on the real session
//! ([`Netplay`] over [`Rides`]) between two machines in this thread on a link three rounds late, the
//! guest's stick swinging so the host guesses wrong and re-runs: both seats start in the disc's
//! biplane on flat ground, the host's taxis away from its bay on the throttle and puts it back with
//! L1+R1. The host's drawn frames, handed to a shell garage and [`PilotLean`] per seat as the online
//! tick does, say the put-back once, on the frame the plane is first drawn back in its bay, and the
//! lean snaps there. Skips without the disc.
//!
//! And off hand-set riders: a wreck going, the ghost clock starting over, and a trick coming into
//! progress are each told on the frame they are first drawn, and nothing on a frame that only
//! carries them on.

#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;
#[allow(dead_code)]
#[path = "../src/peer.rs"]
mod peer;
#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;
#[allow(unused)]
#[path = "../src/thread.rs"]
mod thread;
#[allow(dead_code)]
#[path = "../src/audio.rs"]
mod audio;

use game_sim::Sim;
use game_sim::flight::Flight;
use game_sim::input::{Control, Report};
use game_sim::trick::{Trick, Wreck};
use game_sim::world::Flat;
use net::rides::{self, Rides};
use net::{Lineup, Netplay, Timing};
use ps2_float::{F, Vf};
use vehicle::{Garage, Kind, PilotLean};

/// A round: one tick of the ride's 30 Hz clock (`pace::TICK`).
const ROUND: std::time::Duration = std::time::Duration::from_nanos(1_001_000_000_000 / 30_000);
const DT: F = F(1.0 / 30.0);
/// The rounds a datagram takes.
const LAG: u32 = 3;
/// The host holds the throttle until this round, and presses L1+R1 on it.
const CHORD: u32 = 100;

/// The disc's `biplane.phy`, or `None` and a note without a disc.
fn the_discs_plane() -> Option<game_sim::phy::Plane> {
    use game_render::disc::Disc;
    let mut disc = match Disc::open_configured() {
        Ok(disc) => disc,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.eq_ignore_ascii_case("machine.res"))?.name.clone();
    let res = disc.res(&name).expect("machine.res");
    let bytes = res.read_suffix(game_sim::phy::BIPLANE_PHY).expect("machine.res carries biplane.phy");
    Some(game_sim::phy::Plane::read_bytes(&bytes))
}

/// Seat `seat`'s bay: on the flat world's ground, the plane's wheels on it, a row along x.
fn bay(seat: usize, template: &Flight) -> vehicle::Spawn {
    let spawn = vehicle::Spawn { at: Vf::new(60.0 * seat as f32, 0.0, -30.0, 1.0), facing: Vf::new(0.0, 0.0, 1.0, 0.0) };
    let rest = vehicle::Plane::flying(spawn, template.clone()).rest_height(0.0);
    vehicle::Spawn { at: Vf::new(spawn.at.x(), rest, spawn.at.z(), 1.0), ..spawn }
}

/// Two seats, each standing in the plane in its bay.
fn planes(template: &Flight) -> Vec<Sim> {
    let first = Sim::new(1);
    (0..2)
        .map(|s| {
            let mut sim = first.clone();
            sim.rng = 1 + s as u32;
            vehicle::start_lane(template, bay(s, template), &mut sim);
            sim
        })
        .collect()
}

/// The host's pad: the throttle held, then L1 and R1 pressed together on [`CHORD`], then nothing.
fn host(tick: usize) -> Report {
    match tick as u32 {
        t if t < CHORD => Report::from_buttons(Control::Cross.mask()),
        CHORD => Report::from_buttons(Control::L1.mask() | Control::R1.mask()),
        _ => Report::idle(),
    }
}

/// The guest's pad: the stick swung side to side every few ticks, so the host's guess of it — the
/// last pad heard — is wrong on every swing, and the host re-runs the ticks since.
fn swing(tick: usize) -> Report {
    let mut r = Report::from_buttons(0);
    r.set_axis(2, if (tick / 4) % 2 == 0 { 0x10 } else { 0xf0 });
    r
}

/// **An online plane's put-back snaps its pilot's lean** (P206): said once, on the frame the plane
/// is first drawn back in its bay, and never for the seat that only flew its stick.
#[test]
fn a_headless_online_put_back_resets_the_lean() {
    let Some(file) = the_discs_plane() else { return };
    let template = Flight::new(&file);
    let lineup = Lineup::parse("host,g").unwrap();
    let names = lineup.peers();
    let dumps = std::env::temp_dir().join(format!("openmx-online-put-back-{}", std::process::id()));
    ggrs::clock::drive(true);
    let mut nets: Vec<Netplay<rides::State>> =
        names.iter().map(|me| Netplay::new(lineup.clone(), me, Timing::default(), dumps.clone()).unwrap()).collect();
    let mut sims: Vec<Vec<Sim>> = names.iter().map(|_| planes(&template)).collect();
    let mut worlds: Vec<Flat> = names.iter().map(|_| Flat::default()).collect();
    let mut flights: Vec<Flight> = names.iter().map(|_| template.clone()).collect();

    let mut said = audio::Said::default();
    let mut garages: Vec<Garage> = (0..2).map(|_| Garage::new(Kind::Bike, None).unwrap()).collect();
    let mut leans = [PilotLean::default(); 2];
    // the lean as it was before P206: never told of a put-back
    let mut deaf = [PilotLean::default(); 2];
    let mut put_back: Vec<(u32, usize)> = Vec::new();
    let mut snapped: Vec<(u32, usize)> = Vec::new();
    let mut deaf_snapped: Vec<(u32, usize)> = Vec::new();
    // the round each seat's plane is first drawn back in its bay, from away
    let mut back: Vec<Option<u32>> = vec![None; 2];
    let mut away = [0.0f32; 2];

    let mut wire: Vec<(usize, u32, String, Vec<u8>)> = Vec::new();
    for round in 0..180u32 {
        for (to, at, from, d) in std::mem::take(&mut wire) {
            if at > round {
                wire.push((to, at, from, d));
            } else {
                nets[to].heard(&from, &d);
            }
        }
        for i in 0..names.len() {
            let np = &mut nets[i];
            let local: Vec<Report> =
                np.lineup.mine(&np.me).into_iter().map(|_| if i == 0 { host(np.tick()) } else { swing(np.tick()) }).collect();
            let mut game = Rides { sims: &mut sims[i], world: &mut worlds[i], after: None, time_scale: None, plane: Some(&mut flights[i]) };
            np.frame(&mut game, &local, ROUND.as_secs_f32());
            for (to, d) in np.outgoing() {
                let j = names.iter().position(|n| *n == to).unwrap();
                wire.push((j, round + LAG, names[i].clone(), d));
            }
            np.said.clear();
        }
        // The host's drawn frame, as the shell's online tick reads it.
        for (s, sim) in sims[0].iter().enumerate() {
            let f = said.of(s, &sim.rider, sim.plane.as_ref());
            garages[s].follow(sim);
            let flight = sim.plane.as_ref().expect("every seat flies the plane");
            let from = flight.body.position.sub(flight.spawn.0);
            let off = (from.x() * from.x() + from.y() * from.y() + from.z() * from.z()).sqrt();
            if off < 1.0 && away[s] > 20.0 && back[s].is_none() {
                back[s] = Some(round);
            }
            away[s] = away[s].max(off);
            if f.respawned {
                put_back.push((round, s));
            }
            let was = (leans[s].reset.just, deaf[s].reset.just);
            leans[s].step(DT, garages[s].plane_flown(), f.respawned);
            deaf[s].step(DT, garages[s].plane_flown(), false);
            // a snap raised this frame, past the swap onto the plane's own
            if round > 2 && leans[s].reset.just && !was.0 {
                snapped.push((round, s));
            }
            if round > 2 && deaf[s].reset.just && !was.1 {
                deaf_snapped.push((round, s));
            }
        }
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    let _ = std::fs::remove_dir_all(&dumps);

    let h = &nets[0];
    println!("host: {} ticks, {} re-run; went {away:?} ft from the bays; back {back:?}; put back {put_back:?}", h.tick(), h.rb.reruns);
    assert!(h.rb.reruns > 0, "the host never re-ran a tick: nothing here was rolled back");
    assert!(h.desyncs.is_empty() && !h.desynced, "{:?}", h.desyncs);
    let at = back[0].unwrap_or_else(|| panic!("the host's plane never came back to its bay ({} ft out)", away[0]));
    assert!(back[1].is_none(), "g's plane only swung its stick, and was not put back");
    assert_eq!(put_back, [(at, 0)], "the put-back is said once, on the frame it is first drawn");
    assert_eq!(snapped, [(at, 0)], "the pilot's lean snaps onto the stick on it");
    assert!(deaf_snapped.is_empty(), "a lean not told of it never snaps: {deaf_snapped:?}");
}

/// Two drawn frames of one seat: what [`audio::Said::of`] tells of the second.
fn told(was: &Sim, now: &Sim) -> game_sim::rider::Frame {
    let mut said = audio::Said::default();
    let first = said.of(0, &was.rider, was.plane.as_ref());
    assert_eq!(first, game_sim::rider::Frame::default(), "a seat's first drawn frame says nothing");
    said.of(0, &now.rider, now.plane.as_ref())
}

/// **A bike's put-back and a trick's start, off hand-set riders** (P206).
#[test]
fn a_put_back_and_a_trick_start_are_read_off_the_drawn_ride() {
    let riding = Sim::new(1);

    // a wreck that goes is a put-back, and one that stays is nothing
    let mut down = riding.clone();
    down.rider.state.wreck = Some(Wreck(3));
    assert!(told(&down, &riding).respawned, "the wreck went: put back");
    assert_eq!(told(&down, &down), game_sim::rider::Frame::default());

    // the chord's put-back of a rider still riding starts the ghost over
    let mut ghost = riding.clone();
    ghost.rider.ghost.start();
    let mut later = ghost.clone();
    later.rider.ghost.clock = F(2.0);
    assert!(told(&riding, &ghost).respawned, "the ghost started: put back");
    assert!(told(&later, &ghost).respawned, "the ghost started over: put back");
    assert!(!told(&ghost, &later).respawned, "the ghost running down is no put-back");

    // a trick coming into progress is a start, once
    let mut tricking = riding.clone();
    tricking.rider.tricks.in_progress = true;
    tricking.rider.tricks.current = Trick::WHEELIE;
    assert_eq!(told(&riding, &tricking).started, Some(Trick::WHEELIE));
    assert_eq!(told(&tricking, &tricking).started, None, "a trick running on is not started again");
    let mut queued = tricking.clone();
    queued.rider.tricks.current = Trick::FRONTFLIP;
    assert_eq!(told(&tricking, &queued).started, None, "a queued trick following on is not a start, as offline");

    // a flip's points shown are told as its start, as the rider tells them
    let mut flip = riding.clone();
    flip.rider.score.shown = 7;
    assert_eq!(told(&riding, &flip).started, Some(Trick(7)));
    assert_eq!(told(&flip, &flip).started, None);
}
