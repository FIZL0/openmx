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
        let mut game = Rides { sims: &mut self.sims, world: &mut self.world, after: Some(&mut after) };
        np.frame(&mut game, local, ROUND.as_secs_f32());
    }
    fn hash(&self) -> u64 {
        let mut sims = self.sims.clone();
        let mut world = Flat::default();
        Rides { sims: &mut sims, world: &mut world, after: None }.hash()
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
    assert!(Lineup::parse("a,b,c,d,e,f,g,h,i").is_err(), "nine seats");
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
            let mut game = Rides { sims: &mut sims, world: &mut world, after: None };
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
    let mut game = Rides { sims: &mut there, world: &mut world, after: None };
    for t in 0..200 {
        game.step(t, &pads(t));
    }
    let bytes = game.encode().expect("the ride has a codec");
    let hash = game.hash();
    let mut here = sims(3);
    let mut world2 = Flat::default();
    let mut mine = Rides { sims: &mut here, world: &mut world2, after: None };
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
    let mut other = Rides { sims: &mut two, world: &mut world3, after: None };
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
        let mut game = Rides { sims: &mut sims, world: &mut world, after: Some(&mut after) };
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
        let mut game = Rides { sims: &mut self.ride.sims, world: &mut self.ride.world, after: Some(&mut after) };
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
        Rides { sims: &mut there.sims, world: &mut there.world, after: None }.step(t, &pads);
    }
    assert!(there.sims[0].rider.ghost.on && !there.sims[1].rider.ghost.on);
    let mut solid = Ride { sims: there.sims.clone(), world: Flat::default(), glitch: false };
    solid.sims[0].rider.ghost = Ghost::default();
    assert_eq!(solid.sims[0].hash(), there.sims[0].hash(), "a one-player sim's hash has no ghost in it");
    assert_ne!(solid.hash(), there.hash(), "the online hash has");

    let bytes = Rides { sims: &mut there.sims, world: &mut there.world, after: None }.encode().unwrap();
    let mut here = Ride::new(2);
    Rides { sims: &mut here.sims, world: &mut here.world, after: None }.decode(&bytes).unwrap();
    assert_eq!(here.sims[0].rider.ghost, there.sims[0].rider.ghost);
    assert_eq!(here.hash(), there.hash());
}
