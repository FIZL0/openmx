//! **Online play** (N2): ggrs rollback between up to [`SEATS`] riders (N12: thirty-two) on as many
//! machines, any number of them on one machine, taken from opentennis
//! (`crates/hst-play/src/play/rollback.rs`, `resync.rs`, `recover.rs`, `netplay.rs`) with its numbers
//! kept.
//!
//! Each machine runs the next tick at once on its own seats' pads (sent [`DELAY`] ticks ahead) and a
//! guess for everyone else's (their last pads held). When a peer's real pads for a guessed tick arrive
//! and differ, the ride goes back to that tick and runs the ticks since again on the real ones, all
//! before the next frame is drawn ([`rollback`]). A peer more than [`WINDOW`] ticks behind holds the
//! others. Every [`HASH_EVERY`]th tick's state hash goes to the others with the inputs, and two that
//! differ on a confirmed tick are a desync: every seat's pads are written out ([`desync`]), and the
//! host sends its own state for the others to take on ([`resync`]). A peer that goes quiet is waited
//! for, and then ridden on without ([`recover`]).
//!
//! # The pieces
//!
//! * [`Game`] is what the session drives: save, restore, a tick on every seat's pad, a hash, and
//!   the state as bytes. [`rides::Rides`] is the shell's — one [`game_sim::Sim`] a seat — and the
//!   tests drive the same session over it.
//! * A seat's pad goes on the wire as the **original's own pad bytes** ([`wire`]): the twenty bytes
//!   of [`Report`] the game's poll reads, so a pad that rode here rides the same on every machine,
//!   pressures and all.
//! * [`Netplay`] is one machine's whole session: ggrs' (in [`rollback`]), the messages beside it
//!   ([`link`]), and the resync and recovery bookkeeping. It speaks in datagrams to named peers and
//!   nothing else ([`Netplay::heard`], [`Netplay::outgoing`]): which socket carries them is the
//!   caller's ([`udp`] in the shell, a simulated link in `tests/netplay.rs`, iroh once N3 lands).
//!
//! # What changed from opentennis
//!
//! * No Bevy: the session is a plain value the frame loop calls once a frame, and the game is a
//!   trait rather than a `World` and its schedule.
//! * One transport, datagrams. opentennis's pause, end-of-match and resync messages went over iroh
//!   streams; here they are [`link`]'s own small reliable channel over the same datagrams ggrs uses,
//!   so a lossy link loses them too and the tests prove they get there anyway.
//! * No lobby, no bots, no rejoin, no "switched away": those are the front end's and the transport's
//!   (N3). A peer that stays away past the wait has its seats emptied — the rider's pad unplugged —
//!   instead of a bot taking them or everyone going back to a lobby that does not exist yet.
//! * The desync report is a text file of hex pads rather than JSON (no serde_json here), and it
//!   reads back ([`desync::Report::read`]) so a desync can be ridden again offline.

#![forbid(unsafe_code)]

pub mod admit;
pub mod desync;
pub mod link;
pub mod recover;
pub mod resync;
pub mod rides;
pub mod rollback;
pub mod udp;

use std::path::PathBuf;

use game_sim::input::Report;
use serde::{Deserialize, Serialize};

use link::{Link, Msg};

/// The local input delay, ticks: one 30 Hz tick (N6), the 33 ms opentennis's two 60 Hz ticks are.
pub const DELAY: usize = 1;
/// How many ticks a machine may run on guessed pads before it waits (ggrs' prediction window).
/// opentennis's B197: 80 of its 60 Hz ticks (1.3 s), so the others ride on through a peer's 1 s
/// hitch instead of stalling; here 40 of the ride's 30 Hz ones, the same 1.3 s (ggrs' input queue
/// holds 128).
pub const WINDOW: usize = 40;
/// Hashes are compared every this many ticks.
pub const HASH_EVERY: u32 = 30;
/// The most seats a ride has, every machine's together (one rider each): the lobby's cap, which says
/// what limits it. A ride's own count is its [`Lineup`]'s, a runtime number; this machine's seats of
/// it are as many as its pads and its split screen have (`pad::SEATS`).
pub const SEATS: usize = openmx_net::SEATS;

/// The pad bytes a seat sends: bytes 0 to 19 of the pad library's report — status, mode, the button
/// word, the four stick bytes and the twelve pressures, everything the original's poll reads. The
/// report's last twelve bytes are never read by the game and never set by this shell.
pub const WIRE: usize = 20;

/// A seat's pad on the wire ([`wire`]); an idle pad by default, which is what ggrs assumes for a
/// seat before its first real one.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Pad(pub [u8; WIRE]);

impl Default for Pad {
    fn default() -> Pad {
        wire(&Report::idle())
    }
}

/// A pad report as it goes on the wire: its first [`WIRE`] bytes, as the pad library left them.
pub fn wire(r: &Report) -> Pad {
    let mut p = [0; WIRE];
    p.copy_from_slice(&r.0[..WIRE]);
    Pad(p)
}

/// The report a seat's wire bytes stand for; the bytes past [`WIRE`] are zero, as in every report
/// [`Report::idle`] and the shell's pad build.
pub fn unwire(p: Pad) -> Report {
    let mut r = [0; game_sim::input::REPORT_BYTES];
    r[..WIRE].copy_from_slice(&p.0);
    Report(r)
}

/// What a session drives: the whole ride, as rollback needs it.
///
/// `State` is a snapshot ggrs keeps one of a tick (cheap to clone: the shell's shares its tables).
/// `encode`/`decode` are the state as bytes, for a resync and a restart; a game that cannot yet say
/// its state in bytes answers `None`, and then a desync is reported but not mended and a silent peer
/// ends the ride rather than being ridden on without.
pub trait Game {
    type State: Clone + 'static;
    fn save(&self) -> Self::State;
    fn restore(&mut self, state: &Self::State);
    /// Tick `tick` (the ride's first is 0), every seat's pad in seat order (a seat nobody sits in has
    /// [`Report::missing`]). The tick's number is the session's, so a rollback's re-run of it is
    /// handed the same one: the ride's frame length goes by it ([`rides::tick_dt`]).
    fn step(&mut self, tick: usize, pads: &[Report]);
    /// The state in 64 bits, the same on every machine.
    fn hash(&self) -> u64;
    fn encode(&self) -> Option<Vec<u8>>;
    fn decode(&mut self, bytes: &[u8]) -> Result<(), String>;
    /// Seat `seat` becomes a new rider in `spawn`'s state — one seat in the game's own per-seat
    /// bytes — for a peer joining the ride under way ([`admit`]): a seat past the last is added
    /// (with any between, nobody's), one nobody sits in is taken over. A game whose seats cannot
    /// change answers `Err`, and nobody can join it mid-ride.
    fn seat(&mut self, _seat: usize, _spawn: &[u8]) -> Result<(), String> {
        Err("this ride's seats cannot change".into())
    }
    /// The game's modifiers (P55) go from `was` to `to`, both in the game's own text, on the state
    /// just decoded from a restart ([`admit`]) — every machine at the same tick (P100). A machine
    /// already on `to` with `was` the same changes nothing. A game without modifiers can only take
    /// "no change".
    fn remodify(&mut self, was: &str, to: &str) -> Result<(), String> {
        if was == to { Ok(()) } else { Err("this ride's modifiers cannot change".into()) }
    }
}

/// Who sits where: each seat's peer, or nobody. Every machine starts with the same line-up (the
/// lobby's, once N3 has one); the **host** is the peer in the first taken seat.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lineup {
    pub seats: Vec<Option<String>>,
}

impl Lineup {
    /// `host,host,g,-`: a peer name a seat, `-` for nobody.
    pub fn parse(s: &str) -> Result<Lineup, String> {
        let seats: Vec<Option<String>> = s
            .split(',')
            .map(|p| p.trim())
            .map(|p| (p != "-" && !p.is_empty()).then(|| p.to_string()))
            .collect();
        if seats.len() > SEATS {
            return Err(format!("{} seats, the most is {SEATS}", seats.len()));
        }
        if seats.iter().all(Option::is_none) {
            return Err("nobody in any seat".into());
        }
        Ok(Lineup { seats })
    }

    pub fn host(&self) -> &str {
        self.seats.iter().flatten().next().map_or("", |s| s.as_str())
    }

    /// The seats `me` plays, in seat order.
    pub fn mine(&self, me: &str) -> Vec<usize> {
        (0..self.seats.len()).filter(|&s| self.seats[s].as_deref() == Some(me)).collect()
    }

    /// Every peer with a seat, once each, in seat order.
    pub fn peers(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for p in self.seats.iter().flatten() {
            if !v.contains(p) {
                v.push(p.clone());
            }
        }
        v
    }

    pub fn text(&self) -> String {
        self.seats.iter().map(|s| s.as_deref().unwrap_or("-")).collect::<Vec<_>>().join(",")
    }
}

/// How long the recovery waits, seconds (opentennis's numbers; `tests/netplay.rs` shortens the wait).
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// A peer not heard for this long is away.
    pub silent: f32,
    /// ... before it has been heard at all (a machine still loading its disc).
    pub loading: f32,
    /// How long the others wait for an away peer before its seats are emptied.
    pub wait: f32,
    /// How long the guests wait for a silent host before the ride ends.
    pub host_wait: f32,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing { silent: recover::SILENT, loading: recover::LOADING, wait: recover::WAIT, host_wait: recover::HOST_WAIT }
    }
}

/// One machine's online session.
pub struct Netplay<S: Clone + 'static> {
    pub me: String,
    pub lineup: Lineup,
    pub timing: Timing,
    /// Where a desync's report goes.
    pub dumps: PathBuf,
    pub rb: rollback::Rollback<S>,
    pub link: Link,
    /// Every seat's pads, every tick, as the ride ran them (re-runs write over the guesses).
    pub log: Vec<Vec<Pad>>,
    /// The state's hash before every tick, as the ride ran it (re-runs write over it too).
    pub hashes: Vec<u64>,
    pub resync: resync::Resync,
    pub recover: recover::Recover,
    /// Hashes differed and no resync has mended it yet.
    pub desynced: bool,
    /// Every desync seen: (tick, this machine's hash, a peer's).
    pub desyncs: Vec<(u32, u64, u64)>,
    /// The desync report, once written (once a session).
    pub report: Option<PathBuf>,
    /// Why the session is over, once it is.
    pub end: Option<String>,
    /// What the frame would say on screen ("Waiting for g... 12 s"); empty: nothing.
    pub notice: String,
    /// Log lines not printed yet (the caller prints them; the tests keep them quiet).
    pub said: Vec<String>,
    /// Peers joining the ride under way (N10): the host's queue, and every join so far.
    pub admit: admit::Admit,
    /// The game's modifiers as every machine rides them (P100), in the game's own text: the host's
    /// to set when the ride starts; each change the host makes goes out with a restart.
    pub mods: String,
    /// Every change of the modifiers this machine took: (the tick it took them at, the new set).
    pub remods: Vec<(u32, String)>,
}

impl<S: Clone + 'static> Netplay<S> {
    pub fn new(lineup: Lineup, me: &str, timing: Timing, dumps: PathBuf) -> Result<Netplay<S>, String> {
        if lineup.mine(me).is_empty() {
            return Err(format!("{me} has no seat in {}", lineup.text()));
        }
        let rb = rollback::Rollback::new(&lineup, me, 0, 0).map_err(|e| format!("ggrs: {e}"))?;
        let others = lineup.peers().into_iter().filter(|p| p != me);
        Ok(Netplay {
            me: me.to_string(),
            recover: recover::Recover::new(others),
            lineup,
            timing,
            dumps,
            rb,
            link: Link::default(),
            log: Vec::new(),
            hashes: Vec::new(),
            resync: resync::Resync::default(),
            desynced: false,
            desyncs: Vec::new(),
            report: None,
            end: None,
            notice: String::new(),
            said: Vec::new(),
            admit: admit::Admit::default(),
            mods: String::new(),
            remods: Vec::new(),
        })
    }

    /// A machine joining `host`'s ride under way (N10): no seat and no session until the host's
    /// restart brings both ([`admit`]); until then it only keeps in touch with the host.
    pub fn joining(host: &str, me: &str, timing: Timing, dumps: PathBuf) -> Result<Netplay<S>, String> {
        if host == me {
            return Err(format!("{me} cannot join its own ride"));
        }
        let mut np = Netplay::new(Lineup { seats: vec![Some(me.to_string())] }, me, timing, dumps)?;
        // a session of its own seat alone, never run, and numbered off every real one's datagrams
        np.rb.round = u8::MAX;
        np.lineup = Lineup { seats: vec![Some(host.to_string())] };
        np.recover = recover::Recover::new(std::iter::once(host.to_string()));
        Ok(np)
    }

    /// Waiting for the host's state to join with: no seat yet.
    pub fn waiting(&self) -> bool {
        self.lineup.mine(&self.me).is_empty()
    }

    /// The host: peer `peer` joins the ride under way on `seats`, each spawned as `spawn` (one seat
    /// in the game's own bytes). It happens at the first frame that can once `peer` has been heard
    /// from: [`admit`].
    pub fn admit(&mut self, peer: &str, seats: &[usize], spawn: Vec<u8>) -> Result<(), String> {
        admit::ask(self, peer, seats, spawn)
    }

    /// The host: the ride's modifiers become `to` (the game's own text), on every machine at one tick
    /// — the first frame that can take a restart, as a join is taken ([`admit`]). A second change
    /// before then replaces the first; a change back to the set ridden now is no change.
    pub fn remodify(&mut self, to: String) -> Result<(), String> {
        if !self.host() || self.waiting() {
            return Err("only the host changes the modifiers".into());
        }
        self.admit.mods = (to != self.mods).then_some(to);
        Ok(())
    }

    /// The host: a join asked for and not taken on yet is off (the peer left first).
    pub fn unadmit(&mut self, peer: &str) {
        self.admit.forget(peer);
    }

    pub fn host(&self) -> bool {
        self.lineup.host() == self.me
    }

    /// The tick the next run is.
    pub fn tick(&self) -> usize {
        self.rb.frame
    }

    /// The last tick every seat's pads are in for, here.
    pub fn confirmed(&self) -> i64 {
        self.rb.base as i64 + self.rb.sess.confirmed_frame() as i64
    }

    fn say(&mut self, s: String) {
        self.said.push(s);
    }

    /// The session is over, with why.
    pub fn finish(&mut self, why: String) {
        if self.end.is_none() {
            self.say(format!("online: {why}"));
            self.end = Some(why);
        }
    }

    /// A datagram from peer `from`.
    pub fn heard(&mut self, from: &str, data: &[u8]) {
        self.recover.heard_from(from);
        self.admit.heard(from);
        match link::open(data) {
            Some(link::Kind::Ggrs(round, body)) => self.rb.heard(from, round, body),
            Some(link::Kind::Reliable(seq, body)) => {
                for m in self.link.heard(from, seq, body) {
                    self.take(from, m);
                }
            }
            Some(link::Kind::Piece { round, id, index, count, body }) => self.rb.piece(from, round, id, index, count, body),
            Some(link::Kind::Ack(next)) => self.link.acked(from, next),
            Some(link::Kind::Alive) | None => {}
        }
    }

    /// A message from a peer, in order.
    fn take(&mut self, from: &str, m: Msg) {
        match m {
            Msg::State { .. } | Msg::Resynced(_) => resync::heard(self, from, m),
            Msg::Away { .. } | Msg::Restart { .. } | Msg::Mods { .. } => recover::heard(self, from, m),
            Msg::End(why) => self.finish(why),
        }
    }

    /// A message to every other peer.
    pub(crate) fn tell(&mut self, m: Msg) {
        for p in self.lineup.peers() {
            if p != self.me {
                self.link.send(&p, m.clone());
            }
        }
    }

    /// Everything to send now: (peer, datagram).
    pub fn outgoing(&mut self) -> Vec<(String, Vec<u8>)> {
        let mut out = self.rb.outgoing();
        out.extend(self.recover.alive(&self.lineup, &self.me));
        out.extend(self.link.outgoing());
        out
    }

    /// One tick of real time (`dt` seconds, `pace::TICK` on schedule: the shell calls it once per
    /// tick the ride's 30 Hz clock owes): ggrs' upkeep, the recovery's clocks, a state taken from the
    /// host, and then this call's ticks — none, one, or two to catch up — on `local`, this machine's
    /// seats' pads in seat order. Re-runs happen inside.
    pub fn frame<G: Game<State = S>>(&mut self, game: &mut G, local: &[Report], dt: f32) {
        self.poll();
        recover::tick(self, dt);
        recover::restarted(self, game);
        resync::take_state(self, game);
        if self.end.is_some() || self.waiting() || self.rb.sess.current_state() != ggrs::SessionState::Running {
            return;
        }
        rollback::gate(self, game, local);
        resync::send_state(self, game);
        admit::run(self, game);
    }

    /// ggrs' upkeep (acks, keep-alives, its events), every frame.
    fn poll(&mut self) {
        self.rb.sess.poll_remote_clients();
        self.rb.resend();
        let events: Vec<_> = self.rb.sess.events().collect();
        for e in events {
            if let ggrs::GgrsEvent::DesyncDetected { frame, local_checksum, remote_checksum, .. } = e {
                let t = self.rb.base as u32 + frame as u32;
                desync::seen(self, (t, local_checksum as u64, remote_checksum as u64));
            }
        }
    }
}
