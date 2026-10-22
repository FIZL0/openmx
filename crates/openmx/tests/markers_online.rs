//! P105 — each player's saved markers (P99's files) on the two online paths with no lobby pick to carry
//! them: `--peer` rides over plain UDP, where every machine's go to every peer in pieces before the
//! first tick (`net::udp`), and a peer dropping in on a ride under way, whose markers come with its
//! handshake and go on its seat in the state every machine is sent at the join (`rides::spawn_marked`).
//!
//! The ride is N2's test ride (`tests/netplay.rs`): [`Rides`], one [`Sim`] a seat over a flat floor,
//! every seat mashing a pad of the original's bytes — a lone Up among them, which rides a seat back
//! to its first marker, so the markers are in the ride and not only in its state.

#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;
#[allow(dead_code)]
#[path = "../src/saved_markers.rs"]
mod saved_markers;
#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use game_sim::input::{Control, PADS, Report};
use game_sim::markers::{self, Did, Markers, Spot};
use game_sim::world::Flat;
use game_sim::Sim;
use net::rides::{self, Rides};
use net::udp::Udp;
use net::{Lineup, Netplay, Timing};
use ps2_float::F;

const BIKE: &str = "race125p";
const TRACK: &str = "free06";

/// A round: one tick of the ride's 30 Hz clock.
const ROUND: std::time::Duration = std::time::Duration::from_nanos(1_001_000_000_000 / 30_000);

/// ggrs' clock is one for the process: one test drives it at a time.
static CLOCK: Mutex<()> = Mutex::new(());

fn dumps() -> PathBuf {
    std::env::temp_dir().join(format!("openmx-p105-{}", std::process::id()))
}

fn sims(seats: usize) -> Vec<Sim> {
    let first = Sim::new(1);
    (0..seats)
        .map(|s| {
            let mut sim = first.clone();
            sim.rng = 1 + s as u32;
            sim
        })
        .collect()
}

/// `tests/netplay.rs`'s pads — throttle, the stick swinging, a trick chord now and then — and Up
/// on the gas alone in another phase (a trick button down holds the markers' d-pad), which rides a
/// seat with a first marker back to it.
fn mash(seat: usize, frame: usize) -> Report {
    let phase = (frame / 20 + seat * 7) % 11;
    let mut buttons = Control::Cross.mask();
    if phase == 3 {
        buttons |= Control::Circle.mask() | Control::Up.mask();
    }
    if phase == 7 {
        buttons = Control::Square.mask();
    }
    if phase == 5 {
        buttons |= Control::Up.mask();
    }
    let mut r = Report::from_buttons(buttons);
    if buttons & Control::Cross.mask() != 0 {
        r.0[Control::Cross.binding().pressure as usize] = (40 + 21 * phase) as u8;
    }
    r.set_axis(2, ((frame * (seat + 3) * 5) % 256) as u8);
    r.set_axis(3, (0x80 + ((frame / 7 + seat) % 9) * 9) as u8);
    r
}

/// A player's saved markers: `n` slots placed along a ride of the stand-in bike, a second apart.
fn placed(n: usize, rng: u32) -> Markers {
    let mut sim = Sim::new(rng);
    let gas = Control::Cross.mask();
    let dt = F(game_sim::tick::DeltaTime::CAP_30);
    let mut tick = |sim: &mut Sim, buttons: u16| {
        let mut reports = [Report::missing(); PADS];
        reports[0] = Report::from_buttons(buttons);
        let spots = vec![Spot::of(&sim.rider)];
        let hands = sim.poll(dt, &reports).stick_alone();
        sim.step(dt, &hands, &mut Flat::default());
        markers::frame(sim, 0, &spots, &hands)
    };
    for k in 0..n {
        for _ in 0..30 {
            tick(&mut sim, gas);
        }
        if k > 0 {
            tick(&mut sim, gas | Control::Right.mask());
            tick(&mut sim, gas);
        }
        // Down fires once held for markers::HOLD_MS (P106): the press's tick and seven more at 30 Hz.
        for _ in 1..8 {
            assert_eq!(tick(&mut sim, gas | Control::Down.mask()), None);
        }
        assert_eq!(tick(&mut sim, gas | Control::Down.mask()), Some(Did::Placed(k)));
        tick(&mut sim, gas);
    }
    let mut m = sim.markers.clone();
    m.at = 0;
    m
}

/// Each seat's bike after each tick (the seat's own count of them), as (seat, the tick) → its hash and
/// whether it stands on one of the seat's marker slots — where an Up just rode it: a tick a rollback
/// runs again writes it again, so a settled tick's is the one the ride kept.
type Bikes = BTreeMap<(usize, u64), (u64, bool)>;

fn frame(np: &mut Netplay<rides::State>, sims: &mut Vec<Sim>, world: &mut Flat, bikes: &mut Bikes) {
    let local: Vec<Report> = np.lineup.mine(&np.me).into_iter().map(|s| mash(s, np.tick())).collect();
    let mut after = |seat: usize, _: F, sim: &mut Sim, _: &game_sim::ride::Controls| {
        let now = game_sim::hash::of(&sim.rider.ride);
        let on_slot = sim.markers.slots.iter().flatten().any(|slot| game_sim::hash::of(&**slot) == now);
        bikes.insert((seat, sim.ticks), (now, on_slot));
    };
    let mut game = Rides { sims, world, after: Some(&mut after), time_scale: None, plane: None };
    np.frame(&mut game, &local, ROUND.as_secs_f32());
}

/// The ticks before `to` at which seat `seat` stood on one of its slots.
fn landed(bikes: &Bikes, seat: usize, to: u64) -> usize {
    bikes.range((seat, 0)..(seat, to)).filter(|(_, h)| h.1).count()
}

/// What a `--peer` ride did: each machine's session, each machine's seats as its markers came in,
/// and the round each machine first ticked at.
struct PeerRide {
    nets: Vec<Netplay<rides::State>>,
    took: Vec<Vec<u64>>,
    first_tick: Vec<u32>,
    host_bikes: Bikes,
}

/// `host,g,k,k` on localhost's real UDP, each machine bringing `marks[i]` (`saved_markers::to_wire`'s
/// bytes) and doing what `Online::frame` does with them: offer its own until it ticks — k only from
/// round `late` on, a machine slow to load — and hear nothing and run nothing until every peer's
/// are in, then start every seat on them.
fn peer_ride(marks: [Vec<u8>; 3], late: u32, rounds: u32) -> PeerRide {
    let lineup = Lineup::parse("host,g,k,k").unwrap();
    let names = lineup.peers();
    let cards: [&[u8]; 3] = [&[0], &[0], &[0, 0]];
    let local: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut udps: Vec<Udp> = cards.iter().map(|c| Udp::bind_as(local, vec![], "p105", c).unwrap()).collect();
    let ats: Vec<_> = udps.iter().map(|u| u.at()).collect();
    for (i, u) in udps.iter_mut().enumerate() {
        u.meet((0..3).filter(|&j| j != i).map(|j| (names[j].clone(), ats[j])).collect());
        u.set_markers(marks[i].clone());
    }
    ggrs::clock::drive(true);
    let mut nets: Vec<Netplay<rides::State>> = names.iter().map(|me| Netplay::new(lineup.clone(), me, Timing::default(), dumps()).unwrap()).collect();
    let mut games: Vec<(Vec<Sim>, Flat)> = (0..3).map(|_| (sims(4), Flat::default())).collect();
    let mut took = vec![Vec::new(); 3];
    let mut first_tick = vec![u32::MAX; 3];
    let mut bikes = vec![Bikes::new(); 3];
    for round in 0..rounds {
        for i in 0..3 {
            let came = udps[i].receive().unwrap();
            if nets[i].tick() == 0 && round % 3 == 0 && (i != 2 || round >= late) {
                udps[i].offer();
            }
            if took[i].is_empty() {
                let Some(all) = udps[i].markers(&lineup, &names[i]) else { continue };
                for (sim, bytes) in games[i].0.iter_mut().zip(&all) {
                    sim.markers = saved_markers::from_wire(bytes, BIKE, TRACK);
                }
                took[i] = games[i].0.iter().map(|s| saved_markers::signature(&s.markers)).collect();
            }
            for (from, d) in came {
                nets[i].heard(&from, &d);
            }
            let (sims, world) = &mut games[i];
            frame(&mut nets[i], sims, world, &mut bikes[i]);
            if nets[i].tick() > 0 && first_tick[i] == u32::MAX {
                first_tick[i] = round;
            }
            for (to, d) in nets[i].outgoing() {
                udps[i].send(&to, &d);
            }
            nets[i].said.clear();
        }
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    PeerRide { nets, took, first_tick, host_bikes: bikes.swap_remove(0) }
}

/// The ticks every machine has every pad for, after checking they all had the same state before
/// each of them and ran the same pads.
fn agree(nets: &[&Netplay<rides::State>], from: usize) -> usize {
    let n = nets.iter().map(|np| (np.confirmed().max(0) as usize).min(np.hashes.len())).min().unwrap();
    for np in &nets[1..] {
        let at = (from..n).find(|&t| np.hashes[t] != nets[0].hashes[t]);
        assert_eq!(at, None, "{}'s state differs from {}'s before tick {at:?}", np.me, nets[0].me);
        let at = (from..n).find(|&t| np.log[t] != nets[0].log[t]);
        assert_eq!(at, None, "{} ran other pads than {} at tick {at:?}", np.me, nets[0].me);
    }
    assert!(nets.iter().all(|np| np.desyncs.is_empty() && np.end.is_none()));
    n.saturating_sub(from)
}

/// `--peer`s: the host brings two saved markers, g none and k five on its first seat (its second has
/// no pad and no file). No machine ticks before k's are in — k offers them a second late — and then
/// every machine starts every seat on the same slots, the host's and k's first seat's their own and
/// the rest none, and every machine rides the same ride from the first tick: the same state before
/// every settled tick and the same pads. The markers are in the ride, not only its state: Up rides the
/// host's bike and k's back onto one of their slots in the settled ticks (the hashes say every machine
/// rode it alike). The same pads with nobody's markers are no second ride to compare against here:
/// a machine samples its pads by the frame it is on, and when that is depends on the network.
#[test]
fn peer_udp_machines_start_every_seat_on_its_players_saved_markers() {
    let _clock = CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    let (host, k) = (placed(2, 7), placed(5, 9));
    let wire = |m: &Markers| saved_markers::to_wire(m, BIKE, TRACK);
    assert!(wire(&k).len() > 4 * net::udp::MARKS_PIECE, "k's are several pieces: {}", wire(&k).len());
    let late = 30;
    let r = peer_ride([wire(&host), Vec::new(), wire(&k)], late, 300);

    let want = vec![saved_markers::signature(&host), saved_markers::signature(&Markers::default()), saved_markers::signature(&k), saved_markers::signature(&Markers::default())];
    for (i, took) in r.took.iter().enumerate() {
        assert_eq!(*took, want, "machine {i}: every seat's markers as they started");
        assert!(r.first_tick[i] > late, "machine {i} ticked at round {} before k's markers were offered at {late}", r.first_tick[i]);
    }
    let nets: Vec<&Netplay<rides::State>> = r.nets.iter().collect();
    let settled = agree(&nets, 0);
    assert!(settled > 150, "{settled} ticks settled");

    // the ticks the host settled, in each seat's own count (every seat starts at the first tick)
    let n = settled as u64;
    for seat in [0, 2] {
        assert!(landed(&r.host_bikes, seat, n) > 0, "seat {} never rode back to its markers in {n} ticks", seat + 1);
    }
    for seat in [1, 3] {
        assert_eq!(landed(&r.host_bikes, seat, n), 0, "seat {} brought no markers", seat + 1);
    }
}

/// What a drop-in did: the two sessions, their seats at the end, and the tick g joined at.
fn dropped_in(markers: &Markers, rounds: u32) -> (Vec<Netplay<rides::State>>, Vec<Vec<Sim>>, usize, Bikes) {
    const JOIN: u32 = 120;
    ggrs::clock::drive(true);
    let spawn = rides::seat_bytes(&sims(1)[0]);
    let mut nets = vec![Netplay::new(Lineup::parse("host").unwrap(), "host", Timing::default(), dumps()).unwrap()];
    let mut games = vec![(sims(1), Flat::default())];
    let mut wire: Vec<(usize, u32, String, Vec<u8>)> = Vec::new();
    let mut bikes = vec![Bikes::new(), Bikes::new()];
    for round in 0..rounds {
        if round + 30 == JOIN {
            // what `Online` does as g arrives: its pick's markers (its handshake's) on its spawn
            let pick = saved_markers::to_pick(markers, BIKE, TRACK);
            let marked = rides::spawn_marked(&games[0].0[0], &spawn, saved_markers::from_pick(&pick, BIKE, TRACK)).unwrap();
            nets[0].admit("g", &[1], rides::spawn_on(&marked, &[(1, 0)])).unwrap();
        }
        if round == JOIN {
            nets.push(Netplay::joining("host", "g", Timing::default(), dumps()).unwrap());
            games.push((sims(1), Flat::default()));
        }
        for (to, at, from, d) in std::mem::take(&mut wire) {
            if at > round {
                wire.push((to, at, from, d));
            } else {
                nets[to].heard(&from, &d);
            }
        }
        for i in 0..nets.len() {
            let (sims, world) = &mut games[i];
            frame(&mut nets[i], sims, world, &mut bikes[i]);
            for (to, d) in nets[i].outgoing() {
                if let Some(j) = ["host", "g"].iter().position(|n| *n == to).filter(|&j| j < nets.len()) {
                    wire.push((j, round + 2, nets[i].me.clone(), d));
                }
            }
            nets[i].said.clear();
        }
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    let joined = nets[0].admit.joins[0].1 as usize;
    (nets, games.into_iter().map(|g| g.0).collect(), joined, bikes.swap_remove(0))
}

/// A drop-in: g's saved markers (three slots), as its handshake brought them to the host, go on its
/// seat in the state both machines take the join on. Both ride the same ride from the join's tick,
/// both still hold g's slots at the end, the host's seat holds none, and the markers are in the ride:
/// Up rides g's bike back onto one of its slots in the settled ticks, the hashes the same on both.
#[test]
fn a_drop_in_rides_in_on_its_saved_markers_on_every_machine() {
    let _clock = CLOCK.lock().unwrap_or_else(|e| e.into_inner());
    let g = placed(3, 11);
    let (nets, seats, joined, bikes) = dropped_in(&g, 600);
    let both: Vec<&Netplay<rides::State>> = nets.iter().collect();
    let settled = agree(&both, joined);
    assert!(settled > 300, "{settled} ticks settled from the join at {joined}");
    for (i, sims) in seats.iter().enumerate() {
        assert_eq!(sims.len(), 2, "machine {i}");
        assert_eq!(saved_markers::signature(&sims[1].markers), saved_markers::signature(&g), "machine {i}: g's seat holds its slots");
        assert!(sims[0].markers.slots.iter().all(Option::is_none), "machine {i}: the host's seat brought none");
    }

    // g's seat counts its ticks from the join
    let n = settled as u64;
    assert!(landed(&bikes, 1, n) > 0, "g's seat never rode back to its markers in {n} ticks");
    assert_eq!(landed(&bikes, 0, (joined + settled) as u64), 0, "the host's seat brought none");
}
