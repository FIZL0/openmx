//! The ggrs session, run the way opentennis's `rollback.rs` runs it: input delay [`DELAY`], an
//! [`WINDOW`]-tick prediction window, desync detection every [`HASH_EVERY`] ticks, opentennis's time
//! sync (B163/B197: a machine ahead of the others sits a tick out now and then, never several in a
//! row) and its resend (B198: a stalled machine sends its unacknowledged inputs again, and holds before
//! a peer's queue reaches ggrs' 128).
//!
//! ggrs' own disconnect is off: a silent peer is [`super::recover`]'s to deal with, as in opentennis.

use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use game_sim::input::Report;
use ggrs::{Config, DesyncDetection, GgrsError, GgrsRequest, Message, NonBlockingSocket, P2PSession, PlayerType, SessionBuilder};

use super::{DELAY, Game, HASH_EVERY, Lineup, Netplay, Pad, WINDOW, link, unwire, wire};

/// Time sync: while this machine runs ahead of the others, its clock goes back a tick in every
/// `SPREAD` calls (B163: never several in a row; B197: gently).
pub const SPREAD: u32 = 20;
/// How far time sync may set this machine's clock off real time, ticks (B197).
pub const SHIFT: i32 = WINDOW as i32 / 2;
/// The most ticks a machine catches up after a stall (B197).
pub const OWE: i32 = WINDOW as i32;
/// The most inputs a peer may leave unacked before this machine holds (ggrs drops it past 128).
const UNACKED: usize = 120;
/// How long a stall goes before this machine's inputs go again (ggrs' own resend interval).
const RESEND: Duration = Duration::from_millis(200);

pub struct Cfg<S>(PhantomData<S>);

impl<S: Clone + 'static> Config for Cfg<S> {
    type Input = Pad;
    // a held button stays held: no press is guessed
    type InputPredictor = ggrs::PredictRepeatLast;
    type State = S;
    /// Peer names.
    type Address = String;
}

/// ggrs' socket: what it sends waits in `out` for the network, what came in waits in `inbox`.
#[derive(Clone, Default)]
pub struct Wires {
    out: Arc<Mutex<Vec<(String, Message)>>>,
    inbox: Arc<Mutex<Vec<(String, Message)>>>,
    /// The last inputs message to each peer (every input it has not acked yet), sent again while
    /// stalled (B198).
    last: Arc<Mutex<BTreeMap<String, Message>>>,
}

/// Whether `msg` is ggrs' inputs message (its body private: bincode's variant index after the 2-byte
/// header) — opentennis's test, unchanged.
fn inputs(msg: &Message) -> bool {
    bincode::serialize(msg).is_ok_and(|b| b.get(2..6) == Some(&[2, 0, 0, 0]))
}

impl NonBlockingSocket<String> for Wires {
    fn send_to(&mut self, msg: &Message, addr: &String) {
        if inputs(msg) {
            self.last.lock().unwrap().insert(addr.clone(), msg.clone());
        }
        self.out.lock().unwrap().push((addr.clone(), msg.clone()));
    }

    fn receive_all_messages(&mut self) -> Vec<(String, Message)> {
        std::mem::take(&mut *self.inbox.lock().unwrap())
    }
}

type Cell<S> = ggrs::GameStateCell<S>;

/// The rollback session: ggrs', its wires, which seat each ggrs player is.
pub struct Rollback<S: Clone + 'static> {
    pub sess: P2PSession<Cfg<S>>,
    wires: Wires,
    /// ggrs player `h` is seat `seats[h]`; `count` seats in all (the empty ones too).
    seats: Vec<usize>,
    count: usize,
    /// The next tick a run gives its pads to.
    pub frame: usize,
    /// The tick ggrs' frame 0 is (a session restarted mid-ride), and the restart's number (every
    /// datagram carries it, so an older session's are dropped).
    pub base: usize,
    pub round: u8,
    since: u32,
    owed: i32,
    shift: i32,
    /// Ticks run again after a restore, ticks sat out (time sync), frames held (the window, or a peer's
    /// queue), in all.
    pub reruns: u64,
    pub skipped: u64,
    pub held: u64,
    /// ggrs' state cells by the frame last saved in each: a resync's state comes from one and its
    /// re-run saves into them.
    cells: Vec<Option<(i32, Cell<S>)>>,
    /// The frame last run (or inputs last sent again) and when.
    moved: (i32, ggrs::clock::Instant),
    /// Inputs sent again, in all.
    pub resent: u64,
    /// The number the next message in pieces gets, and the pieces coming in.
    ids: u16,
    pieces: link::Pieces,
}

impl<S: Clone + 'static> Rollback<S> {
    /// `lineup`'s session for peer `me`, ggrs' frame 0 being tick `base`: a ggrs player per taken
    /// seat, in seat order.
    pub fn new(lineup: &Lineup, me: &str, base: usize, round: u8) -> Result<Rollback<S>, GgrsError> {
        let seats: Vec<usize> = (0..lineup.seats.len()).filter(|&s| lineup.seats[s].is_some()).collect();
        let mut b = SessionBuilder::<Cfg<S>>::new()
            .with_num_players(seats.len())?
            .with_input_delay(DELAY)
            .with_max_prediction_window(WINDOW)
            .with_desync_detection_mode(DesyncDetection::On { interval: HASH_EVERY })
            // a silent peer is `recover.rs`'s to deal with (ggrs' own drops it after 2 s)
            .with_disconnect_timeout(Duration::from_secs(1 << 20))
            .with_disconnect_notify_delay(Duration::from_secs(1 << 20));
        for (h, &s) in seats.iter().enumerate() {
            let peer = lineup.seats[s].as_ref().unwrap();
            b = b.add_player(if peer == me { PlayerType::Local } else { PlayerType::Remote(peer.clone()) }, h)?;
        }
        let wires = Wires::default();
        Ok(Rollback {
            sess: b.start_p2p_session(wires.clone())?,
            wires,
            seats,
            count: lineup.seats.len(),
            frame: base,
            base,
            round,
            since: 0,
            owed: 0,
            shift: 0,
            reruns: 0,
            skipped: 0,
            held: 0,
            cells: vec![None; WINDOW + 2],
            moved: (0, ggrs::clock::now()),
            resent: 0,
            ids: 0,
            pieces: link::Pieces::default(),
        })
    }

    /// A peer's ggrs datagram (`round`: the session it was sent in).
    pub fn heard(&mut self, from: &str, round: u8, body: &[u8]) {
        if round != self.round {
            return;
        }
        if let Ok(m) = bincode::deserialize::<Message>(body) {
            self.wires.inbox.lock().unwrap().push((from.to_string(), m));
        }
    }

    /// A piece of a ggrs message too big for one datagram.
    pub fn piece(&mut self, from: &str, round: u8, id: u16, index: u8, count: u8, body: &[u8]) {
        if round == self.round
            && let Some(whole) = self.pieces.heard(from, id, index, count, body)
        {
            self.heard(from, round, &whole);
        }
    }

    /// ggrs' datagrams for the network.
    pub fn outgoing(&mut self) -> Vec<(String, Vec<u8>)> {
        let out = std::mem::take(&mut *self.wires.out.lock().unwrap());
        let mut datagrams = Vec::new();
        for (to, m) in out {
            let body = bincode::serialize(&m).unwrap();
            let pieces = link::ggrs(self.round, self.ids, &body);
            if pieces.len() > 1 {
                self.ids = self.ids.wrapping_add(1);
            }
            datagrams.extend(pieces.into_iter().map(|d| (to.clone(), d)));
        }
        datagrams
    }

    /// Stalled [`RESEND`], this machine's last inputs to each peer go again, and again every
    /// [`RESEND`] (B198: two peers stalled at their windows, each missing one of the other's lost
    /// inputs, would otherwise wait for each other for ever).
    pub fn resend(&mut self) {
        let (frame, now) = (self.sess.current_frame(), ggrs::clock::now());
        if frame != self.moved.0 || now - self.moved.1 < RESEND {
            if frame != self.moved.0 {
                self.moved = (frame, now);
            }
            return;
        }
        self.moved.1 = now;
        let last: Vec<_> = self.wires.last.lock().unwrap().iter().map(|(p, m)| (p.clone(), m.clone())).collect();
        self.resent += last.len() as u64;
        self.wires.out.lock().unwrap().extend(last);
    }

    /// The seats on other machines.
    pub fn remote_seats(&self) -> Vec<usize> {
        self.sess.remote_player_handles().into_iter().map(|h| self.seats[h]).collect()
    }

    fn cell(&self, frame: i32) -> Option<Cell<S>> {
        self.cells.iter().flatten().find(|c| c.0 == frame).map(|c| c.1.clone())
    }
}

/// The state before the last tick every seat's pads are in for, as bytes, and that tick (the game is
/// put back as it is now); before the first tick, the game as it is. `None`: no cell for it this
/// frame, or a game that cannot say its state in bytes.
pub(super) fn confirmed<S: Clone + 'static, G: Game<State = S>>(np: &Netplay<S>, game: &mut G) -> Option<(u32, Vec<u8>)> {
    let rb = &np.rb;
    let f = rb.sess.confirmed_frame().max(0);
    let t = (rb.base + f as usize) as u32;
    if rb.frame == rb.base {
        return Some((t, game.encode()?));
    }
    // a machine riding alone (N10: a freeride opened online) has every pad in as it runs them: the
    // state now is settled, and ggrs counts its delayed input as confirmed past it
    if rb.sess.remote_player_handles().is_empty() && f as usize >= rb.frame - rb.base {
        return Some((rb.frame as u32, game.encode()?));
    }
    let then = rb.cell(f)?.load()?;
    let now = game.save();
    game.restore(&then);
    let data = game.encode();
    game.restore(&now);
    Some((t, data?))
}

/// A peer with the host's state before tick `from`: ticks `from..` run again on the pads they ran on,
/// saved into ggrs' cells where those are still its, so a later rollback loads the mended state.
pub(super) fn rerun<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G, from: usize) {
    let (frame, base) = (np.rb.frame, np.rb.base);
    let mut reqs = Vec::new();
    for f in from..frame {
        let g = (f - base) as i32;
        let cell = np.rb.cell(g).unwrap_or_default();
        reqs.push(GgrsRequest::SaveGameState { cell, frame: g });
        let pads = &np.log[f];
        let inputs = np.rb.seats.iter().map(|&s| (pads[s], ggrs::InputStatus::Confirmed)).collect();
        reqs.push(GgrsRequest::AdvanceFrame { inputs });
    }
    np.rb.frame = from;
    np.rb.reruns += (frame - from) as u64;
    handle(np, game, reqs);
}

/// Carries out ggrs' requests: save, restore, run a tick on these pads.
pub(super) fn handle<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G, reqs: Vec<GgrsRequest<Cfg<S>>>) {
    for r in reqs {
        match r {
            GgrsRequest::SaveGameState { cell, frame } => {
                let tick = np.rb.base + frame as usize;
                let h = game.hash();
                np.hashes.resize(tick, 0);
                np.hashes.push(h);
                let n = np.rb.cells.len();
                np.rb.cells[frame as usize % n] = Some((frame, cell.clone()));
                let sum = (frame as u32 % HASH_EVERY == 0).then_some(h as u128);
                cell.save(frame, Some(game.save()), sum);
            }
            GgrsRequest::LoadGameState { cell, frame } => {
                game.restore(&cell.load().expect("a saved state"));
                let f = np.rb.base + frame as usize;
                np.rb.reruns += (np.rb.frame - f) as u64;
                np.rb.frame = f;
            }
            GgrsRequest::AdvanceFrame { inputs } => {
                let mut pads = vec![wire(&Report::missing()); np.rb.count];
                for (h, (p, _)) in inputs.iter().enumerate() {
                    pads[np.rb.seats[h]] = *p;
                }
                let f = np.rb.frame;
                np.log.truncate(f);
                np.log.resize(f, vec![Pad::default(); np.rb.count]);
                let reports: Vec<Report> = pads.iter().map(|&p| unwire(p)).collect();
                np.log.push(pads);
                game.step(f, &reports);
                np.rb.frame += 1;
            }
        }
    }
}

/// The ride's tick gate, once a tick of real time (the shell calls it on pace's 30 Hz schedule):
/// runs the next tick (and any re-runs), or two to catch up, or none (time sync, a stall).
pub(super) fn gate<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G, local: &[Report]) {
    // B197: every call is a tick of real time; one that did not run (a stall, time sync) is run
    // later, a second tick in a call, so the ride keeps 30 ticks a second whatever the others do.
    let want = {
        let rb = &mut np.rb;
        rb.since += 1;
        let ahead = rb.sess.frames_ahead();
        if ahead != 0 && rb.since >= SPREAD {
            let s = (rb.shift + ahead.signum()).clamp(-SHIFT, SHIFT);
            if s != rb.shift {
                (rb.shift, rb.since) = (s, 0);
            }
        }
        rb.owed = (rb.owed + 1).min(OWE.max(rb.shift + 1));
        (rb.owed - rb.shift).clamp(0, 2)
    };
    if want == 0 {
        np.rb.skipped += 1;
    }
    for _ in 0..want {
        if !advance(np, game, local) {
            break;
        }
        np.rb.owed -= 1;
    }
}

/// Runs the next tick (and any re-runs ggrs asks for): false if it could not (waiting on a peer).
fn advance<S: Clone + 'static, G: Game<State = S>>(np: &mut Netplay<S>, game: &mut G, local: &[Report]) -> bool {
    // B198: hold before a peer's unacked inputs reach ggrs' 128, sending them again until it acks
    let rb = &np.rb;
    if rb.sess.remote_player_handles().into_iter().any(|h| rb.sess.network_stats(h).is_ok_and(|n| n.send_queue_len >= UNACKED)) {
        np.rb.held += 1;
        return false;
    }
    // this machine's seats' pads, in seat order (ggrs lists its handles in a hash map's order)
    let mut handles = np.rb.sess.local_player_handles();
    handles.sort_unstable();
    for (i, &h) in handles.iter().enumerate() {
        let pad = local.get(i).map_or(Pad::default(), wire);
        np.rb.sess.add_local_input(h, pad).expect("a local seat");
    }
    let f0 = np.rb.frame;
    match np.rb.sess.advance_frame() {
        Ok(reqs) => {
            handle(np, game, reqs);
            // at the prediction window ggrs answers with a rollback's re-runs and no new tick (B299)
            let new = np.rb.frame > f0;
            np.rb.held += !new as u64;
            new
        }
        Err(GgrsError::PredictionThreshold) => {
            np.rb.held += 1;
            false
        }
        Err(e) => {
            np.say(format!("rollback: {e}"));
            false
        }
    }
}
