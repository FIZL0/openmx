//! N2 — online play, proved headless the way opentennis's netplay tests prove it
//! (`crates/hst-play/src/play/netplay.rs`'s `lossy`, `on_a_bad_network`, `play_rollback`).
//!
//! Every machine is a [`Netplay`] session in this one thread, and a round of the harness is one tick
//! of the ride's 30 Hz clock ([`ROUND`], `pace::TICK`; N6): each machine's frame runs, and what it
//! sends goes on a simulated link that loses a share of the datagrams, delivers the rest some rounds
//! late, and in an outage delivers nothing. ggrs' clock is driven a [`ROUND`] a round (opentennis's
//! B494 patch, `third_party/ggrs/src/clock.rs`), so its pings,
//! resends and time sync — and the session's own — see the same time however long a round really
//! takes, and every run of a test is the same run.
//!
//! **The ride is the real one**: [`Rides`], one [`Sim`] a seat, which is what the shell steps online,
//! on [`game_sim::stock`]'s bike over a flat floor (no disc), each seat mashing its own pad of the
//! original's bytes — throttle pressure, steering, trick buttons. The resync and the restart need
//! the ride's state in bytes, every seat's [`Sim`] in N1's codec (N5): both are proved on the ride
//! itself, and on [`Tally`], a small game of the same pads, which is quick enough to try them over
//! more seats and a worse link.
//!
//! What "in sync" means here: every tick that every machine has every seat's pads for, every machine
//! ran on the same pads (the logs) and had the same state before it (the per-tick hashes — every
//! tick, not only the thirtieth ggrs compares).

#[allow(dead_code)]
#[path = "../src/net/mod.rs"]
mod net;
#[allow(dead_code)]
#[path = "../src/peer.rs"]
mod peer;
#[allow(dead_code)]
#[path = "../src/vehicle.rs"]
mod vehicle;

use std::ops::Range;
use std::path::PathBuf;

use game_sim::input::{Control, Report};
use game_sim::world::Flat;
use game_sim::Sim;
use net::link::DATAGRAM_MAX;
use net::rides::{self, Rides};
use net::{Game, Lineup, Netplay, Timing, WINDOW, unwire, wire};
use ps2_float::{F, Vf};

/// The tick a glitch makes one machine's state differ (the next checksum after it is 330).
const GLITCH: u64 = 320;

/// A round of the harness: one tick of the ride's clock, the PS2's 30 fps frame (`pace::TICK`).
const ROUND: std::time::Duration = std::time::Duration::from_nanos(1_001_000_000_000 / 30_000);

// ---------------------------------------------------------------------------------------------------
// The machines
// ---------------------------------------------------------------------------------------------------

/// One machine's game, as the harness drives it.
trait Machine {
    type S: Clone + 'static;
    fn frame(&mut self, np: &mut Netplay<Self::S>, local: &[Report]);
    fn hash(&self) -> u64;
}

/// The ride: a [`Sim`] a seat, as the shell has it; `glitch` knocks seat 0's bike sideways at
/// [`GLITCH`] on this machine only.
struct Ride {
    sims: Vec<Sim>,
    world: Flat,
    glitch: bool,
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

impl Ride {
    fn new(seats: usize) -> Ride {
        Ride { sims: sims(seats), world: Flat::default(), glitch: false }
    }
}

impl Machine for Ride {
    type S = rides::State;
    fn frame(&mut self, np: &mut Netplay<rides::State>, local: &[Report]) {
        let glitch = self.glitch;
        let mut after = |seat: usize, _: F, sim: &mut Sim, _: &game_sim::ride::Controls| {
            if glitch && seat == 0 && sim.ticks == GLITCH {
                sim.rider.ride.chassis.vel = Vf::new(0.5, 0.0, 0.0, 0.0);
            }
        };
        let mut game = Rides { sims: &mut self.sims, world: &mut self.world, after: Some(&mut after), time_scale: None, plane: None };
        np.frame(&mut game, local, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        let mut sims = self.sims.clone();
        let mut world = Flat::default();
        Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None }.hash()
    }
}

/// A small game of the same pads that **can** say its state in bytes: per seat, the sticks summed,
/// the buttons counted and every pad byte folded into a running mix, so any pad that differs on any
/// tick shows in the hash from then on. `glitch` flips a bit of seat 0's mix at [`GLITCH`].
#[derive(Clone, Default, PartialEq, Debug)]
struct Tally {
    tick: u64,
    seats: Vec<[i64; 3]>,
    glitch: bool,
}

impl Tally {
    fn new(seats: usize) -> Tally {
        Tally { tick: 0, seats: vec![[0; 3]; seats], glitch: false }
    }
}

impl Game for Tally {
    type State = Tally;
    fn save(&self) -> Tally {
        self.clone()
    }
    fn restore(&mut self, s: &Tally) {
        let glitch = self.glitch;
        *self = s.clone();
        self.glitch = glitch;
    }
    fn step(&mut self, _: usize, pads: &[Report]) {
        for (s, t) in self.seats.iter_mut().enumerate() {
            let p = pads[s];
            t[0] += p.axis(2) as i64 - 0x80;
            t[1] += p.buttons().count_ones() as i64;
            t[2] = t[2].rotate_left(7) ^ game_sim::state_hash(&p.0, 1) as i64;
            if self.glitch && s == 0 && self.tick == GLITCH {
                t[2] ^= 1;
            }
        }
        self.tick += 1;
    }
    fn hash(&self) -> u64 {
        let mut h = game_sim::state_hash(&self.tick.to_le_bytes(), 0);
        for t in &self.seats {
            for v in t {
                h = game_sim::state_hash(&v.to_le_bytes(), h);
            }
        }
        h
    }
    fn encode(&self) -> Option<Vec<u8>> {
        let mut b = self.tick.to_le_bytes().to_vec();
        for t in &self.seats {
            for v in t {
                b.extend(v.to_le_bytes());
            }
        }
        // padding, so a state is several pieces on the wire as the ride's will be
        b.extend(std::iter::repeat_n(0xa5, 3000));
        Some(b)
    }
    fn decode(&mut self, b: &[u8]) -> Result<(), String> {
        let word = |i: usize| b.get(8 * i..8 * i + 8).map(|w| i64::from_le_bytes(w.try_into().unwrap())).ok_or("short");
        self.tick = word(0)? as u64;
        for (s, t) in self.seats.iter_mut().enumerate() {
            for (k, v) in t.iter_mut().enumerate() {
                *v = word(1 + 3 * s + k)?;
            }
        }
        Ok(())
    }
}

impl Machine for Tally {
    type S = Tally;
    fn frame(&mut self, np: &mut Netplay<Tally>, local: &[Report]) {
        np.frame(self, local, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        Game::hash(self)
    }
}

// ---------------------------------------------------------------------------------------------------
// The harness
// ---------------------------------------------------------------------------------------------------

/// Seat `seat`'s pad for the frame this machine is at: throttle at a pressure that changes, the stick
/// swinging, a trick chord now and then — the original's bytes, as the shell's pad builds them.
fn mash(seat: usize, frame: usize) -> Report {
    let phase = (frame / 20 + seat * 7) % 11;
    let mut buttons = Control::Cross.mask();
    if phase == 3 {
        buttons |= Control::Circle.mask() | Control::Up.mask();
    }
    if phase == 7 {
        buttons = Control::Square.mask();
    }
    let mut r = Report::from_buttons(buttons);
    if buttons & Control::Cross.mask() != 0 {
        r.0[Control::Cross.binding().pressure as usize] = (40 + 21 * phase) as u8;
    }
    r.set_axis(2, ((frame * (seat + 3) * 5) % 256) as u8);
    r.set_axis(3, (0x80 + ((frame / 7 + seat) % 9) * 9) as u8);
    r
}

struct Link {
    /// The share of datagrams lost, the rounds the rest take, the rounds in which nothing gets through
    /// (`oneway`: only the host's datagrams are lost then).
    loss: f64,
    lag: u32,
    outage: Range<u32>,
    oneway: bool,
}

struct Run<M: Machine> {
    names: Vec<String>,
    nets: Vec<Netplay<M::S>>,
    games: Vec<M>,
    /// Each machine's tick after every round.
    ran: Vec<Vec<usize>>,
    biggest: usize,
    /// Each machine's datagrams and their bytes, sent in all (lost ones too).
    sent: Vec<(u64, u64)>,
}

fn dumps() -> PathBuf {
    std::env::temp_dir().join(format!("openmx-netplay-{}", std::process::id()))
}

/// `lineup`'s machines, `make(seats)` each one's game, `rounds` rounds over `link`. `each(round, run,
/// live)` runs after every round, and may take a machine off the network (`live[i] = false`: it
/// neither runs nor hears nor is heard).
fn run<M: Machine>(
    lineup: &str,
    timing: Timing,
    make: impl Fn(usize) -> M,
    link: &Link,
    rounds: Range<u32>,
    each: impl FnMut(u32, &mut Run<M>, &mut Vec<bool>),
) -> Run<M> {
    run_asleep(lineup, timing, make, link, rounds, &[], each)
}

/// [`run`] with the machines `asleep` off the network from the first round, until `each` wakes them:
/// a machine still loading its ride (P127).
fn run_asleep<M: Machine>(
    lineup: &str,
    timing: Timing,
    make: impl Fn(usize) -> M,
    link: &Link,
    rounds: Range<u32>,
    asleep: &[usize],
    mut each: impl FnMut(u32, &mut Run<M>, &mut Vec<bool>),
) -> Run<M> {
    let lineup = Lineup::parse(lineup).unwrap();
    let names = lineup.peers();
    // driven before the sessions are made: ggrs reads the clock as it starts them
    ggrs::clock::drive(true);
    let mut r = Run {
        nets: names.iter().map(|me| Netplay::new(lineup.clone(), me, timing, dumps()).unwrap()).collect(),
        games: names.iter().map(|_| make(lineup.seats.len())).collect(),
        ran: vec![Vec::new(); names.len()],
        sent: vec![(0, 0); names.len()],
        names,
        biggest: 0,
    };
    let mut live: Vec<bool> = (0..r.names.len()).map(|i| !asleep.contains(&i)).collect();
    let mut wire: Vec<(usize, u32, String, Vec<u8>)> = Vec::new();
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut lost = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f64 / (1u64 << 53) as f64 <= link.loss
    };
    for round in rounds {
        for (to, at, from, d) in std::mem::take(&mut wire) {
            if at > round {
                wire.push((to, at, from, d));
            } else if live[to] {
                r.nets[to].heard(&from, &d);
            }
        }
        for i in (0..r.names.len()).filter(|&i| live[i]) {
            let np = &mut r.nets[i];
            let local: Vec<Report> = np.lineup.mine(&np.me).into_iter().map(|s| mash(s, np.tick())).collect();
            r.games[i].frame(np, &local);
            for (to, d) in np.outgoing() {
                r.biggest = r.biggest.max(d.len());
                r.sent[i].0 += 1;
                r.sent[i].1 += d.len() as u64;
                let out = link.outage.contains(&round) && (!link.oneway || i == 0);
                if out || lost() || d.len() > DATAGRAM_MAX {
                    continue;
                }
                let j = r.names.iter().position(|n| *n == to).unwrap();
                wire.push((j, round + link.lag, r.names[i].clone(), d));
            }
            np.said.clear();
            r.ran[i].push(np.tick());
        }
        each(round, &mut r, &mut live);
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    r
}

impl<M: Machine> Run<M> {
    /// The ticks every machine in `who` has every seat's pads for.
    fn settled(&self, who: &[usize]) -> usize {
        who.iter().map(|&i| self.nets[i].confirmed().max(0) as usize).min().unwrap()
    }

    /// Every machine in `who` ran the same pads and had the same state before every settled tick
    /// from `from`: how many ticks that is.
    fn agree(&self, who: &[usize], from: usize) -> usize {
        let n = self.settled(who);
        let first = &self.nets[who[0]];
        for &i in who {
            let np = &self.nets[i];
            assert!(np.log.len() >= n && np.hashes.len() >= n, "{}: {} ticks logged, {n} settled", np.me, np.log.len());
            let at = (from..n).find(|&t| np.hashes[t] != first.hashes[t]);
            assert_eq!(at, None, "{}'s hash differs from {}'s at tick {at:?}", np.me, first.me);
            let at = (from..n).find(|&t| np.log[t] != first.log[t]);
            assert_eq!(at, None, "{} ran other pads than {} at tick {at:?}", np.me, first.me);
        }
        n - from
    }
}

// ---------------------------------------------------------------------------------------------------
// The wire
// ---------------------------------------------------------------------------------------------------

#[test]
fn a_pad_goes_on_the_wire_as_its_own_bytes() {
    for r in [Report::idle(), Report::missing(), mash(3, 77), mash(0, 1234)] {
        let w = wire(&r);
        // the game's twenty bytes, then the machine the seat wants (N17), which no pad sets
        assert_eq!(w.0[..], r.0[..net::WIRE], "the wire is the report's first {} bytes", net::WIRE);
        assert_eq!((net::WIRE, r.0[net::SWITCH]), (21, 0));
        assert_eq!(unwire(w), r, "and comes back as the report");
    }
    assert_eq!(net::Pad::default(), wire(&Report::idle()));
}

#[test]
fn a_big_ggrs_message_goes_in_pieces() {
    use net::link::{Kind, Pieces, ggrs, open};
    let body: Vec<u8> = (0..3000u32).map(|i| (i * 7) as u8).collect();
    assert_eq!(ggrs(0, 9, &body[..100]).len(), 1, "a small one is one datagram");
    let parts = ggrs(2, 9, &body);
    assert_eq!(parts.len(), 3);
    assert!(parts.iter().all(|d| d.len() <= DATAGRAM_MAX));
    let mut got = Pieces::default();
    let mut put = |got: &mut Pieces, d: &[u8]| match open(d) {
        Some(Kind::Piece { round: 2, id, index, count, body }) => got.heard("g", id, index, count, body),
        _ => panic!("not a piece"),
    };
    // out of order, one of an older message lost in between
    assert_eq!(put(&mut got, &ggrs(2, 8, &body)[0]), None);
    assert_eq!(put(&mut got, &parts[2]), None);
    assert_eq!(put(&mut got, &parts[0]), None);
    assert_eq!(put(&mut got, &ggrs(2, 8, &body)[1]), None, "the older message's late piece");
    assert_eq!(put(&mut got, &parts[1]).as_deref(), Some(&body[..]));
}

#[test]
fn a_lineup_reads_and_names_its_host() {
    let l = Lineup::parse("host,host,g,-,k,g").unwrap();
    assert_eq!(l.host(), "host");
    assert_eq!(l.peers(), ["host", "g", "k"]);
    assert_eq!(l.mine("g"), [2, 5]);
    assert_eq!(l.text(), "host,host,g,-,k,g");
    let names = |n: usize| (0..n).map(|i| format!("p{i}")).collect::<Vec<_>>().join(",");
    assert_eq!(Lineup::parse(&names(net::SEATS)).unwrap().seats.len(), net::SEATS, "as many as the cap");
    let past = Lineup::parse(&names(net::SEATS + 1)).unwrap_err();
    assert!(past.contains(&format!("the most is {}", net::SEATS)), "{past}");
    assert!(Lineup::parse("-,-").is_err(), "nobody");
}

/// N6: the online ride's frame length is the offline 30 Hz ride's — 0.033 after the load, up to the
/// cap within about a second, and the cap for good (`pace.rs` holds it to the offline engine's tick
/// for tick).
#[test]
fn the_online_step_is_the_30_hz_one() {
    use game_sim::tick::DeltaTime;
    assert!((rides::tick_dt(0).0 - DeltaTime::AFTER_LONG_FRAME).abs() < 1e-6, "{:?}", rides::tick_dt(0));
    let settled = (0..100).position(|t| rides::tick_dt(t) == F(DeltaTime::CAP_30)).expect("it reaches the cap");
    assert!(settled < 40, "{settled} ticks to the cap");
    assert!((settled..100_000).step_by(997).all(|t| rides::tick_dt(t) == F(DeltaTime::CAP_30)));
}

// ---------------------------------------------------------------------------------------------------
// A bad network
// ---------------------------------------------------------------------------------------------------

/// opentennis's B198 acceptance, on the ride: 20 % of datagrams lost and the rest 3 rounds (100 ms)
/// on their way, then for 3 s nothing gets through (`oneway`: nothing from the host). Every machine's
/// ticks go on within a second of the network coming back, nobody waits a second outside it, and every
/// settled tick has the same pads and the same state everywhere. No desync, no datagram too big.
fn on_a_bad_network(lineup: &str, oneway: bool) {
    let link = Link { loss: 0.2, lag: 3, outage: 120..210, oneway };
    let r = run(lineup, Timing::default(), Ride::new, &link, 0..360, |_, _, _| {});
    let all: Vec<usize> = (0..r.names.len()).collect();
    for (i, f) in r.ran.iter().enumerate() {
        let (mut still, mut worst) = (0, 0);
        for k in 60..f.len() {
            still = if f[k] == f[k - 1] { still + 1 } else { 0 };
            if !(link.outage.start..link.outage.end + 30).contains(&(k as u32)) {
                worst = worst.max(still);
            }
        }
        let np = &r.nets[i];
        eprintln!("{}: {} ticks, longest wait {worst} rounds, {} held, {} re-run, {} resent", np.me, f[f.len() - 1], np.rb.held, np.rb.reruns, np.rb.resent);
        assert!(worst < 30, "{} waited {worst} rounds", np.me);
        assert!(np.desyncs.is_empty() && !np.desynced, "{}: {:?}", np.me, np.desyncs);
        assert_eq!(np.end, None);
    }
    assert!(r.biggest <= DATAGRAM_MAX, "a {}-byte datagram", r.biggest);
    let n = r.agree(&all, 0);
    eprintln!("{n} ticks settled and agreed on every machine");
    assert!(n > 250, "only {n} ticks settled");
    // every machine re-ran ticks: the guesses were wrong and were put right
    assert!(r.nets.iter().all(|np| np.rb.reruns > 0));
}

#[test]
fn two_peers_on_a_bad_network() {
    // two seats on each machine
    on_a_bad_network("host,host,g,g", false);
}

#[test]
fn four_peers_and_eight_riders_on_a_bad_network() {
    on_a_bad_network("host,h,g,g,k,host,g,k", false);
}

#[test]
fn a_one_way_outage() {
    on_a_bad_network("host,g", true);
}

// ---------------------------------------------------------------------------------------------------
// A desync
// ---------------------------------------------------------------------------------------------------

/// The ride runs differently on one machine: both see it at the next checksum, both save every
/// seat's pads, and the host's report, ridden again offline from a fresh start, is the host's own
/// ride hash for hash. Then the host's state — every seat's [`Sim`] in N1's codec, many pieces — goes
/// over a lossy link, the guest takes it on, and from the tick it took it at every settled tick agrees
/// again: one resync each side and nothing out of sync after.
#[test]
fn a_desync_on_the_ride_is_caught_saved_and_resynced() {
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    let r = run("host,g", Timing::default(), Ride::new, &link, 0..900, |round, r, _| {
        if round == 0 {
            r.games[1].glitch = true;
        }
    });
    let fresh = r.nets[1].resync.fresh as usize;
    // A machine sees the desync at the first checksum after the glitch it heard: ggrs never sends a
    // lost checksum report again, so one the link dropped is caught at the next (on this link's
    // losses the host's 330 is lost and it sees 360; the guest sees 330).
    let mut seen = Vec::new();
    for np in &r.nets {
        let first = np.desyncs.first().unwrap_or_else(|| panic!("{} saw no desync", np.me)).0;
        assert!(first >= 330 && first % 30 == 0 && first <= 390, "{}: first seen at {first}", np.me);
        seen.push(first);
        assert!(np.desyncs.iter().all(|d| d.0 == first || (d.0 as usize) < fresh.max(np.resync.fresh as usize)), "{}: {:?}", np.me, np.desyncs);
        assert_eq!(np.resync.done, 1, "{}: resyncs", np.me);
        assert!(!np.desynced, "{} still out of sync", np.me);
        let at = np.report.as_ref().expect("a report");
        let report = net::desync::Report::read(at).unwrap();
        assert_eq!((report.me.as_str(), &report.lineup), (np.me.as_str(), &np.lineup));
        assert!(report.why.starts_with(&format!("Out of sync at tick {first}")), "{}", report.why);
        if np.me == "host" {
            let mut sims = sims(2);
            let mut world = Flat::default();
            let mut game = Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None };
            let hashes = report.replay(&mut game);
            // through the tick it was caught at, every pad was in; the newest ticks' may have been
            // guesses the host ran again on the real ones after the report was written
            assert!(hashes.len() > first as usize);
            let to = first as usize + 1;
            assert!(hashes[..to] == np.hashes[..to], "the report rides again to the host's hashes");
        }
        std::fs::remove_file(at).unwrap();
    }
    assert!(seen.contains(&330), "nobody saw the first checksum after the glitch: {seen:?}");
    assert!(fresh > 330, "taken at {fresh}");
    let n = r.agree(&[0, 1], fresh);
    assert!(n > 300, "only {n} ticks agreed after the resync");
    assert_eq!(r.nets[0].hashes[..320], r.nets[1].hashes[..320]);
    assert_ne!(r.nets[0].hashes[330], r.nets[1].hashes[330]);
}

/// [`Rides`]' bytes: every seat's state goes over and comes back the same ride, the constants stay
/// the machine's own, and a state for another number of seats or cut short is refused with nothing
/// written.
#[test]
fn the_ride_says_every_seat_in_bytes() {
    let mut world = Flat::default();
    let mut there = sims(3);
    let pads = |t: usize| (0..3).map(|s| mash(s, t)).collect::<Vec<_>>();
    let mut game = Rides { sims: &mut there, world: &mut world, after: None, time_scale: None, plane: None };
    for t in 0..200 {
        game.step(t, &pads(t));
    }
    let bytes = game.encode().expect("the ride has a codec");
    let hash = game.hash();
    let mut here = sims(3);
    let mut world2 = Flat::default();
    let mut mine = Rides { sims: &mut here, world: &mut world2, after: None, time_scale: None, plane: None };
    assert_ne!(mine.hash(), hash);
    mine.decode(&bytes).unwrap();
    assert_eq!(mine.hash(), hash);
    assert_eq!(mine.encode().unwrap(), bytes);
    // and they ride on as one
    for t in 200..400 {
        mine.step(t, &pads(t));
        game.step(t, &pads(t));
    }
    assert_eq!(mine.hash(), game.hash());
    let before = mine.hash();
    let mut two = sims(2);
    let mut world3 = Flat::default();
    let mut other = Rides { sims: &mut two, world: &mut world3, after: None, time_scale: None, plane: None };
    let theirs = other.encode().unwrap();
    assert!(mine.decode(&theirs).unwrap_err().contains("2 seats in the state, 3 here"));
    let cut = mine.encode().unwrap();
    assert!(mine.decode(&cut[..cut.len() - 1]).is_err());
    assert!(mine.decode(&[cut.clone(), vec![0]].concat()).is_err());
    assert_eq!(mine.hash(), before, "a refused state writes nothing");
}

/// N13: each seat's controller preset is a constant of the ride that every machine must hold the same.
/// It rides in the ride's bytes, so a state decoded onto seats built on preset 0 (a resync, a
/// restart, the seats a join adds) reads each seat through its own; a joiner's seats spawn on the
/// presets its spawn says; and it matters — the same pads on another preset are another ride.
#[test]
fn every_seat_rides_its_own_preset_and_says_it_in_bytes() {
    let mut world = Flat::default();
    let mut there = sims(3);
    for (sim, p) in there.iter_mut().zip([1, 0, 2]) {
        sim.set_presets([p, 0]);
    }
    let pads = |t: usize| (0..3).map(|s| mash(s, t)).collect::<Vec<_>>();
    let mut game = Rides { sims: &mut there, world: &mut world, after: None, time_scale: None, plane: None };
    for t in 0..300 {
        game.step(t, &pads(t));
    }
    let bytes = game.encode().unwrap();
    let mut here = sims(2);
    let mut world2 = Flat::default();
    let mut mine = Rides { sims: &mut here, world: &mut world2, after: None, time_scale: None, plane: None };
    mine.decode(&bytes).unwrap();
    assert_eq!(mine.sims.iter().map(rides::preset).collect::<Vec<_>>(), [Some(1), Some(0), Some(2)], "the third seat grown, too");
    assert_eq!(mine.hash(), game.hash());
    let mut shipped: Vec<Sim> = mine.sims.clone();
    for t in 300..600 {
        mine.step(t, &pads(t));
        game.step(t, &pads(t));
    }
    assert_eq!(mine.hash(), game.hash(), "the same presets ride as one");
    // the same state and pads with every seat on the shipped table: preset 1's Square is not a brake
    for sim in &mut shipped {
        sim.set_presets([0, 0]);
    }
    let mut world3 = Flat::default();
    let mut off = Rides { sims: &mut shipped, world: &mut world3, after: None, time_scale: None, plane: None };
    for t in 300..600 {
        off.step(t, &pads(t));
    }
    assert_ne!(off.sims[0].hash(), game.sims[0].hash(), "seat 1 on preset 0 is another ride");
    assert_eq!(off.sims[1].hash(), game.sims[1].hash(), "seat 2 was on preset 0 all along");

    // a joiner's seats, each on its own preset; one the list leaves out on the spawn's
    let spawn = rides::seat_bytes(&sims(1)[0]);
    mine.seat(3, &rides::spawn_on(&spawn, &[(3, 3), (4, 2)])).unwrap();
    mine.seat(4, &rides::spawn_on(&spawn, &[(3, 3), (4, 2)])).unwrap();
    mine.seat(5, &rides::spawn_on(&spawn, &[])).unwrap();
    assert_eq!(mine.sims[3..].iter().map(rides::preset).collect::<Vec<_>>(), [Some(3), Some(2), Some(0)]);
    // and a preset the original has not is refused
    let mut bad = rides::seat_bytes(&sims(1)[0]);
    bad[0] = game_sim::bindings::PRESETS as u8;
    assert!(rides::from_seat_bytes(&mut sims(1)[0], &bad).unwrap_err().contains("controller preset"));
}

/// [`mash`]'s pads are a ride as the game reads them (N9): every seat's bike is on the gas nearly
/// every tick at more than one pressure, brakes now and then, holds a stunt on the chord, and the
/// d-pad the chord touches (up) never reaches the hands: since P129 it is the markers' with a trick
/// button down too. And the bikes get going.
#[test]
fn the_mashed_pads_are_a_ride() {
    let mut world = Flat::default();
    let mut sims = sims(3);
    #[derive(Default, Debug)]
    struct Seen {
        gas: usize,
        gases: std::collections::BTreeSet<u32>,
        brake: usize,
        stunt: usize,
        down: usize,
        up: usize,
        speed: f32,
    }
    let mut seen: Vec<Seen> = (0..3).map(|_| Seen::default()).collect();
    let ticks = 900;
    {
        let mut after = |seat: usize, _: F, sim: &mut Sim, c: &game_sim::ride::Controls| {
            let s = &mut seen[seat];
            if c.gas > F::ZERO {
                s.gas += 1;
                s.gases.insert(c.gas.0.to_bits());
            }
            s.brake += (c.brake > F::ZERO) as usize;
            s.stunt += c.stunt as usize;
            s.down += c.dpad[3] as usize;
            s.up += c.dpad[2] as usize;
            s.speed = s.speed.max(sim.rider.ride.chassis.speed.0);
        };
        let mut game = Rides { sims: &mut sims, world: &mut world, after: Some(&mut after), time_scale: None, plane: None };
        for t in 0..ticks {
            game.step(t, &(0..3).map(|s| mash(s, t)).collect::<Vec<_>>());
        }
    }
    for (seat, s) in seen.iter().enumerate() {
        assert!(s.gas > ticks * 8 / 11 && s.gases.len() > 5, "seat {seat}: {s:?}");
        assert!(s.brake > 0 && s.stunt > 0, "seat {seat}: {s:?}");
        assert_eq!((s.up, s.down), (0, 0), "seat {seat}: the d-pad reached the hands");
        assert!(s.speed > 10.0, "seat {seat} never got going: {s:?}");
    }
}

/// The same on [`Tally`], which can say its state in bytes: caught at 330 on both, the host's state
/// goes out in pieces over a lossy link, the guest takes it and runs on, and from the tick it took it
/// at every settled tick agrees again — no desync after, one resync each side.
#[test]
fn a_desync_is_caught_and_resynced() {
    let link = Link { loss: 0.2, lag: 4, outage: 0..0, oneway: false };
    let r = run("host,g,g,host", Timing::default(), Tally::new, &link, 0..900, |round, r, _| {
        if round == 0 {
            r.games[1].glitch = true;
        }
    });
    let fresh = r.nets[1].resync.fresh as usize;
    for np in &r.nets {
        assert_eq!(np.desyncs.first().map(|d| d.0), Some(330), "{}", np.me);
        assert!(np.desyncs.iter().all(|d| (d.0 as usize) < fresh.max(np.resync.fresh as usize) || d.0 == 330), "{}: {:?}", np.me, np.desyncs);
        assert_eq!(np.resync.done, 1, "{}: resyncs", np.me);
        assert!(!np.desynced, "{} still out of sync", np.me);
        std::fs::remove_file(np.report.as_ref().expect("a report")).unwrap();
    }
    assert!(fresh > 330, "taken at {fresh}");
    let n = r.agree(&[0, 1], fresh);
    assert!(n > 300, "only {n} ticks agreed after the resync");
    // and before the glitch they agreed too; between it and the resync they did not
    assert_eq!(r.nets[0].hashes[..321], r.nets[1].hashes[..321]);
    assert_ne!(r.nets[0].hashes[330], r.nets[1].hashes[330]);
}

// ---------------------------------------------------------------------------------------------------
// A silent peer
// ---------------------------------------------------------------------------------------------------

fn short() -> Timing {
    Timing { wait: 5.0, ..Timing::default() }
}

/// `g`'s load takes 20 s (P127: its machine has said nothing yet, as the shell's online ride makes its
/// session only once the ride is built): the host stays on tick 0 all that time, nobody counts `g`
/// as away, and once `g` is in the two start together, in sync from the first tick.
#[test]
fn the_ride_starts_only_once_a_slow_loading_peer_is_in() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let loaded = 600;
    let mut held = Vec::new();
    let r = run_asleep("host,g", Timing::default(), Tally::new, &link, 0..loaded + 300, &[1], |round, r, live| {
        live[1] = round + 1 >= loaded;
        if round + 1 < loaded {
            held.push(r.nets[0].tick());
            assert!(r.nets[0].recover.away.is_empty(), "g away at round {round} while loading");
        }
    });
    assert!(held.iter().all(|&t| t == 0), "the host rode on before g was in: {:?}", held.iter().max());
    assert!(r.nets[0].tick() > 200 && r.nets[1].tick() > 200, "{} {}", r.nets[0].tick(), r.nets[1].tick());
    assert!(r.agree(&[0, 1], 0) > 200);
    assert_eq!((r.nets[0].recover.restarts, r.nets[0].end.as_deref()), (0, None));
}

/// `h` goes silent at round 150 (5 s) for good: the others hold and say so, and after the wait the host
/// restarts the ride without it — its seat empty — and the two ride on in sync.
#[test]
fn a_silent_peer_is_waited_for_then_ridden_on_without() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let mut notices = Vec::new();
    let mut held = None;
    let r = run("host,h,g", short(), Tally::new, &link, 0..750, |round, r, live| {
        if round == 150 {
            live[1] = false;
        }
        if round == 300 {
            notices.push(r.nets[2].notice.clone());
            held = Some((r.nets[0].tick(), r.nets[2].tick()));
        }
        if round == 330 {
            assert_eq!(Some((r.nets[0].tick(), r.nets[2].tick())), held, "the ride held while h was away");
        }
    });
    assert!(notices[0].starts_with("Waiting for h..."), "{:?}", notices);
    for i in [0, 2] {
        let np = &r.nets[i];
        assert_eq!(np.lineup.text(), "host,-,g", "{}", np.me);
        assert_eq!((np.recover.restarts, np.rb.round, np.end.as_deref()), (1, 1, None), "{}", np.me);
    }
    let from = r.nets[0].rb.base;
    let n = r.agree(&[0, 2], 0);
    assert!(n > from + 150, "{n} settled, restarted at {from}");
    // the empty seat's pad is unplugged from the restart on
    assert_eq!(r.nets[0].log[from + 10][1], wire(&Report::missing()));
}

/// `h` silent for 4 s, past the 3 s that makes it away but inside the wait: the host says so, and
/// when it is heard again the ride goes on with everyone in it, nothing restarted.
#[test]
fn an_away_peer_back_in_time_rides_on() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let mut away = false;
    let r = run("host,h,g", short(), Tally::new, &link, 0..600, |round, r, live| {
        live[1] = !(150..270).contains(&round);
        away |= r.nets[0].recover.away == ["h"];
    });
    assert!(away, "h was never away");
    for np in &r.nets {
        assert_eq!((np.recover.restarts, np.lineup.text().as_str(), np.end.as_deref()), (0, "host,h,g", None), "{}", np.me);
        assert!(np.recover.away.is_empty());
    }
    let n = r.agree(&[0, 1, 2], 0);
    assert!(n > 350, "only {n} ticks settled");
}

/// A [`Tally`] that cannot say its state in bytes: nobody can take its ride over.
struct Mute(Tally);

impl Game for Mute {
    type State = Tally;
    fn save(&self) -> Tally {
        self.0.save()
    }
    fn restore(&mut self, s: &Tally) {
        self.0.restore(s)
    }
    fn step(&mut self, tick: usize, pads: &[Report]) {
        self.0.step(tick, pads)
    }
    fn hash(&self) -> u64 {
        Game::hash(&self.0)
    }
    fn encode(&self) -> Option<Vec<u8>> {
        None
    }
    fn decode(&mut self, _: &[u8]) -> Result<(), String> {
        Err("no bytes".into())
    }
}

impl Machine for Mute {
    type S = Tally;
    fn frame(&mut self, np: &mut Netplay<Tally>, local: &[Report]) {
        np.frame(self, local, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        Game::hash(&self.0)
    }
}

/// The host gone for good, on a ride that cannot hand its state over: the others wait, and the one
/// that would take it over ends it for everyone, saying why (N11).
#[test]
fn a_silent_host_whose_ride_cannot_be_handed_over_ends_it() {
    let link = Link { loss: 0.0, lag: 2, outage: 0..0, oneway: false };
    let r = run("host,g,k", short(), |n| Mute(Tally::new(n)), &link, 0..800, |round, _, live| {
        if round == 100 {
            live[0] = false;
        }
    });
    let why = "host went silent, and this ride cannot hand its state over to go on without them";
    assert_eq!((r.nets[1].end.as_deref(), r.nets[2].end.as_deref()), (Some(why), Some(why)));
}

/// N11: the host gone for good (killed, nothing said): the others hold, and once it has been silent
/// past the away time and the host's wait, the peer in the next seat takes the ride over — its
/// settled state and the line-up without the host, as a restart — and hosts it from there: the two
/// ride on in sync, and when the third peer goes silent too, the new host empties its seat as the
/// old one would have.
#[test]
fn a_silent_hosts_ride_is_taken_over_by_the_next_seat() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let (mut notice, mut went, mut taken_at) = (String::new(), 0, None);
    let r = run("host,g,k,m", short(), Ride::new, &link, 0..1500, |round, r, live| {
        if round == 150 {
            went = r.nets[1].tick();
            live[0] = false;
        }
        if taken_at.is_none() && !r.nets[1].recover.takeovers.is_empty() {
            taken_at = Some(round);
        }
        if round == 300 {
            notice = r.nets[2].notice.clone();
        }
        if round == 900 {
            live[3] = false;
        }
    });
    assert!(notice.starts_with("The host stopped answering: g takes over in"), "{notice:?}");
    let (g, k) = (&r.nets[1], &r.nets[2]);
    for np in [g, k] {
        assert_eq!(np.lineup.text(), "-,g,k,-", "{}", np.me);
        assert_eq!((np.recover.restarts, np.end.as_deref()), (2, None), "{}", np.me);
        assert!(np.desyncs.is_empty(), "{}: {:?}", np.me, np.desyncs);
    }
    assert!(g.host() && !k.host());
    let taken = g.recover.takeovers.clone();
    assert_eq!(taken.len(), 1, "{taken:?}");
    assert_eq!(taken[0].0, "host");
    let t = taken[0].1 as usize;
    // the host silent 3 s, then g's 10 s: about 390 rounds after it went; from about the tick the
    // ride stalled at without it
    let at = taken_at.unwrap();
    assert!((150 + 360..150 + 450).contains(&at), "taken over at round {at}");
    assert!((went.saturating_sub(10)..went + 20).contains(&t), "taken over at tick {t}, the host went at {went}");
    // every settled tick from it, past the hold for m's wait
    let n = r.agree(&[1, 2], t);
    assert!(n > 500, "g and k agree on {n} ticks from {t}");
    assert_eq!(g.log[t + 10][0], wire(&Report::missing()), "the old host's seat is unplugged");
    assert_eq!(g.log[g.rb.base + 10][3], wire(&Report::missing()), "m's seat emptied by the new host");
    assert_eq!(r.nets[3].lineup.text(), "-,g,k,m", "m took the takeover before it went");
}

/// N11's acceptance, session side: of four machines in a freeride — the host and three peers — `g`
/// leaves from the pause menu and `k` is killed. `g`'s seat goes at one tick on every machine, the
/// ride carrying on without a wait; `k`'s once it has been silent the wait. The host and `m` ride on
/// with every settled tick's state and pads agreeing.
#[test]
fn of_three_peers_one_leaves_from_the_menu_and_one_is_killed_and_the_ride_goes_on() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let (mut left_at, mut asked, mut killed) = (None, 0, 0);
    let r = run("host,g,k,m", short(), Ride::new, &link, 0..1100, |round, r, live| {
        if round == 200 {
            asked = r.nets[1].tick();
            r.nets[1].leave(&[]).unwrap();
            assert!(r.nets[1].leave(&[5]).is_err(), "not g's seat");
        }
        if left_at.is_none() && r.nets[1].end.is_some() {
            left_at = Some(round);
        }
        if round == 400 {
            killed = r.nets[0].tick();
            live[2] = false;
        }
    });
    let (h, g, m) = (&r.nets[0], &r.nets[1], &r.nets[3]);
    assert_eq!(g.end.as_deref(), Some("you left the ride"));
    assert!(g.flushed());
    // g was out within a second: the restart's round trip, no away wait
    let out = left_at.unwrap();
    assert!(out < 230, "g out at round {out}");
    let left = &h.leave.left;
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!((left[0].0.as_str(), left[0].1.as_slice()), ("g", &[1usize][..]));
    // at a tick about when g asked: the host's last settled one when its Leave came in
    let t = left[0].2 as usize;
    assert!((asked - 10..asked + 30).contains(&t), "g's seat went at tick {t}, asked at {asked}");
    for np in [h, m] {
        assert_eq!(np.lineup.text(), "host,-,-,m", "{}", np.me);
        assert_eq!((np.recover.restarts, np.end.as_deref()), (2, None), "{}", np.me);
        assert!(np.desyncs.is_empty(), "{}: {:?}", np.me, np.desyncs);
    }
    // from g's leave on, the host and m (k too, until it went) agree
    // every settled tick from it: all but the hold for k's wait
    let n = r.agree(&[0, 3], t);
    assert!(n > 450, "the host and m agree on {n} ticks from {t}");
    assert_eq!(h.log[t + 10][1], wire(&Report::missing()), "g's seat unplugged from its leave");
    // the ride held from about where k went until its wait was up, and went on from there
    let k_out = h.rb.base;
    assert!((killed.saturating_sub(10)..killed + 20).contains(&k_out), "k's seat emptied at {k_out}, k went at {killed}");
    assert_eq!(h.log[k_out + 10][2], wire(&Report::missing()));
    assert!(r.games[0].sims[3].ticks > k_out as u64 + 300, "m rode on");
}

/// N11: the host leaves from the pause menu. Its seat goes at one tick like anyone's, and the
/// restart that takes it out names the peer in the next seat the host: that peer hosts the ride
/// from there — here it empties a silent peer's seat, a host's job — and the ride goes on. The old
/// host keeps sending until both peers have the restart, then it is through.
#[test]
fn the_host_leaving_hands_the_ride_to_the_next_seat() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let (mut asked, mut killed) = (0, 0);
    let r = run("host,g,k,m", short(), Ride::new, &link, 0..1000, |round, r, live| {
        if round == 200 {
            asked = r.nets[0].tick();
            r.nets[0].leave(&[]).unwrap();
        }
        if round == 400 {
            killed = r.nets[1].tick();
            live[3] = false;
        }
    });
    let (h, g, k) = (&r.nets[0], &r.nets[1], &r.nets[2]);
    assert_eq!(h.end.as_deref(), Some("you left the ride: g hosts it now"));
    assert!(h.flushed());
    let t = h.leave.left[0].2 as usize;
    assert!((asked - 10..asked + 30).contains(&t), "the host's seat went at tick {t}, asked at {asked}");
    for np in [g, k] {
        assert_eq!(np.lineup.text(), "-,g,k,-", "{}", np.me);
        assert_eq!((np.lineup.host(), np.recover.restarts, np.end.as_deref()), ("g", 2, None), "{}", np.me);
        assert!(np.desyncs.is_empty(), "{}: {:?}", np.me, np.desyncs);
    }
    assert!(g.recover.takeovers.is_empty(), "handed over, not taken");
    // every settled tick from it, past the hold for m's wait
    let n = r.agree(&[1, 2], t);
    assert!(n > 400, "g and k agree on {n} ticks from {t}");
    assert_eq!(g.log[t + 10][0], wire(&Report::missing()), "the old host's seat unplugged");
    let m_out = g.rb.base;
    assert!((killed.saturating_sub(10)..killed + 20).contains(&m_out), "m's seat emptied by g at {m_out}, m went at {killed}");
    assert_eq!(g.log[m_out + 10][3], wire(&Report::missing()));
}

/// N11: a machine with two seats lets one player go (a second local player leaving from their
/// pause): that seat goes at one tick, and the machine rides on in the other.
#[test]
fn a_local_seat_on_an_online_peer_leaves_and_its_machine_rides_on() {
    let link = Link { loss: 0.05, lag: 2, outage: 0..0, oneway: false };
    let r = run("host,g,g", short(), Tally::new, &link, 0..500, |round, r, _| {
        if round == 150 {
            r.nets[1].leave(&[2]).unwrap();
        }
    });
    let (h, g) = (&r.nets[0], &r.nets[1]);
    for np in [h, g] {
        assert_eq!((np.lineup.text().as_str(), np.recover.restarts, np.end.as_deref()), ("host,g,-", 1, None), "{}", np.me);
    }
    assert!(!g.leave.leaving(), "g is still in, on seat 2");
    let t = h.leave.left[0].2 as usize;
    assert!(r.agree(&[0, 1], t) > 250);
    assert_eq!(h.log[t + 5][2], wire(&Report::missing()));
}

/// On the ride: `h` goes silent at round 150 for good, the others hold, and after the wait the host
/// hands everyone its state and the new line-up — `h`'s seat empty, its bike left with an unplugged
/// pad — and the two ride on in sync, every seat's state alike on both.
#[test]
fn on_the_ride_a_silent_peers_seat_empties_and_the_others_ride_on() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let r = run("host,h,g", short(), Ride::new, &link, 0..750, |round, _, live| {
        if round == 150 {
            live[1] = false;
        }
    });
    for i in [0, 2] {
        let np = &r.nets[i];
        assert_eq!(np.lineup.text(), "host,-,g", "{}", np.me);
        assert_eq!((np.recover.restarts, np.rb.round, np.end.as_deref()), (1, 1, None), "{}", np.me);
    }
    let from = r.nets[0].rb.base;
    let n = r.agree(&[0, 2], 0);
    assert!(n > from + 150, "{n} settled, restarted at {from}");
    assert_eq!(r.nets[0].log[from + 10][1], wire(&Report::missing()));
    // the empty seat is still a bike on the track (its hash is in the agreeing ones above)
    let a = &r.games[0];
    assert!(a.sims[1].ticks > from as u64 + 150, "the empty seat rode on: {} ticks", a.sims[1].ticks);
    let _ = WINDOW;
}

// ---------------------------------------------------------------------------------------------------
// Joining a ride under way (N10)
// ---------------------------------------------------------------------------------------------------

/// A freeride opened online: the host rides alone in a session of one, `g` joins on seat 2 at round
/// `joins[0]` and `k` on seats 3 and 4 at `joins[1]`, over a lossy link. As the lobby has it, the
/// host is asked to take each on (its handshake passed) 30 rounds before the joiner's game is up,
/// loading its ride; the joiner then comes as [`Netplay::joining`] (no seat, no session), and is
/// taken on once the host hears it. The run's machines in join order, and the host's tick and its
/// first seat's own tick count after every round.
fn joined_mid_ride(link: &Link, joins: [u32; 2], rounds: u32) -> (Run<Ride>, Vec<(usize, u64)>) {
    ggrs::clock::drive(true);
    let spawn = rides::seat_bytes(&sims(1)[0]);
    // every rider on a controller preset of their own (N13): the host's on 3, g's 2, k's 1 and 0
    let mut first = Ride::new(1);
    first.sims[0].set_presets([3, 0]);
    let mut r: Run<Ride> = Run {
        names: vec!["host".into()],
        nets: vec![Netplay::new(Lineup::parse("host").unwrap(), "host", Timing::default(), dumps()).unwrap()],
        games: vec![first],
        ran: vec![Vec::new()],
        biggest: 0,
        sent: Vec::new(),
    };
    let mut host = Vec::new();
    let mut wire: Vec<(usize, u32, String, Vec<u8>)> = Vec::new();
    let mut rng = 0x9e37_79b9_7f4a_7c15u64;
    let mut lost = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 11) as f64 / (1u64 << 53) as f64 <= link.loss
    };
    for round in 0..rounds {
        for (who, seats, at, presets) in [("g", vec![1], joins[0], vec![(1, 2)]), ("k", vec![2, 3], joins[1], vec![(2, 1), (3, 0)])] {
            if round == at {
                r.names.push(who.into());
                r.nets.push(Netplay::joining("host", who, Timing::default(), dumps()).unwrap());
                r.games.push(Ride::new(1));
                r.ran.push(Vec::new());
            }
            if round + 30 == at {
                r.nets[0].admit(who, &seats, rides::spawn_on(&spawn, &presets)).unwrap();
            }
        }
        for (to, at, from, d) in std::mem::take(&mut wire) {
            if at > round {
                wire.push((to, at, from, d));
            } else {
                r.nets[to].heard(&from, &d);
            }
        }
        for i in 0..r.names.len() {
            let np = &mut r.nets[i];
            let local: Vec<Report> = np.lineup.mine(&np.me).into_iter().map(|s| mash(s, np.tick())).collect();
            r.games[i].frame(np, &local);
            for (to, d) in np.outgoing() {
                r.biggest = r.biggest.max(d.len());
                if lost() || d.len() > DATAGRAM_MAX {
                    continue;
                }
                // a datagram for a machine not on the network yet is lost, as it would be
                if let Some(j) = r.names.iter().position(|n| *n == to) {
                    wire.push((j, round + link.lag, r.names[i].clone(), d));
                }
            }
            np.said.clear();
            r.ran[i].push(np.tick());
        }
        host.push((r.nets[0].tick(), r.games[0].sims[0].ticks));
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    (r, host)
}

/// N10's acceptance, on the ride: a one-peer freeride opens, a second and then a third peer join it
/// mid-ride, and from each join's tick every machine in the ride has the same state hash before
/// every settled tick and ran the same pads; the host's ride never resets — alone, it loses not a
/// tick to the first join, and its ride before it is the ride it had; with others in, the second
/// join steps it back no further than the settled tick, the few ticks a rollback re-runs anyway.
#[test]
fn a_freeride_opened_online_takes_two_peers_mid_ride() {
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    let joins = [200, 500];
    // the host alone, as it rode before anyone came
    let (alone, _) = joined_mid_ride(&link, [u32::MAX - 20, u32::MAX - 10], joins[0] + 100);
    let (r, host) = joined_mid_ride(&link, joins, 1100);
    let (h, g, k) = (&r.nets[0], &r.nets[1], &r.nets[2]);
    let at: Vec<(&str, u32)> = h.admit.joins.iter().map(|(p, t)| (p.as_str(), *t)).collect();
    assert_eq!(at.iter().map(|a| a.0).collect::<Vec<_>>(), ["g", "k"], "{at:?}");
    let (t1, t2) = (at[0].1 as usize, at[1].1 as usize);
    for np in [h, g, k] {
        assert_eq!(np.lineup.text(), "host,g,k,k", "{}", np.me);
        assert_eq!((np.rb.round, np.end.as_deref()), (2, None), "{}", np.me);
        assert!(np.desyncs.is_empty() && !np.desynced, "{}: {:?}", np.me, np.desyncs);
    }
    assert_eq!((h.recover.restarts, g.recover.restarts, k.recover.restarts), (2, 2, 1));
    for i in 0..3 {
        assert_eq!(r.games[i].sims.len(), 4, "{}: every seat a bike", r.names[i]);
        let presets: Vec<Option<usize>> = r.games[i].sims.iter().map(rides::preset).collect();
        assert_eq!(presets, [Some(3), Some(2), Some(1), Some(0)], "{}: every seat on its rider's preset (N13)", r.names[i]);
    }

    // the host: the first join at the tick it was at once g was heard (a keep-alive, half a second
    // in, and the lag), its ride before it untouched, not a tick lost to it
    let before = &alone.nets[0];
    let g_round = joins[0] as usize;
    assert!(t1 > g_round && t1 < g_round + 40, "g heard and taken on at tick {t1}, up at round {g_round}");
    assert_eq!(h.hashes[..t1], before.hashes[..t1], "the host's ride before the join is the one it rode");
    let at = r.ran[0].iter().position(|&t| t >= t1).unwrap();
    assert_eq!(r.ran[0][..=at], alone.ran[0][..=at], "the host's ticks round by round up to the join");
    // after it the host holds while the new session synchronises — ggrs' five round trips, as at
    // every restart, and the state's way to g — then rides on: about 6 rounds a trip at lag 3
    let trip = 2 * link.lag as usize;
    let held = r.ran[0][at..at + 120].windows(2).filter(|w| w[0] == w[1]).count();
    assert!(held <= 7 * trip, "the host held {held} rounds at g's join");
    assert_eq!(h.log[..t1].iter().map(|p| p[0]).collect::<Vec<_>>(), before.log[..t1].iter().map(|p| p[0]).collect::<Vec<_>>());
    let k_round = joins[1] as usize;
    let mut back = Vec::new();
    for (round, w) in host.windows(2).enumerate() {
        let ((t0, s0), (t1_, s1)) = (w[0], w[1]);
        assert!(t1_ + WINDOW >= t0 && s1 + WINDOW as u64 >= s0, "round {round}: the host went back from {t0} to {t1_}");
        if t1_ < t0 || s1 < s0 {
            back.push((round + 1, t0, t1_));
        }
    }
    assert!(back.len() <= 1 && back.iter().all(|b| b.0 >= k_round && b.0 < k_round + 40 && b.2 == t2), "the host stepped back only at k's join, to its tick: {back:?}");
    let (end_t, end_s) = *host.last().unwrap();
    assert!(end_t > 1000 && end_s > 1000, "the host rode on: tick {end_t}, its bike {end_s} ticks");

    // from each join on, the machines in the ride agree
    let n = r.agree(&[0, 1], t1);
    assert!(n > 700, "host and g agree on {n} ticks from {t1}");
    let n = r.agree(&[0, 1, 2], t2);
    assert!(n > 450, "everyone agrees on {n} ticks from {t2}");
    assert!(t2 > t1 + 250 && t2 < k_round + 40, "k joined at {t2}, up at round {k_round}");
    // a joiner's own seats ride its own pads (mashed a tick or so earlier: the input delay)
    for seat in [2, 3] {
        let t = t2 + 20;
        assert!((0..4).any(|d| k.log[t][seat] == wire(&mash(seat, t - d))), "seat {}: {:?}", seat + 1, k.log[t][seat]);
    }
    assert!(r.biggest <= DATAGRAM_MAX, "a {}-byte datagram", r.biggest);
}

/// A game that cannot change its seats ([`Tally`]) refuses the join, and the ride goes on as it was.
#[test]
fn a_ride_that_cannot_seat_a_joiner_says_so() {
    ggrs::clock::drive(true);
    let mut np: Netplay<Tally> = Netplay::new(Lineup::parse("host").unwrap(), "host", Timing::default(), dumps()).unwrap();
    let mut game = Tally::new(1);
    for t in 0..40 {
        if t == 20 {
            np.admit("g", &[1], Vec::new()).unwrap();
            np.heard("g", &[]);
            assert!(np.admit("g", &[2], Vec::new()).is_err(), "g twice");
            assert!(np.admit("k", &[0], Vec::new()).is_err(), "the host's seat");
        }
        let pad = [mash(0, np.tick())];
        game.frame(&mut np, &pad);
        np.outgoing();
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    assert!(np.said.iter().any(|s| s.contains("g cannot join: this ride's seats cannot change")), "{:?}", np.said);
    assert_eq!((np.lineup.text().as_str(), np.recover.restarts, np.admit.pending()), ("host", 0, 0));
    assert!(np.tick() > 30);
    let mut guest: Netplay<Tally> = Netplay::joining("host", "g", Timing::default(), dumps()).unwrap();
    assert!(guest.waiting() && guest.admit("k", &[1], Vec::new()).is_err(), "only the host takes a peer on");
}

/// N10's acceptance over the lobby's real connections, on the LAN (mDNS, no relay, no board, nothing
/// leaves the machine): a host opens its freeride of one ([`peer::Peer::open`], the pause's "Open to
/// Online"), rides alone, and two peers join it by its room code mid-ride, one after the other, each
/// through the lobby's handshake and into the session at a tick. From each join's tick every machine
/// in the ride agrees — the state's hash before every settled tick and the pads it ran — and the
/// host's ride never went back but for the settled tick at the second join.
#[test]
fn a_freeride_opened_on_the_lan_takes_two_peers_mid_ride() {
    use openmx_net::{Board, Cmd, Names, Net, Online, Pick, Relay, Room, HOST};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    let lan = |name: &str| Online { name: name.into(), relay: Relay::Off, board: Board::Off, lan: true, code_servers: Some(vec![]), ..Online::default() };
    let tmp = std::env::temp_dir().join(format!("openmx-n10-desyncs-{}", std::process::id()));
    let mut peers: Vec<peer::Peer> = ["Hosty", "Guesty", "Thirdy"].iter().map(|n| peer::Peer::new(Net::start(lan(n), Arc::new(Names)))).collect();
    for p in &mut peers {
        p.dumps = tmp.clone();
    }
    // every rider on a controller preset of their own (N13), which the room carries to every machine
    let on = |rider: &str, bike: &str, preset: u8| Pick { preset, ..Pick::new(rider, bike) };
    let mut room = Room::new("Hosty", "free06", 4);
    room.take(HOST, &[on("thq1", "race125p", 3)]);
    peers[0].open(room).unwrap();

    let frame = Duration::from_nanos(1_001_000_000_000 / 30_000);
    // ticks ridden after each join before the next step: it rides in real time, so this is the test's length
    const AFTER: usize = 150;
    let started = Instant::now();
    let mut last = Instant::now();
    // the host's tick after every frame, and when each guest asked to join
    let mut host: Vec<usize> = Vec::new();
    let mut asked = [false; 2];
    let code = loop {
        std::thread::sleep(frame.saturating_sub(last.elapsed()));
        let dt = last.elapsed().as_secs_f32().min(0.25);
        last = Instant::now();
        for (i, p) in peers.iter_mut().enumerate() {
            p.frame(&peer::mash, dt);
            for line in p.said.drain(..) {
                eprintln!("{i}: {line}");
            }
            assert!(p.failed.is_none(), "peer {i}: {:?} ({})", p.failed, p.status);
        }
        let h = peers[0].riding.as_ref().unwrap();
        host.push(h.np.tick());
        let tick = h.np.tick();
        let joins = h.np.admit.joins.len();
        let short = peers[0].room.as_ref().map(|r| r.short.clone()).unwrap_or_default();
        // g once the host has ridden two seconds alone and its code is out; k five seconds after g is in
        if !asked[0] && tick >= 60 && !short.is_empty() {
            assert!(peers[0].room.as_ref().unwrap().riding, "the room is the ride");
            peers[1].net.send(Cmd::Join(short.clone(), vec![on("thq2", "pro250a", 2)]));
            asked[0] = true;
        }
        if !asked[1] && joins == 1 && tick >= h.np.admit.joins[0].1 as usize + AFTER {
            peers[2].net.send(Cmd::Join(short.clone(), vec![on("thq3", "race125p", 1), on("thq4", "race125p", 0)]));
            asked[1] = true;
        }
        if joins == 2 && h.np.confirmed() > h.np.admit.joins[1].1 as i64 + AFTER as i64 && peers.iter().all(|p| p.riding.as_ref().is_some_and(|r| r.np.confirmed() > h.np.admit.joins[1].1 as i64 + AFTER as i64)) {
            break short;
        }
        if started.elapsed() > Duration::from_secs(150) {
            for (i, p) in peers.iter().enumerate() {
                let Some(r) = &p.riding else { continue };
                let np = &r.np;
                eprintln!("{i} {}: tick {} waiting {} restarts {} lineup {} notice {:?} round {} state {:?} sims {} too_big {}", np.me, np.tick(), np.waiting(), np.recover.restarts, np.lineup.text(), np.notice, np.rb.round, np.rb.sess.current_state(), r.sims.len(), p.too_big);
            }
            panic!("not done in 150 s: tick {tick}, {joins} joined; {:?}", peers.iter().map(|p| p.status.clone()).collect::<Vec<_>>());
        }
    };
    eprintln!("room {code}: done in {:.1} s", started.elapsed().as_secs_f32());

    let rides: Vec<&peer::Riding> = peers.iter().map(|p| p.riding.as_ref().unwrap()).collect();
    let (h, g, k) = (&rides[0].np, &rides[1].np, &rides[2].np);
    let joins: Vec<(&str, usize)> = h.admit.joins.iter().map(|(p, t)| (p.as_str(), *t as usize)).collect();
    assert_eq!((joins[0].0, joins[1].0), (g.me.as_str(), k.me.as_str()));
    let (t1, t2) = (joins[0].1, joins[1].1);
    for np in [h, g, k] {
        assert_eq!(np.lineup.seats.len(), 4, "{}", np.me);
        assert_eq!(np.lineup.seats.iter().flatten().filter(|p| **p == k.me).count(), 2, "k rides two seats");
        assert!(np.desyncs.is_empty() && np.end.is_none(), "{}: {:?} {:?}", np.me, np.desyncs, np.end);
    }
    // a drop-in rides the host's bike set: the room says so, as its handshake compared it
    let r = peers[0].room.as_ref().unwrap();
    assert!(r.seats.iter().flatten().all(|s| s.pick.bike == "race125p"), "{:?}", r.seats);
    assert_eq!(r.seats_of(&g.me), [1]);
    assert_eq!(r.seats_of(&k.me), [2, 3]);
    // and every machine reads each seat through its rider's controller preset (N13)
    for (i, ride) in rides.iter().enumerate() {
        let presets: Vec<Option<usize>> = ride.sims.iter().map(net::rides::preset).collect();
        assert_eq!(presets, [Some(3), Some(2), Some(1), Some(0)], "peer {i}");
    }

    // from each join on, the machines in the ride ran the same ticks
    let settled = |nps: &[&net::Netplay<rides::State>]| nps.iter().map(|np| np.confirmed().max(0) as usize).min().unwrap();
    for (who, from) in [(vec![h, g], t1), (vec![h, g, k], t2)] {
        let to = settled(&who);
        assert!(to > from + AFTER - 30, "{to} settled from {from}");
        let first = peer::ticks(who[0], from, to);
        for np in &who[1..] {
            let theirs = peer::ticks(np, from, to);
            if let Some((t, (a, b))) = first.lines().zip(theirs.lines()).enumerate().find(|(_, (a, b))| a != b) {
                panic!("tick {}: {} has {a}, {} has {b} (hash, pads)", from + t, who[0].me, np.me);
            }
            assert_eq!(first.lines().count(), theirs.lines().count());
        }
    }
    // the bikes move: the agreement is about something
    let states: std::collections::BTreeSet<u64> = h.hashes[t2..t2 + AFTER].iter().copied().collect();
    assert!(states.len() > AFTER / 2, "{} states", states.len());

    // the host's ride: forward all the way, but for the one step back to the settled tick at k's join
    let back: Vec<(usize, usize)> = host.windows(2).filter(|w| w[1] < w[0]).map(|w| (w[0], w[1])).collect();
    assert!(back.len() <= 1 && back.iter().all(|b| b.1 == t2 && b.0 - b.1 <= WINDOW), "the host went back: {back:?} (joins at {t1}, {t2})");
    assert!(host[0] <= 1 && *host.last().unwrap() > t2 + AFTER, "{:?}..{:?}", host.first(), host.last());
    assert_eq!((h.recover.restarts, g.recover.restarts, k.recover.restarts), (2, 2, 1));
}

// ---------------------------------------------------------------------------------------------------
// The put-back ghost online (P59)
// ---------------------------------------------------------------------------------------------------

use game_sim::ghost::Ghost;

/// The tick in every [`PUT_BACK_EVERY`] the host's seat asks for a put-back on.
const PUT_BACK_AT: usize = 60;
const PUT_BACK_EVERY: usize = 120;

/// The ride, with the host's seat pressing L1+R1 for one tick every four seconds, and what every
/// seat's ghost was after each tick it ran — written again when a rollback runs the tick again, so
/// what is left for a settled tick is what that machine drew from it.
struct Ghosted {
    ride: Ride,
    seen: Vec<Vec<Option<Ghost>>>,
}

impl Ghosted {
    fn new(seats: usize) -> Ghosted {
        Ghosted { ride: Ride::new(seats), seen: vec![Vec::new(); seats] }
    }
}

fn chord() -> Report {
    Report::from_buttons(Control::L1.mask() | Control::R1.mask())
}

impl Machine for Ghosted {
    type S = rides::State;
    fn frame(&mut self, np: &mut Netplay<rides::State>, local: &[Report]) {
        let mut local = local.to_vec();
        if np.tick() % PUT_BACK_EVERY == PUT_BACK_AT
            && let Some(i) = np.lineup.mine(&np.me).iter().position(|&s| s == 0)
        {
            local[i] = chord();
        }
        let seen = &mut self.seen;
        let mut after = |seat: usize, _: F, sim: &mut Sim, _: &game_sim::ride::Controls| {
            let t = sim.ticks as usize - 1;
            let seen = &mut seen[seat];
            if seen.len() <= t {
                seen.resize(t + 1, None);
            }
            seen[t] = Some(sim.rider.ghost);
        };
        let mut game = Rides { sims: &mut self.ride.sims, world: &mut self.ride.world, after: Some(&mut after), time_scale: None, plane: None };
        np.frame(&mut game, &local, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        self.ride.hash()
    }
}

/// P59: the host's L1+R1 puts its bike back, and on both machines that bike is a ghost from that
/// tick, at the same alpha on the same tick, over a link that loses and delays datagrams (so ticks
/// are guessed, rolled back and run again). Each machine's last run of every settled tick — the
/// state its picture was drawn from — has the same ghost on both; the settled hashes, which name
/// every seat's ghost, agree; and the ghost is the put-back's: on at half on its own tick, fading by
/// the tick's `dt`, solid on the 91st tick after, and never on the guest's bike.
#[test]
fn a_put_back_on_one_peer_is_a_ghost_on_both() {
    let link = Link { loss: 0.2, lag: 3, outage: 0..0, oneway: false };
    let r = run("host,g", Timing::default(), Ghosted::new, &link, 0..420, |_, _, _| {});
    for np in &r.nets {
        assert!(np.desyncs.is_empty() && !np.desynced, "{}: {:?}", np.me, np.desyncs);
        assert!(np.rb.reruns > 0, "{} never ran a tick again", np.me);
    }
    let n = r.agree(&[0, 1], 0);
    assert!(n > 300, "only {n} ticks settled");

    let (host, guest) = (&r.games[0].seen, &r.games[1].seen);
    let (mut put_backs, mut ghosted) = (Vec::new(), 0);
    for t in 0..n {
        for seat in 0..2 {
            let (a, b) = (host[seat][t].unwrap(), guest[seat][t].unwrap());
            assert_eq!(a, b, "tick {t}, seat {seat}: the host drew {a:?}, the guest {b:?}");
            assert_eq!(a.shown().to_bits(), b.shown().to_bits());
        }
        let g = host[0][t].unwrap();
        if g.on && g.alpha == Ghost::START && g.clock == Ghost::LENGTH {
            put_backs.push(t);
        }
        ghosted += g.on as usize;
        assert!(!host[1][t].unwrap().on, "tick {t}: the guest's bike is a ghost");
    }
    eprintln!("put back on ticks {put_backs:?}, {ghosted} ghost ticks of {n}");
    assert!(put_backs.len() >= 2, "put back on {put_backs:?}");
    // on the ticks the chord rode on (the session's input delay puts it a tick after it was pressed)
    let chords: Vec<usize> = (0..n).filter(|&t| unwire(r.nets[0].log[t][0]).0 == chord().0).collect();
    assert_eq!(put_backs, chords);
    for w in put_backs.windows(2) {
        assert_eq!(w[1] - w[0], PUT_BACK_EVERY, "put backs on {put_backs:?}");
    }

    // The fade, as a lone ghost steps it on the ride's own frame lengths: 3 s at 30 Hz, the
    // put-back's tick at half, then 90 more, the 91st solid.
    let first = put_backs[0];
    let mut g = Ghost::default();
    g.start();
    let mut fade = vec![g];
    while g.on {
        g.step(rides::tick_dt(first + fade.len()), || false);
        fade.push(g);
    }
    assert_eq!(fade.len(), 92);
    assert!(fade[1].alpha < fade[45].alpha && fade[45].alpha < fade[90].alpha && fade[90].alpha < F(1.0));
    for (k, want) in fade.iter().enumerate() {
        assert_eq!(host[0][first + k].unwrap(), *want, "{k} ticks after the put-back");
    }
    assert_eq!(ghosted, put_backs.iter().filter(|&&t| t + 91 <= n).count() * 91 + put_backs.iter().filter(|&&t| t + 91 > n).map(|&t| n - t).sum::<usize>());
}

/// P59: the online checksum names the ghost, though a one-player [`Sim`]'s hash does not: two rides
/// alike but for seat 0's ghost hash apart, and the rides' bytes carry it over to a peer mid-fade.
#[test]
fn the_online_hash_and_bytes_carry_the_ghost() {
    let mut there = Ride::new(2);
    for t in 0..100 {
        let pads = [if t == 70 { chord() } else { mash(0, t) }, mash(1, t)];
        Rides { sims: &mut there.sims, world: &mut there.world, after: None, time_scale: None, plane: None }.step(t, &pads);
    }
    assert!(there.sims[0].rider.ghost.on && !there.sims[1].rider.ghost.on);
    let mut solid = Ride { sims: there.sims.clone(), world: Flat::default(), glitch: false };
    solid.sims[0].rider.ghost = Ghost::default();
    assert_eq!(solid.sims[0].hash(), there.sims[0].hash(), "a one-player sim's hash has no ghost in it");
    assert_ne!(solid.hash(), there.hash(), "the online hash has");

    let bytes = Rides { sims: &mut there.sims, world: &mut there.world, after: None, time_scale: None, plane: None }.encode().unwrap();
    let mut here = Ride::new(2);
    Rides { sims: &mut here.sims, world: &mut here.world, after: None, time_scale: None, plane: None }.decode(&bytes).unwrap();
    assert_eq!(here.sims[0].rider.ghost, there.sims[0].rider.ghost);
    assert_eq!(here.hash(), there.hash());
}

// ---------------------------------------------------------------------------------------------------
// Sixteen riders (N12)
// ---------------------------------------------------------------------------------------------------

/// A machine whose frames are timed: the wall clock each took, session and ride together.
struct Timed<M: Machine> {
    m: M,
    spent: Vec<std::time::Duration>,
}

impl<M: Machine> Machine for Timed<M> {
    type S = M::S;
    fn frame(&mut self, np: &mut Netplay<M::S>, local: &[Report]) {
        let t0 = std::time::Instant::now();
        self.m.frame(np, local);
        self.spent.push(t0.elapsed());
    }
    fn hash(&self) -> u64 {
        self.m.hash()
    }
}

/// `peers` machines with `seats` seats between them, dealt round the table (seat `s` is machine
/// `s % peers`'s), as a line-up.
fn dealt(peers: usize, seats: usize) -> String {
    let name = |p: usize| if p == 0 { "host".to_string() } else { format!("g{p}") };
    (0..seats).map(|s| name(s % peers)).collect::<Vec<_>>().join(",")
}

/// `lineup` ridden `rounds` rounds over `link`, every machine's frames timed, and what it cost said:
/// each machine's mean and worst frame, its re-runs, the datagrams and bytes it sent a tick, the
/// ride's state in bytes (what a resync or a restart sends) and the time ggrs' save of it takes.
fn measured(lineup: &str, link: &Link, rounds: u32) -> Run<Timed<Ride>> {
    let r = run(lineup, Timing::default(), |n| Timed { m: Ride::new(n), spent: Vec::new() }, link, 0..rounds, |_, _, _| {});
    let seats = r.nets[0].lineup.seats.len();
    for (i, np) in r.nets.iter().enumerate() {
        let spent = &r.games[i].spent;
        let mean = spent.iter().sum::<std::time::Duration>() / spent.len() as u32;
        let worst = spent.iter().max().unwrap();
        let (n, bytes) = r.sent[i];
        eprintln!(
            "{seats} seats, {} machines: {}: frame mean {:.2} ms, worst {:.2} ms; {} re-run, {} held; sent {:.1} datagrams, {:.0} bytes a tick ({:.1} kB/s)",
            r.names.len(),
            np.me,
            mean.as_secs_f64() * 1e3,
            worst.as_secs_f64() * 1e3,
            np.rb.reruns,
            np.rb.held,
            n as f64 / rounds as f64,
            bytes as f64 / rounds as f64,
            bytes as f64 / rounds as f64 * 30.0 / 1e3,
        );
    }
    let mut sims = r.games[0].m.sims.clone();
    let mut world = Flat::default();
    let game = Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None };
    let t0 = std::time::Instant::now();
    let saves = 100;
    for _ in 0..saves {
        std::hint::black_box(game.save());
    }
    eprintln!(
        "{seats} seats: the state is {} bytes; ggrs' save of it takes {:.1} us; the biggest datagram {} bytes",
        game.encode().unwrap().len(),
        t0.elapsed().as_secs_f64() * 1e6 / saves as f64,
        r.biggest
    );
    r
}

/// N12's acceptance: sixteen riders over four machines on a link that loses 5 % and holds the rest
/// 100 ms ride a few hundred ticks with every settled tick the same pads and state everywhere, and
/// ggrs' checksums (every 30th tick) never differ.
#[test]
fn sixteen_riders_on_four_peers() {
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    let r = measured(&dealt(4, 16), &link, 360);
    for np in &r.nets {
        assert_eq!(np.lineup.seats.len(), 16);
        assert_eq!(np.lineup.mine(&np.me).len(), 4, "{}", np.me);
        assert!(np.desyncs.is_empty() && !np.desynced, "{}: {:?}", np.me, np.desyncs);
        assert_eq!(np.end, None, "{}", np.me);
    }
    assert!(r.biggest <= DATAGRAM_MAX, "a {}-byte datagram", r.biggest);
    let n = r.agree(&[0, 1, 2, 3], 0);
    assert!(n > 300, "only {n} ticks settled");
    assert!(r.nets.iter().all(|np| np.rb.reruns > 0), "the guesses were put right somewhere");
    // and the bikes move: sixteen of them, each its own ride
    let at: std::collections::BTreeSet<u64> = r.games[0].m.sims.iter().map(|s| s.hash()).collect();
    assert_eq!(at.len(), 16, "every seat a state of its own");
}

/// The measurement N12 asks for: sixteen machines, one rider each, the same link, and then the cap,
/// every seat its own machine. Numbers only (the journal has them): `--ignored` to run.
#[test]
#[ignore]
fn sixteen_machines_measured() {
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    for (peers, seats) in [(4, 8), (4, 16), (16, 16), (net::SEATS, net::SEATS)] {
        let r = measured(&dealt(peers, seats), &link, 300);
        let n = r.agree(&(0..peers).collect::<Vec<_>>(), 0);
        assert!(r.nets.iter().all(|np| np.desyncs.is_empty()));
        eprintln!("{seats} seats on {peers} machines: {n} ticks agreed");
    }
}

/// N15: the `--peer` UDP path has no room or handshake, so each machine's datagrams carry its own
/// seats' controller presets ahead of the session's bytes ([`net::udp`]'s card). Three machines on
/// localhost's real UDP, each with its seats built on preset 0 the way `--online` starts them, the
/// host on 3, g on 2 and k's two seats on 1 and 0: every machine builds every seat on its own
/// machine's preset before the first tick (ggrs ticks only once it has heard every peer), and from
/// then on every machine agrees on every settled tick.
#[test]
fn every_seat_rides_its_own_preset_over_peer_udp() {
    use net::udp::Udp;

    let lineup = Lineup::parse("host,g,k,k").unwrap();
    let names = lineup.peers();
    let cards: [&[u8]; 3] = [&[3], &[2], &[1, 0]];
    let local: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let mut udps: Vec<Udp> = cards.iter().map(|c| Udp::bind_as(local, vec![], "n15", c).unwrap()).collect();
    let ats: Vec<_> = udps.iter().map(|u| u.at()).collect();
    for (i, u) in udps.iter_mut().enumerate() {
        u.meet((0..3).filter(|&j| j != i).map(|j| (names[j].clone(), ats[j])).collect());
    }
    ggrs::clock::drive(true);
    let mut nets: Vec<Netplay<rides::State>> = names.iter().map(|me| Netplay::new(lineup.clone(), me, Timing::default(), dumps()).unwrap()).collect();
    let mut games: Vec<Ride> = (0..3).map(|_| Ride::new(4)).collect();
    let mut owed = vec![true; 3];
    for _ in 0..240 {
        for i in 0..3 {
            for (from, d) in udps[i].receive().unwrap() {
                nets[i].heard(&from, &d);
            }
            // what `Online::frame` does before the session runs
            if owed[i] && let Some(presets) = udps[i].presets(&lineup, &names[i]).unwrap() {
                for (sim, &p) in games[i].sims.iter_mut().zip(&presets) {
                    sim.set_presets([p as usize, 0]);
                }
                owed[i] = false;
            }
            let np = &mut nets[i];
            let local: Vec<Report> = np.lineup.mine(&np.me).into_iter().map(|s| mash(s, np.tick())).collect();
            games[i].frame(np, &local);
            assert!(!owed[i] || np.tick() == 0, "{} ticked before every preset came", names[i]);
            for (to, d) in np.outgoing() {
                udps[i].send(&to, &d);
            }
            np.said.clear();
        }
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    for (i, game) in games.iter().enumerate() {
        let presets: Vec<Option<usize>> = game.sims.iter().map(rides::preset).collect();
        assert_eq!(presets, [Some(3), Some(2), Some(1), Some(0)], "{}: every seat on its machine's preset", names[i]);
    }
    let settled = nets.iter().map(|np| (np.confirmed().max(0) as usize).min(np.hashes.len())).min().unwrap();
    assert!(settled > 150, "{settled} ticks settled");
    for np in &nets[1..] {
        assert_eq!(np.hashes[..settled], nets[0].hashes[..settled], "{}: the state before every settled tick", np.me);
        assert_eq!(np.log[..settled], nets[0].log[..settled], "{}: the pads", np.me);
    }
    assert!(nets.iter().all(|np| np.desyncs.is_empty() && np.end.is_none()));
}

// ---------------------------------------------------------------------------------------------------
// The host changes the modifiers mid-ride (P100)
// ---------------------------------------------------------------------------------------------------

/// The ride with its modifiers, as the shell's `Online` steps it: [`rides::Modded`] over [`Rides`],
/// each seat's stock bike file beside it.
struct ModRide {
    ride: Ride,
    stock: Vec<game_sim::phy::Phy>,
    mods: game_sim::modifiers::Mods,
}

impl ModRide {
    fn new(seats: usize) -> ModRide {
        let ride = Ride::new(seats);
        let stock = ride.sims.iter().map(|s| (*s.phy).clone()).collect();
        ModRide { ride, stock, mods: Default::default() }
    }
}

impl Machine for ModRide {
    type S = rides::State;
    fn frame(&mut self, np: &mut Netplay<rides::State>, local: &[Report]) {
        let time_scale = self.mods.time_scale();
        let mut rides = Rides { sims: &mut self.ride.sims, world: &mut self.ride.world, after: None, time_scale, plane: None };
        let mut game = rides::Modded { rides: &mut rides, stock: &self.stock, mods: &mut self.mods, stock_plane: None };
        np.frame(&mut game, local, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        self.ride.hash()
    }
}

/// P100's acceptance: two peers ride, and the host changes gravity mid-ride (the pause's change,
/// [`Netplay::remodify`]) over a lossy, laggy link. Both machines take it at the same tick, with
/// the same state before every settled tick and the same pads from the first tick to the last; the
/// tick it took is the ride the same pads ride offline with the change put on there (the settled
/// state moved onto the new set, as the pause does it offline, P98), and a ride never changed parts
/// from it there. A guest cannot change them, and the host's ride never steps back past the settled
/// tick.
#[test]
fn the_host_changes_gravity_mid_ride_and_both_peers_stay_in_sync() {
    use game_sim::modifiers::Mods;
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    let half = Mods::parse("gravity=50").unwrap();
    let change = 300;
    let mut refused = None;
    let r = run("host,g", Timing::default(), ModRide::new, &link, 0..900, |round, r, _| {
        if round == change {
            refused = Some(r.nets[1].remodify(half.text()));
            r.nets[0].remodify(half.text()).unwrap();
        }
    });
    let plain = run("host,g", Timing::default(), ModRide::new, &link, 0..900, |_, _, _| {});
    assert!(refused.unwrap().is_err(), "a guest changed the modifiers");

    // one change, on both machines at the same tick, a settled one near the round it was asked at
    let (h, g) = (&r.nets[0], &r.nets[1]);
    assert_eq!(h.remods.len(), 1, "{:?}", h.remods);
    assert_eq!(h.remods, g.remods, "both took the change at one tick");
    let t = h.remods[0].0 as usize;
    assert!(t + WINDOW > change as usize && t < change as usize + 40, "the change at tick {t}, asked at round {change}");
    for (i, np) in r.nets.iter().enumerate() {
        assert_eq!((np.mods.as_str(), np.rb.round, np.end.as_deref()), ("gravity=50", 1, None), "{}", np.me);
        assert!(np.desyncs.is_empty() && !np.desynced, "{}: {:?}", np.me, np.desyncs);
        assert_eq!(r.games[i].mods, half, "{}", np.me);
        for (s, sim) in r.games[i].ride.sims.iter().enumerate() {
            let mut want = r.games[i].stock[s].clone();
            half.bike(0, &mut want);
            assert_eq!(format!("{:?}", *sim.phy), format!("{want:?}"), "{} seat {}: the bike on half gravity", np.me, s + 1);
        }
    }

    // in sync from the first tick to the last settled one, across the change
    let n = r.agree(&[0, 1], 0);
    assert!(n > 700 && n > t + 400, "host and g agree on {n} ticks, the change at {t}");

    // the same pads offline, the change put on before tick t: the same hash before every tick
    let mut sims = sims(2);
    let mut world = Flat::default();
    let stock: Vec<_> = sims.iter().map(|s| (*s.phy).clone()).collect();
    for tick in 0..n {
        if tick == t {
            for (s, sim) in sims.iter_mut().enumerate() {
                half.resim(&Mods::original(), 0, &stock[s], sim);
            }
        }
        let mut game = Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None };
        assert_eq!(game.hash(), h.hashes[tick], "offline, the state before tick {tick} (the change at {t})");
        let pads: Vec<Report> = h.log[tick].iter().map(|&p| unwire(p)).collect();
        game.step(tick, &pads);
    }

    // a ride never changed is this one up to the change and another after it
    let same = plain.nets[0].hashes.len().min(n);
    let parted = (0..same).find(|&k| plain.nets[0].hashes[k] != h.hashes[k] || plain.nets[0].log[k] != h.log[k]);
    let parted = parted.expect("the change made no difference to the ride");
    assert!(parted >= t, "the rides parted at {parted}, before the change at {t}");
    let hashes_parted = (0..same).find(|&k| plain.nets[0].hashes[k] != h.hashes[k]).unwrap();
    assert!(hashes_parted > t && hashes_parted <= t + 30, "half gravity first shows in the state at {hashes_parted}, the change at {t}");

    // the host's ride never went back past the settled tick
    for (round, w) in r.ran[0].windows(2).enumerate() {
        assert!(w[1] + WINDOW >= w[0], "round {round}: the host went back from {} to {}", w[0], w[1]);
    }
}

/// P134's acceptance online: a guest changes its own seat's modifiers mid-ride (its pause's change,
/// [`Netplay::remodify_own`]) over a lossy, laggy link: the host takes it as its own change, and both
/// machines take it at the same tick, the guest's seat alone on it — its bike faster, the host's
/// seat and the world's as they were, though the guest's own set has a gravity too — and the two
/// rides stay in sync across it. The host's own change of its seat goes the same way after.
#[test]
fn a_guests_own_change_both_peers_take_at_the_same_tick() {
    use game_sim::modifiers::{Knob, Mods, online};
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    // the guest's own: its bike speed, and a gravity that is not its to set
    let mut mine = Mods::original();
    (0..8).for_each(|s| mine.set(s, Knob::BikeSpeed, 200));
    mine.set(0, Knob::Gravity, 50);
    let (asked, host_asked) = (300, 600);
    let r = run("host,g", Timing::default(), ModRide::new, &link, 0..1000, |round, r, _| {
        if round == asked {
            r.nets[1].remodify_own(online::own_set(&mine, 0, false).text()).unwrap();
        }
        if round == host_asked {
            let mut grip = Mods::original();
            grip.set(0, Knob::Grip, 150);
            r.nets[0].remodify_own(online::own_set(&grip, 0, true).text()).unwrap();
        }
    });
    let (h, g) = (&r.nets[0], &r.nets[1]);
    assert_eq!(h.remods.len(), 2, "{:?}", h.remods);
    assert_eq!(h.remods, g.remods, "both took each change at one tick");
    let t = h.remods[0].0 as usize;
    assert!(t + WINDOW > asked as usize && t < asked as usize + 60, "the guest's change at tick {t}, asked at round {asked}");
    let mut want = Mods::original();
    want.set(1, Knob::BikeSpeed, 200);
    assert_eq!(Mods::parse(&h.remods[0].1).unwrap(), want, "the guest's seat alone, and not the world");
    want.set(0, Knob::Grip, 150);
    for (i, np) in r.nets.iter().enumerate() {
        assert!(np.desyncs.is_empty() && !np.desynced, "{}: {:?}", np.me, np.desyncs);
        assert_eq!(r.games[i].mods, want, "{}", np.me);
        for (s, sim) in r.games[i].ride.sims.iter().enumerate() {
            let mut bike = r.games[i].stock[s].clone();
            want.bike(s, &mut bike);
            assert_eq!(format!("{:?}", *sim.phy), format!("{bike:?}"), "{} seat {}: the bike on its own set", np.me, s + 1);
        }
        let (a, b) = (&r.games[i].ride.sims[0].phy, &r.games[i].ride.sims[1].phy);
        assert_ne!(format!("{a:?}"), format!("{b:?}"), "{}: the two seats ride one bike", np.me);
    }
    let n = r.agree(&[0, 1], 0);
    assert!(n > h.remods[1].0 as usize + 200, "host and g agree on {n} ticks, the changes at {:?}", h.remods);
}

/// A peer that joins after the host changed the modifiers (P100) loaded the room's older set; the
/// join's restart carries the set the ride is on, and the joiner's seats ride it from the join tick.
/// The host opened its ride of one, changed gravity alone at round 100, and `g` drops in at 300.
#[test]
fn a_peer_that_joins_after_a_change_rides_the_changed_set() {
    use game_sim::modifiers::Mods;
    ggrs::clock::drive(true);
    let half = Mods::parse("gravity=50").unwrap();
    let spawn = rides::seat_bytes(&sims(1)[0]);
    let mut nets = vec![Netplay::new(Lineup::parse("host").unwrap(), "host", Timing::default(), dumps()).unwrap()];
    let mut games = vec![ModRide::new(1)];
    let names = ["host", "g"];
    let mut wire: Vec<(usize, u32, String, Vec<u8>)> = Vec::new();
    for round in 0..700u32 {
        if round == 100 {
            nets[0].remodify(half.text()).unwrap();
        }
        if round == 270 {
            nets[0].admit("g", &[1], rides::spawn_on(&spawn, &[])).unwrap();
        }
        if round == 300 {
            nets.push(Netplay::joining("host", "g", Timing::default(), dumps()).unwrap());
            games.push(ModRide::new(1));
        }
        for (to, at, from, d) in std::mem::take(&mut wire) {
            if at > round {
                wire.push((to, at, from, d));
            } else {
                nets[to].heard(&from, &d);
            }
        }
        let up = nets.len();
        for i in 0..up {
            let np = &mut nets[i];
            let local: Vec<Report> = np.lineup.mine(&np.me).into_iter().map(|s| mash(s, np.tick())).collect();
            games[i].frame(np, &local);
            for (to, d) in np.outgoing() {
                if let Some(j) = names.iter().position(|n| *n == to).filter(|&j| j < up) {
                    wire.push((j, round + 2, names[i].to_string(), d));
                }
            }
            np.said.clear();
        }
        ggrs::clock::advance(ROUND);
    }
    ggrs::clock::drive(false);
    let (h, g) = (&nets[0], &nets[1]);
    let changed = h.remods[0].0;
    assert!(changed >= 100 && changed < 110, "the host alone took the change at once: tick {changed}");
    let joined = h.admit.joins[0].1;
    assert_eq!(g.remods, [(joined, "gravity=50".to_string())], "g took the ride's set at its join");
    assert_eq!((h.lineup.text().as_str(), g.lineup.text().as_str()), ("host,g", "host,g"));
    for (i, game) in games.iter().enumerate() {
        assert_eq!(game.mods, half, "{}", names[i]);
        for sim in &game.ride.sims {
            let mut want = game.stock[0].clone();
            half.bike(0, &mut want);
            assert_eq!(format!("{:?}", *sim.phy), format!("{want:?}"), "{}: every seat on half gravity", names[i]);
        }
    }
    let settled = (h.confirmed().min(g.confirmed()).max(0)) as usize;
    assert!(settled > joined as usize + 300, "{settled} settled, joined at {joined}");
    for t in joined as usize..settled {
        assert_eq!((h.hashes[t], &h.log[t]), (g.hashes[t], &g.log[t]), "tick {t}, joined at {joined}");
    }
    assert!(h.desyncs.is_empty() && g.desyncs.is_empty());
}

// ---------------------------------------------------------------------------------------------------
// A seat switches machine online (N17)
// ---------------------------------------------------------------------------------------------------

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

/// The machine seat `seat` wants at `tick`: seat 1 the plane from 120 to 450, seat 2 from 300 to 600,
/// the bike otherwise (a level, not an edge: what the shell's pad says every tick).
fn wanted(seat: usize, tick: usize) -> vehicle::Wanted {
    let (from, to) = [(120, 450), (300, 600)][seat % 2];
    let up = if (from..to).contains(&tick) { vehicle::Kind::Biplane } else { vehicle::Kind::Bike };
    vehicle::Wanted { to: Some(up), every_bank: seat == 1, round_stick: false, dpad_rides: false, together: false, paused: false }
}

/// The ride with the plane every seat may take, each seat's pad saying the machine it wants.
struct Switching {
    ride: Ride,
    plane: game_sim::flight::Flight,
    /// The ticks this machine stepped (not rolled back) with each seat flying.
    flown: Vec<usize>,
}

impl Machine for Switching {
    type S = rides::State;
    fn frame(&mut self, np: &mut Netplay<rides::State>, local: &[Report]) {
        let seats = np.lineup.mine(&np.me);
        let mut pads = local.to_vec();
        for (pad, &s) in pads.iter_mut().zip(&seats) {
            pad.0[net::SWITCH] = wanted(s, np.tick()).byte();
        }
        let mut game = Rides {
            sims: &mut self.ride.sims,
            world: &mut self.ride.world,
            after: None,
            time_scale: None,
            plane: Some(&mut self.plane),
        };
        np.frame(&mut game, &pads, ROUND.as_secs_f32());
        for (s, sim) in self.ride.sims.iter().enumerate() {
            self.flown[s] += sim.plane.is_some() as usize;
        }
    }
    fn hash(&self) -> u64 {
        self.ride.hash()
    }
}

#[test]
fn a_wanted_machine_goes_in_one_byte() {
    use vehicle::{Kind, Wanted};
    for to in [None, Some(Kind::Bike), Some(Kind::Biplane)] {
        for (every_bank, round_stick) in [(false, false), (true, false), (false, true), (true, true)] {
            for (dpad_rides, together) in [(false, false), (true, false), (false, true), (true, true)] {
                for paused in [false, true] {
                    let w = Wanted { to, every_bank, round_stick, dpad_rides, together, paused };
                    assert_eq!(Wanted::of(w.byte()), w);
                }
            }
        }
    }
    assert_eq!(Wanted::of(0), Wanted::default(), "a pad that says nothing switches nothing");
    // the switch byte is on the wire, and the game's own bytes never reach it
    assert!(net::SWITCH < net::WIRE);
    let r = mash(0, 3);
    assert_eq!(r.0[net::SWITCH], 0);
}

/// N17's acceptance: two peers ride on a lossy, laggy link, and each one's seat takes the plane and
/// later the bike back, at ticks of its own (the host's flight overlaps g's). The switch is a byte of
/// the seat's pad, so it is a rollback input like the throttle: both machines take every switch at
/// the same tick, with the same state before every settled tick and the same pads; the same pads
/// ridden offline (no network) are the same state tick for tick, the plane comes up where the bike
/// was, and a flying seat's state goes through the codec (a resync, a join) unchanged.
#[test]
fn two_peers_take_the_plane_and_the_bike_back_and_stay_in_sync() {
    let Some(file) = the_discs_plane() else { return };
    let template = game_sim::flight::Flight::new(&file);
    let make = |seats: usize| Switching { ride: Ride::new(seats), plane: template.clone(), flown: vec![0; seats] };
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    let r = run("host,g", Timing::default(), make, &link, 0..900, |_, _, _| {});
    let h = &r.nets[0];
    for np in &r.nets {
        assert!(np.desyncs.is_empty() && !np.desynced && np.end.is_none(), "{}: {:?}", np.me, np.desyncs);
    }
    for game in &r.games {
        assert!(game.flown[0] > 250 && game.flown[1] > 250, "the seats flew {:?} ticks", game.flown);
    }
    let n = r.agree(&[0, 1], 0);
    assert!(n > 700, "host and g agree on {n} ticks");

    // offline, the same pads: the same state before every tick, and each switch where it was asked
    let mut seats = sims(2);
    let mut world = Flat::default();
    let mut plane = template.clone();
    let mut flying = [Vec::new(), Vec::new()];
    let mut coded = 0;
    // the shell's garages following the seats, as it draws them (this one parked no plane)
    let mut garages = [0, 1].map(|_| vehicle::Garage::new(vehicle::Kind::Bike, None).unwrap());
    let mut took = [Vec::new(), Vec::new()];
    for tick in 0..n {
        let before: Vec<Vf> = seats.iter().map(|s| s.rider.ride.position()).collect();
        let mut game = Rides { sims: &mut seats, world: &mut world, after: None, time_scale: None, plane: Some(&mut plane) };
        assert_eq!(game.hash(), h.hashes[tick], "offline, the state before tick {tick}");
        if tick % 50 == 0 {
            // a seat's state in bytes and back (the resync's and the join's way) is the same state
            let bytes = game.encode().expect("the ride says its state in bytes");
            let mut copy = sims(2);
            let mut world2 = Flat::default();
            let mut plane2 = template.clone();
            let mut other = Rides { sims: &mut copy, world: &mut world2, after: None, time_scale: None, plane: Some(&mut plane2) };
            other.decode(&bytes).unwrap();
            assert_eq!(other.hash(), h.hashes[tick], "tick {tick} through the codec");
            coded += game.sims.iter().filter(|s| s.plane.is_some()).count();
        }
        let pads: Vec<Report> = h.log[tick].iter().map(|&p| unwire(p)).collect();
        game.step(tick, &pads);
        for s in 0..2 {
            let up = seats[s].plane.is_some();
            // the machine up after tick t is the one tick t's pad asked for
            let asked = vehicle::Wanted::of(pads[s].0[net::SWITCH]).to;
            assert_eq!(up, asked == Some(vehicle::Kind::Biplane), "seat {} at tick {tick}, its pad asked for {asked:?}", s + 1);
            if up && flying[s].last().is_none_or(|&(_, t)| t + 1 != tick) {
                // taken this tick: the plane is put down where the bike was
                let at = seats[s].plane.as_ref().unwrap().spawn.0;
                assert_eq!((at.x(), at.z()), (before[s].x(), before[s].z()), "seat {} took the plane at {tick}", s + 1);
            }
            if let Some(up) = garages[s].follow(&seats[s]) {
                took[s].push((tick, up));
            }
            let shown = garages[s].plane_shown().map(|p| (p.at.x(), p.at.y(), p.at.z()));
            let flown = seats[s].plane.as_ref().map(|f| f.pose().1).map(|o| (o.x(), o.y(), o.z()));
            assert_eq!(shown, flown, "seat {}'s plane drawn where it flies, tick {tick}", s + 1);
            assert_eq!(garages[s].bike_shown(), !up);
            if up {
                match flying[s].last_mut() {
                    Some((_, t)) if *t + 1 == tick => *t = tick,
                    _ => flying[s].push((tick, tick)),
                }
            }
        }
    }
    assert!(coded >= 8, "{coded} flying seats went through the codec");
    // each seat flew once, as long as its pad asked (the shell's pad lands a tick on: the ride's
    // input delay)
    for s in 0..2 {
        let (from, to) = [(120, 450), (300, 600)][s];
        assert_eq!(flying[s], [(from + 1, to)], "seat {}'s flight", s + 1);
        let (b, p) = (vehicle::Kind::Bike, vehicle::Kind::Biplane);
        assert_eq!(took[s], [(from + 1, p), (to + 1, b)], "seat {}'s garage", s + 1);
    }
}

// ---------------------------------------------------------------------------------------------------
// A seat starts the ride on the plane (N18)
// ---------------------------------------------------------------------------------------------------

/// Seat `seat`'s bay in [`Starting`]'s lot: on the ground of the flat world, the plane's wheels on
/// it, a row of them along x.
fn bay(seat: usize, template: &game_sim::flight::Flight) -> vehicle::Spawn {
    let spawn = vehicle::Spawn { at: Vf::new(40.0 * seat as f32, 0.0, -30.0, 1.0), facing: Vf::new(0.0, 0.0, 1.0, 0.0) };
    let rest = vehicle::Plane::flying(spawn, template.clone()).rest_height(0.0);
    vehicle::Spawn { at: Vf::new(spawn.at.x(), rest, spawn.at.z(), 1.0), ..spawn }
}

/// g's seat (2) asks for the bike from this tick on; nobody asks for anything before.
const BACK_ON_THE_BIKE: usize = 300;

/// The ride with seat 2 starting in the plane, as the shell's `--online --vehicle biplane` or a
/// lobby pick of the plane has it (`Online::start_planes`): every machine puts it in its bay before
/// the first tick.
struct Starting {
    ride: Ride,
    plane: game_sim::flight::Flight,
    flown: Vec<usize>,
}

impl Starting {
    fn new(seats: usize, template: &game_sim::flight::Flight) -> Starting {
        let mut ride = Ride::new(seats);
        vehicle::start_lane(template, bay(1, template), &mut ride.sims[1]);
        Starting { ride, plane: template.clone(), flown: vec![0; seats] }
    }
}

impl Machine for Starting {
    type S = rides::State;
    fn frame(&mut self, np: &mut Netplay<rides::State>, local: &[Report]) {
        let seats = np.lineup.mine(&np.me);
        let mut pads = local.to_vec();
        for (pad, &s) in pads.iter_mut().zip(&seats) {
            let to = (s == 1 && np.tick() >= BACK_ON_THE_BIKE).then_some(vehicle::Kind::Bike);
            pad.0[net::SWITCH] = vehicle::Wanted { to, ..Default::default() }.byte();
        }
        let mut game = Rides { sims: &mut self.ride.sims, world: &mut self.ride.world, after: None, time_scale: None, plane: Some(&mut self.plane) };
        np.frame(&mut game, &pads, ROUND.as_secs_f32());
        for (s, sim) in self.ride.sims.iter().enumerate() {
            self.flown[s] += sim.plane.is_some() as usize;
        }
    }
    fn hash(&self) -> u64 {
        self.ride.hash()
    }
}

/// N18's acceptance: g's seat starts the ride in the plane, standing in its bay, on both machines
/// before the first tick, and the host's on its bike. Over a lossy, laggy link both machines hold
/// the same state before every settled tick from tick 0 (the plane in it); the same pads ridden
/// offline are the same state; the flying seat's flight is tick for tick the one the offline shell
/// flies for a player whose setup picked the plane (a [`vehicle::Garage`] started on it, the plane
/// in its bay, stepped as `Seat::pass` steps it); and the seat takes its bike back when it asks.
#[test]
fn a_seat_starts_the_ride_in_the_plane_on_every_machine() {
    let Some(file) = the_discs_plane() else { return };
    let template = game_sim::flight::Flight::new(&file);
    let link = Link { loss: 0.05, lag: 3, outage: 0..0, oneway: false };
    let r = run("host,g", Timing::default(), |seats| Starting::new(seats, &template), &link, 0..700, |_, _, _| {});
    let h = &r.nets[0];
    for np in &r.nets {
        assert!(np.desyncs.is_empty() && !np.desynced && np.end.is_none(), "{}: {:?}", np.me, np.desyncs);
    }
    for game in &r.games {
        assert_eq!(game.flown[0], 0, "the host's seat stayed on its bike");
        assert!(game.flown[1] > BACK_ON_THE_BIKE, "g's seat flew {} ticks", game.flown[1]);
    }
    let n = r.agree(&[0, 1], 0);
    assert!(n > 500, "host and g agree on {n} ticks");

    // offline: the same start, the same pads
    let mut seats = Starting::new(2, &template).ride.sims;
    assert!(seats[1].plane.is_some() && seats[0].plane.is_none());
    let at = seats[1].plane.as_ref().unwrap().spawn;
    let want = bay(1, &template);
    assert_eq!((at.0, at.1), (want.at, want.facing), "g's plane stands in its bay");
    let mut world = Flat::default();
    let mut plane = template.clone();
    // the offline shell's seat for a player who picked the plane: its own bike, its plane in its bay
    let mut shell = Ride::new(2).sims.remove(1);
    let mut garage = vehicle::Garage::new(vehicle::Kind::Biplane, Some(vehicle::Plane::flying(want, template.clone()))).unwrap();
    let mut shell_world = Flat::default();
    let mut flew = 0;
    let mut back = None;
    for tick in 0..n {
        let mut game = Rides { sims: &mut seats, world: &mut world, after: None, time_scale: None, plane: Some(&mut plane) };
        assert_eq!(game.hash(), h.hashes[tick], "offline, the state before tick {tick}");
        let pads: Vec<Report> = h.log[tick].iter().map(|&p| unwire(p)).collect();
        game.step(tick, &pads);
        match &seats[1].plane {
            Some(online) if vehicle::Wanted::of(pads[1].0[net::SWITCH]).to.is_none() => {
                let mut reports = [Report::missing(); game_sim::input::PADS];
                reports[0] = pads[1];
                let dt = rides::tick_dt(tick);
                let controls = shell.poll(dt, &reports).stick_alone();
                garage.step(&mut shell, dt, &controls, &mut shell_world);
                let offline = garage.plane.as_ref().and_then(|p| p.flight.as_ref()).unwrap();
                assert_eq!(online, offline, "g's flight online and the offline shell's, tick {tick}");
                flew += 1;
            }
            Some(_) => {}
            None => {
                back.get_or_insert(tick);
            }
        }
    }
    assert_eq!(flew, BACK_ON_THE_BIKE + 1, "flown alike from tick 0 to the bike's pad");
    assert_eq!(back, Some(BACK_ON_THE_BIKE + 1), "g's seat back on its bike a tick after it asked (the input delay)");
}

/// N19: a joiner's seat that picked the plane comes in through [`rides::spawn_flying`] and
/// [`Game::seat`] as N18's start has it — the plane in the seat's own bay, its flight `==` the one
/// [`vehicle::start_lane`] puts there (which N18 holds to the offline shell's start tick for tick) —
/// on its own controller preset, with the joiner's saved markers kept and the ride's other seats
/// untouched; a seat that picked the bike comes in on it.
#[test]
fn a_joiner_that_picked_the_plane_is_seated_flying_from_its_bay() {
    let Some(file) = the_discs_plane() else { return };
    let mut template = game_sim::flight::Flight::new(&file);
    let first = &sims(1)[0];
    let spawn = rides::seat_bytes(first);
    let mut marks = game_sim::markers::Markers::default();
    marks.slots[0] = Some(Box::new(game_sim::markers::Mark::Bike(first.rider.ride.clone())));
    let marked = rides::spawn_marked(first, &spawn, marks).unwrap();
    let flying = rides::spawn_flying(first, &marked, &template, bay(2, &template)).unwrap();
    let mut seats = sims(1);
    let mut world = Flat::default();
    let mut game = Rides { sims: &mut seats, world: &mut world, after: None, time_scale: None, plane: Some(&mut template) };
    game.seat(2, &rides::spawn_on(&flying, &[(2, 3)])).unwrap();
    game.seat(1, &rides::spawn_on(&spawn, &[(1, 1)])).unwrap();
    let template = game_sim::flight::Flight::new(&file);
    let mut want = first.clone();
    vehicle::start_lane(&template, bay(2, &template), &mut want);
    assert_eq!(seats.len(), 3);
    assert_eq!(seats[2].plane, want.plane, "the joiner's flight is N18's start in its bay");
    let at = seats[2].plane.as_ref().map(|f| f.spawn).unwrap();
    assert_eq!((at.0, at.1), (bay(2, &template).at, bay(2, &template).facing));
    assert_eq!(rides::preset(&seats[2]), Some(3), "on its own preset");
    assert!(seats[2].markers.slots[0].is_some() && seats[1].markers.slots[0].is_none(), "with its saved markers");
    assert!(seats[0].plane.is_none() && seats[1].plane.is_none(), "the others on their bikes");
    assert_eq!(rides::preset(&seats[1]), Some(1));
}

// ---------------------------------------------------------------------------------------------------
// One seat's own pause (P84)
// ---------------------------------------------------------------------------------------------------

/// The ticks the host's seat holds its pause open for, by the tick its machine is at.
const PAUSED: Range<usize> = 150..330;

/// The ride with the host's seat pausing over [`PAUSED`]: its pad, the shell's while a seat's pause
/// is open, at rest and saying so in the switch byte.
struct Pausing {
    ride: Ride,
}

impl Machine for Pausing {
    type S = rides::State;
    fn frame(&mut self, np: &mut Netplay<rides::State>, local: &[Report]) {
        let seats = np.lineup.mine(&np.me);
        let mut pads = local.to_vec();
        for (pad, &s) in pads.iter_mut().zip(&seats) {
            if s == 0 && PAUSED.contains(&np.tick()) {
                *pad = Report::idle();
                pad.0[net::SWITCH] = vehicle::Wanted { paused: true, ..Default::default() }.byte();
            }
        }
        let mut game = Rides { sims: &mut self.ride.sims, world: &mut self.ride.world, after: None, time_scale: None, plane: None };
        np.frame(&mut game, &pads, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        self.ride.hash()
    }
}

/// P84 online: the host's seat pauses on its own over a lossy, laggy link. The pause is a bit of
/// the seat's pad, so a rollback input: both machines agree on every settled tick's pads and state;
/// the same pads ridden offline are the same state tick for tick; the paused seat's state does not
/// move a bit while the other seat's rides on, and it rides on from there once resumed.
#[test]
fn one_seat_pauses_alone_online_and_both_peers_agree() {
    let link = Link { loss: 0.1, lag: 3, outage: 0..0, oneway: false };
    let r = run("host,g", Timing::default(), |seats| Pausing { ride: Ride::new(seats) }, &link, 0..480, |_, _, _| {});
    for np in &r.nets {
        assert!(np.desyncs.is_empty() && !np.desynced && np.end.is_none(), "{}: {:?}", np.me, np.desyncs);
        assert!(np.rb.reruns > 0, "{} never ran a tick again", np.me);
    }
    let n = r.agree(&[0, 1], 0);
    assert!(n > 400, "host and g agree on {n} ticks");

    let h = &r.nets[0];
    let mut seats = sims(2);
    let mut world = Flat::default();
    let mut paused = Vec::new();
    let mut moved_after = 0;
    for tick in 0..n {
        let mut game = Rides { sims: &mut seats, world: &mut world, after: None, time_scale: None, plane: None };
        assert_eq!(game.hash(), h.hashes[tick], "offline, the state before tick {tick}");
        let pads: Vec<Report> = h.log[tick].iter().map(|&p| unwire(p)).collect();
        let before = [seats[0].hash(), seats[1].hash()];
        let mut game = Rides { sims: &mut seats, world: &mut world, after: None, time_scale: None, plane: None };
        game.step(tick, &pads);
        let held = vehicle::Wanted::of(pads[0].0[net::SWITCH]).paused;
        assert!(!vehicle::Wanted::of(pads[1].0[net::SWITCH]).paused, "g never paused, tick {tick}");
        assert_ne!(seats[1].hash(), before[1], "g rides on, tick {tick}");
        if held {
            paused.push(tick);
            assert_eq!(seats[0].hash(), before[0], "the paused seat holds still, tick {tick}");
        } else {
            assert_ne!(seats[0].hash(), before[0], "the host's seat rides, tick {tick}");
            moved_after += paused.last().is_some_and(|&t| t < tick) as usize;
        }
    }
    // paused for as long as its pad said so, the session's input delay a tick on
    let want: Vec<usize> = (PAUSED.start + 1..PAUSED.end + 1).collect();
    assert_eq!(paused, want, "the paused ticks");
    assert!(moved_after > 50, "the host's seat rode {moved_after} ticks after resuming");
}

/// P84: a paused seat is passed through. Two seats side by side carrying the bike's collision
/// files push apart; with one paused, the other rides as if nobody were there (as it does with no
/// files at all), and the paused one is not moved either.
#[test]
fn a_paused_seat_is_passed_through_online() {
    use game_sim::bike_col::{self, BikeCol};
    use game_render::disc::Disc;
    let Ok(mut disc) = Disc::open_configured() else {
        eprintln!("skipped: no disc configured");
        return;
    };
    let res = disc.res("Tracks\\free09_ss_2.res").expect("the split track's archive");
    let read = |name: String| res.read_suffix(&name).unwrap_or_else(|e| panic!("{e}"));
    let col = std::sync::Arc::new(
        BikeCol::load(&read(bike_col::rays_file(bike_col::MODEL)), &read(bike_col::shape_file(bike_col::MODEL)))
            .expect("the bike's collision files"),
    );
    let bikes: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).filter(|n| n.to_ascii_lowercase().starts_with("bike\\")).collect();
    let phy = bikes.iter().find_map(|b| disc.res(b).ok()?.read_suffix("race125p.phy").ok()).expect("race125p.phy");
    let tables = game_sim::ride::Tables::build();
    let phy = game_sim::phy::Phy::read_bytes(&phy, &tables.curve);

    // `game-sim/tests/rider_contact.rs`'s pair: 1.5 ft apart along x, wrecks off, one Sim a seat.
    let seat = |i: usize, files: bool| {
        let mut sim = Sim::new(0x5378);
        std::sync::Arc::make_mut(&mut sim.phy).collisions = phy.collisions;
        let mounts = game_sim::stock::mounts(&sim.phy);
        let s = game_sim::stock::SPAWN;
        let at = Vf::new(s.x() + 1.5 * i as f32, s.y(), s.z(), 1.0);
        sim.rider = game_sim::rider::Rider::new(&sim.phy, mounts, at, game_sim::stock::FACING, Default::default());
        sim.rider.sense.wrecks_enabled = false;
        if files {
            sim.rider.carries_bike_col(std::sync::Arc::clone(&col));
        }
        sim
    };
    let ride = |files: bool, pause: bool| {
        let mut sims = vec![seat(0, files), seat(1, files)];
        let mut world = Flat::default();
        let mut pads = [Report::idle(), Report::idle()];
        pads[0].0[net::SWITCH] = vehicle::Wanted { paused: pause, ..Default::default() }.byte();
        let held = sims[0].clone();
        for tick in 0..30 {
            Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None, plane: None }.step(tick, &pads);
            if pause {
                assert_eq!(sims[0].hash(), held.hash(), "the paused seat was moved, tick {tick}");
            }
        }
        sims[1].rider.ride.chassis.clone()
    };
    let (pushed, alone) = (ride(true, false), ride(false, false));
    assert_ne!(pushed, alone, "side by side with the files, the riders push apart");
    assert_eq!(ride(true, true), ride(false, true), "nobody pushes past a paused seat");
}
