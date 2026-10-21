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
        let mut game = Rides { sims: &mut self.sims, world: &mut self.world, after: Some(&mut after), time_scale: None };
        np.frame(&mut game, local, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        let mut sims = self.sims.clone();
        let mut world = Flat::default();
        Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None }.hash()
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
    let mut live = vec![true; r.names.len()];
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
        assert_eq!(w.0[..], r.0[..20], "the wire is the report's first twenty bytes");
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
            let mut game = Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None };
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
    let mut game = Rides { sims: &mut there, world: &mut world, after: None, time_scale: None };
    for t in 0..200 {
        game.step(t, &pads(t));
    }
    let bytes = game.encode().expect("the ride has a codec");
    let hash = game.hash();
    let mut here = sims(3);
    let mut world2 = Flat::default();
    let mut mine = Rides { sims: &mut here, world: &mut world2, after: None, time_scale: None };
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
    let mut other = Rides { sims: &mut two, world: &mut world3, after: None, time_scale: None };
    let theirs = other.encode().unwrap();
    assert!(mine.decode(&theirs).unwrap_err().contains("2 seats in the state, 3 here"));
    let cut = mine.encode().unwrap();
    assert!(mine.decode(&cut[..cut.len() - 1]).is_err());
    assert!(mine.decode(&[cut.clone(), vec![0]].concat()).is_err());
    assert_eq!(mine.hash(), before, "a refused state writes nothing");
}

/// [`mash`]'s pads are a ride as the game reads them (N9): every seat's bike is on the gas nearly
/// every tick at more than one pressure, brakes now and then, holds a stunt on the chord, and the
/// d-pad is touched only by the chord (up), not held throughout. And the bikes get going.
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
        let mut game = Rides { sims: &mut sims, world: &mut world, after: Some(&mut after), time_scale: None };
        for t in 0..ticks {
            game.step(t, &(0..3).map(|s| mash(s, t)).collect::<Vec<_>>());
        }
    }
    for (seat, s) in seen.iter().enumerate() {
        assert!(s.gas > ticks * 8 / 11 && s.gases.len() > 5, "seat {seat}: {s:?}");
        assert!(s.brake > 0 && s.stunt > 0 && s.up > 0, "seat {seat}: {s:?}");
        assert_eq!(s.down, 0, "seat {seat}: d-pad down held");
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

/// The host gone for good: the others wait and end the ride.
#[test]
fn a_silent_host_ends_the_ride() {
    let link = Link { loss: 0.0, lag: 2, outage: 0..0, oneway: false };
    let r = run("host,g", short(), Tally::new, &link, 0..600, |round, _, live| {
        if round == 100 {
            live[0] = false;
        }
    });
    assert_eq!(r.nets[1].end.as_deref(), Some("the host stopped answering: the ride is over"));
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
    let spawn = game_sim::snapshot::encode(&sims(1)[0]);
    let mut r: Run<Ride> = Run {
        names: vec!["host".into()],
        nets: vec![Netplay::new(Lineup::parse("host").unwrap(), "host", Timing::default(), dumps()).unwrap()],
        games: vec![Ride::new(1)],
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
        for (who, seats, at) in [("g", vec![1], joins[0]), ("k", vec![2, 3], joins[1])] {
            if round == at {
                r.names.push(who.into());
                r.nets.push(Netplay::joining("host", who, Timing::default(), dumps()).unwrap());
                r.games.push(Ride::new(1));
                r.ran.push(Vec::new());
            }
            if round + 30 == at {
                r.nets[0].admit(who, &seats, spawn.clone()).unwrap();
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
    let mut room = Room::new("Hosty", "free06", 4);
    room.take(HOST, &[Pick::new("thq1", "race125p")]);
    peers[0].open(room).unwrap();

    let frame = Duration::from_nanos(1_001_000_000_000 / 30_000);
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
        // g once the host has ridden two seconds alone and its code is out; k ten seconds after g is in
        if !asked[0] && tick >= 60 && !short.is_empty() {
            assert!(peers[0].room.as_ref().unwrap().riding, "the room is the ride");
            peers[1].net.send(Cmd::Join(short.clone(), vec![Pick::new("thq2", "pro250a")]));
            asked[0] = true;
        }
        if !asked[1] && joins == 1 && tick >= h.np.admit.joins[0].1 as usize + 300 {
            peers[2].net.send(Cmd::Join(short.clone(), vec![Pick::new("thq3", "race125p"), Pick::new("thq4", "race125p")]));
            asked[1] = true;
        }
        if joins == 2 && h.np.confirmed() > h.np.admit.joins[1].1 as i64 + 300 && peers.iter().all(|p| p.riding.as_ref().is_some_and(|r| r.np.confirmed() > h.np.admit.joins[1].1 as i64 + 300)) {
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

    // from each join on, the machines in the ride ran the same ticks
    let settled = |nps: &[&net::Netplay<rides::State>]| nps.iter().map(|np| np.confirmed().max(0) as usize).min().unwrap();
    for (who, from) in [(vec![h, g], t1), (vec![h, g, k], t2)] {
        let to = settled(&who);
        assert!(to > from + 250, "{to} settled from {from}");
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
    let states: std::collections::BTreeSet<u64> = h.hashes[t2..t2 + 200].iter().copied().collect();
    assert!(states.len() > 100, "{} states", states.len());

    // the host's ride: forward all the way, but for the one step back to the settled tick at k's join
    let back: Vec<(usize, usize)> = host.windows(2).filter(|w| w[1] < w[0]).map(|w| (w[0], w[1])).collect();
    assert!(back.len() <= 1 && back.iter().all(|b| b.1 == t2 && b.0 - b.1 <= WINDOW), "the host went back: {back:?} (joins at {t1}, {t2})");
    assert!(host[0] <= 1 && *host.last().unwrap() > t2 + 300, "{:?}..{:?}", host.first(), host.last());
    assert_eq!((h.recover.restarts, g.recover.restarts, k.recover.restarts), (2, 2, 1));
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
    let game = Rides { sims: &mut sims, world: &mut world, after: None, time_scale: None };
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
