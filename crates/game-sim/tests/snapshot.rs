//! The ride saved and put back (N1): what online play's rollback leans on.
//!
//! Over [`players`]' scripted ride of four riders (a stand-in for S54's players: see that file):
//!
//! * save, step, restore, step gives the same hashes — and the same *whole* state, the parts the hash
//!   leaves out included;
//! * a late input rolled back over gives the state the right input would have given all along;
//! * [`snapshot::encode`]'s bytes put into a machine that is somewhere else entirely make it the same
//!   machine, and broken bytes are refused without touching it;
//! * ggrs' sync test, run over the ride, finds no mismatch at any check distance — and finds the one a
//!   state it cannot restore makes, so its silence means something.

mod players;

use std::collections::HashMap;
use std::sync::Arc;

use game_sim::snapshot::{self, Snapshot};
use game_sim::{Input, Sim};
use players::{PLAYERS, Seen};

/// The riders saved at once, the way a session's state is.
fn save(players: &[Sim]) -> Vec<Snapshot> {
    players.iter().map(snapshot::save).collect()
}

fn restore(players: &mut [Sim], saved: &[Snapshot]) {
    players.iter_mut().zip(saved).for_each(|(sim, s)| snapshot::restore(sim, s));
}

/// The scripted ride from frame `from` for `n` ticks: the hash and every rider's whole state after
/// each tick.
fn ride(players: &mut [Sim], from: usize, n: usize, seen: &mut Seen) -> Vec<(u64, Vec<String>)> {
    (from..from + n)
        .map(|f| {
            players::step(players, &players::inputs(f), seen);
            (players::hash(players), players.iter().map(players::whole).collect())
        })
        .collect()
}

#[test]
fn save_step_restore_step_agrees() {
    let mut players = players::start();
    let mut seen = Seen::default();
    let mut at = 0;
    // Saved at the start, in the middle of the run-up, on and off the kickers, down and put back.
    for &save_at in &[0, 61, 137, 200, 281, 345] {
        ride(&mut players, at, save_at - at, &mut seen);
        at = save_at;
        let saved = save(&players);
        let before = players::hash(&players);
        let first = ride(&mut players, at, 60, &mut seen);
        restore(&mut players, &saved);
        assert_eq!(players::hash(&players), before, "restored at tick {at}: the hash is not the saved one");
        for (p, s) in players.iter().zip(&saved) {
            assert_eq!(players::whole(p), players::whole(s.sim()), "restored at tick {at}: the state is not the saved one");
        }
        let again = ride(&mut players, at, 60, &mut seen);
        for (i, (a, b)) in first.iter().zip(&again).enumerate() {
            assert_eq!(a.0, b.0, "saved at {at}, restored: tick {} hashes differently", at + i + 1);
            assert_eq!(a.1, b.1, "saved at {at}, restored: tick {} is a different state", at + i + 1);
        }
        at += 60;
    }
    // The run is only a proof of the parts it reached.
    assert!(seen.airborne && seen.tricks && seen.landed && seen.wrecks && seen.respawns, "the script missed something: {seen:?}");
}

/// The whole-state comparison above sees what the hash does not: the rider's idle pool is not walked
/// (the hash is the simulation's, the pool only draws), so a restore that lost it would hash the same.
#[test]
fn the_whole_state_sees_what_the_hash_leaves_out() {
    let mut players = players::start();
    ride(&mut players, 0, 20, &mut Seen::default());
    let mut other = players[0].clone();
    other.rider.idles.count += 1;
    assert_eq!(other.hash(), players[0].hash());
    assert_ne!(players::whole(&other), players::whole(&players[0]));
    // and the bytes carry it
    let mut there = players::start().swap_remove(0);
    snapshot::decode(&mut there, &snapshot::encode(&other)).unwrap();
    assert_eq!(there.rider.idles.count, other.rider.idles.count);
}

/// What rollback does with a late input: the ride went on with a guess at one rider's pad, the real
/// pad arrived, and the session goes back to the frame it was for and rides it again.
#[test]
fn a_late_input_rolled_back_over_is_as_if_it_had_come_on_time() {
    const LATE: usize = 140;
    const WINDOW: usize = 8;
    let mut seen = Seen::default();

    let mut on_time = players::start();
    ride(&mut on_time, 0, LATE + WINDOW, &mut seen);

    let mut guessed = players::start();
    ride(&mut guessed, 0, LATE, &mut seen);
    let saved = save(&guessed);
    // The guess: rider 1's pad held as it was on the last frame that arrived, as ggrs predicts.
    for f in LATE..LATE + WINDOW {
        let mut pads = players::inputs(f);
        pads[1] = players::input(1, LATE - 1);
        players::step(&mut guessed, &pads, &mut seen);
    }
    restore(&mut guessed, &saved);
    ride(&mut guessed, LATE, WINDOW, &mut seen);

    assert_eq!(players::hash(&guessed), players::hash(&on_time));
    for (a, b) in guessed.iter().zip(&on_time) {
        assert_eq!(players::whole(a), players::whole(b));
    }
}

#[test]
fn a_snapshot_shares_the_constants_and_copies_the_rest() {
    let mut players = players::start();
    ride(&mut players, 0, 30, &mut Seen::default());
    let s = snapshot::save(&players[0]);
    assert!(Arc::ptr_eq(&s.sim().tables, &players[0].tables));
    assert!(Arc::ptr_eq(&s.sim().phy, &players[0].phy));
    // and the riders share theirs with each other, as the session built them
    assert!(Arc::ptr_eq(&players[0].tables, &players[3].tables));
    // A copy, not a view: stepping the ride leaves the snapshot where it was.
    let h = s.hash();
    ride(&mut players, 30, 5, &mut Seen::default());
    assert_eq!(s.hash(), h);
    assert_ne!(players[0].hash(), h);
    // save_into is save
    let mut ring = snapshot::save(&players[1]);
    snapshot::save_into(&players[0], &mut ring);
    assert_eq!(players::whole(ring.sim()), players::whole(&players[0]));
}

#[test]
fn encode_and_decode_make_another_machine_this_one() {
    let mut players = players::start();
    let mut seen = Seen::default();
    let mut at = 0;
    for &send_at in &[0, 45, 170, 299] {
        ride(&mut players, at, send_at - at, &mut seen);
        at = send_at;
        for (p, sim) in players.iter().enumerate() {
            let bytes = snapshot::encode(sim);
            // The other machine is at tick 0 with this rider's constants, and some other rider's
            // state: the bytes have to replace all of it.
            let mut there = players::start().swap_remove(p);
            ride(std::slice::from_mut(&mut there), 0, 7, &mut Seen::default());
            snapshot::decode(&mut there, &bytes).unwrap();
            assert_eq!(players::whole(&there), players::whole(sim), "rider {p} sent at tick {at}");
            assert_eq!(there.hash(), sim.hash());
            assert_eq!(snapshot::encode(&there), bytes, "rider {p} sent at tick {at}: re-encoded differently");
            // and the two ride on together
            let mut here = sim.clone();
            for f in at..at + 40 {
                let input = players::input(p, f);
                here.tick_in(input, &mut players::Kickers);
                there.tick_in(input, &mut players::Kickers);
                assert_eq!(here.hash(), there.hash(), "rider {p} sent at tick {at}: apart at {}", f + 1);
            }
        }
    }
}

#[test]
fn decode_keeps_this_machines_constants() {
    let mut players = players::start();
    ride(&mut players, 0, 50, &mut Seen::default());
    let bytes = snapshot::encode(&players[2]);
    let mut there = players::start().swap_remove(2);
    there.rider.set.stunts[0].name = "only on this machine".into();
    let (tables, phy) = (Arc::clone(&there.tables), Arc::clone(&there.phy));
    snapshot::decode(&mut there, &bytes).unwrap();
    assert_eq!(there.rider.set.stunts[0].name, "only on this machine");
    assert!(Arc::ptr_eq(&there.tables, &tables) && Arc::ptr_eq(&there.phy, &phy));
    assert_eq!(there.hash(), players[2].hash());
}

#[test]
fn decode_refuses_broken_bytes_and_leaves_the_state_alone() {
    let mut players = players::start();
    ride(&mut players, 0, 90, &mut Seen::default());
    let bytes = snapshot::encode(&players[1]);
    let mut there = players::start().swap_remove(1);
    let before = players::whole(&there);

    let mut wrong_magic = bytes.clone();
    wrong_magic[0] ^= 1;
    let mut longer = bytes.clone();
    longer.push(0);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("nothing", Vec::new()),
        ("the magic", wrong_magic),
        ("cut short", bytes[..bytes.len() - 1].to_vec()),
        ("cut in half", bytes[..bytes.len() / 2].to_vec()),
        ("a byte too many", longer),
    ];
    for (what, b) in cases {
        assert!(snapshot::decode(&mut there, &b).is_err(), "{what} decoded");
        assert_eq!(players::whole(&there), before, "{what} wrote to the state");
    }
}

// ---------------------------------------------------------------------------------------------------
// ggrs' sync test.
//
// ggrs 0.13's `SyncTestSession`, transcribed: game-sim takes no dependency that is not a sibling crate
// (`tests/platforms.rs` holds it to that, dev-dependencies included), and ggrs comes with serde,
// bincode, rand, parking_lot and tracing. What is kept is everything the session asks of a game —
// the order of its save, load and advance requests, the ring of `max_prediction + 1` state cells
// indexed by frame, the input delay, and the checksum comparison — so a ride that passes here is
// handed exactly the same requests by the real session (N2), which is what opentennis's
// `HST_NET_SYNCTEST` runs.
// ---------------------------------------------------------------------------------------------------

/// `GgrsRequest`, with the cell as an index into the session's ring.
enum Request {
    Save { cell: usize, frame: i32 },
    Load { cell: usize, frame: i32 },
    Advance { inputs: Vec<Input> },
}

/// `GameState`: what the game saved in a cell.
struct Cell<S> {
    frame: i32,
    data: Option<S>,
    checksum: Option<u64>,
}

/// `SyncTestSession` with its `SyncLayer`. The input queues are kept only as far as a sync test uses
/// them: every input is local and confirmed, so a queue is the inputs by the frame they land on.
struct SyncTest<S> {
    players: usize,
    max_prediction: usize,
    check_distance: usize,
    delay: usize,
    current_frame: i32,
    cells: Vec<Cell<S>>,
    queues: Vec<HashMap<i32, Input>>,
    checksum_history: HashMap<i32, Option<u64>>,
    local: HashMap<usize, Input>,
}

#[derive(Debug)]
struct Mismatch {
    current_frame: i32,
    mismatched_frames: Vec<i32>,
}

impl<S> SyncTest<S> {
    fn new(players: usize, max_prediction: usize, check_distance: usize, delay: usize) -> Self {
        // `SessionBuilder::start_synctest_session`'s own check
        assert!(check_distance < max_prediction, "the check distance has to be inside the prediction window");
        SyncTest {
            players,
            max_prediction,
            check_distance,
            delay,
            current_frame: 0,
            cells: (0..=max_prediction).map(|_| Cell { frame: -1, data: None, checksum: None }).collect(),
            queues: vec![HashMap::new(); players],
            checksum_history: HashMap::new(),
            local: HashMap::new(),
        }
    }

    fn add_local_input(&mut self, handle: usize, input: Input) {
        self.local.insert(handle, input);
    }

    fn advance_frame(&mut self) -> Result<Vec<Request>, Mismatch> {
        let mut requests = Vec::new();
        let current_frame = self.current_frame;
        if self.check_distance > 0 && current_frame > self.check_distance as i32 {
            let oldest = current_frame - self.check_distance as i32;
            let mismatched_frames: Vec<_> = (oldest..=current_frame).filter(|&f| !self.checksums_consistent(f)).collect();
            if !mismatched_frames.is_empty() {
                return Err(Mismatch { current_frame, mismatched_frames });
            }
            let frame_to = self.current_frame - self.check_distance as i32;
            self.adjust_gamestate(frame_to, &mut requests);
        }
        assert_eq!(self.local.len(), self.players, "missing local input");
        for (handle, input) in std::mem::take(&mut self.local) {
            // `InputQueue::add_input` with a frame delay: the input lands `delay` frames on, and the
            // first `delay` frames read the blank input
            self.queues[handle].insert(self.current_frame + self.delay as i32, input);
        }
        if self.check_distance > 0 {
            requests.push(self.save_current_state());
        }
        requests.push(Request::Advance { inputs: self.synchronized_inputs() });
        self.current_frame += 1;
        // `set_last_confirmed_frame`: the inputs before the safe frame are let go
        let safe = self.current_frame - self.check_distance as i32;
        if safe > 0 {
            self.queues.iter_mut().for_each(|q| q.retain(|&f, _| f >= safe - 1));
        }
        Ok(requests)
    }

    fn checksums_consistent(&mut self, frame: i32) -> bool {
        let oldest = self.current_frame - self.check_distance as i32;
        self.checksum_history.retain(|&k, _| k >= oldest);
        let cell = &self.cells[frame as usize % self.cells.len()];
        if cell.frame != frame {
            return true;
        }
        match self.checksum_history.get(&cell.frame) {
            Some(&cs) => cs == cell.checksum,
            None => {
                self.checksum_history.insert(cell.frame, cell.checksum);
                true
            }
        }
    }

    fn adjust_gamestate(&mut self, frame_to: i32, requests: &mut Vec<Request>) {
        let start = self.current_frame;
        // `load_frame`'s asserts
        assert!(frame_to < self.current_frame && frame_to >= self.current_frame - self.max_prediction as i32);
        let cell = frame_to as usize % self.cells.len();
        assert_eq!(self.cells[cell].frame, frame_to);
        self.current_frame = frame_to;
        requests.push(Request::Load { cell, frame: frame_to });
        for i in 0..start - frame_to {
            let inputs = self.synchronized_inputs();
            if i > 0 {
                requests.push(self.save_current_state());
            }
            self.current_frame += 1;
            requests.push(Request::Advance { inputs });
        }
        assert_eq!(self.current_frame, start);
    }

    fn save_current_state(&mut self) -> Request {
        Request::Save { cell: self.current_frame as usize % self.cells.len(), frame: self.current_frame }
    }

    fn synchronized_inputs(&self) -> Vec<Input> {
        self.queues.iter().map(|q| q.get(&self.current_frame).copied().unwrap_or_default()).collect()
    }
}

/// A game under the sync test: how it saves, loads and advances.
trait Game {
    type State;
    fn save(&self) -> (Self::State, u64);
    fn load(&mut self, s: &Self::State);
    fn advance(&mut self, inputs: &[Input]);
}

/// The riders as a game: `Snapshot`s in the cells, the session's hash as the checksum.
struct Ride {
    players: Vec<Sim>,
    seen: Seen,
}

impl Game for Ride {
    type State = Vec<Snapshot>;
    fn save(&self) -> (Vec<Snapshot>, u64) {
        (save(&self.players), players::hash(&self.players))
    }
    fn load(&mut self, s: &Vec<Snapshot>) {
        restore(&mut self.players, s);
    }
    fn advance(&mut self, inputs: &[Input]) {
        players::step(&mut self.players, inputs, &mut self.seen);
    }
}

/// `frames` frames of the session over `game`, every player's pad off the script; the requests handled
/// the way opentennis's rollback handles them (save into the cell with the checksum, load out of it).
fn sync_test<G: Game>(game: &mut G, frames: usize, check_distance: usize) -> Result<(), Mismatch> {
    let mut s = SyncTest::<G::State>::new(PLAYERS, 8, check_distance, 2);
    for f in 0..frames {
        for p in 0..PLAYERS {
            s.add_local_input(p, players::input(p, f));
        }
        for r in s.advance_frame()? {
            match r {
                Request::Save { cell, frame } => {
                    let (data, checksum) = game.save();
                    s.cells[cell] = Cell { frame, data: Some(data), checksum: Some(checksum) };
                }
                Request::Load { cell, frame } => {
                    assert_eq!(s.cells[cell].frame, frame);
                    game.load(s.cells[cell].data.as_ref().unwrap());
                }
                Request::Advance { inputs } => game.advance(&inputs),
            }
        }
    }
    Ok(())
}

#[test]
fn a_sync_test_over_the_scripted_ride_finds_no_mismatch() {
    let mut seen = Seen::default();
    for check_distance in 1..8 {
        let mut game = Ride { players: players::start(), seen: Seen::default() };
        if let Err(m) = sync_test(&mut game, 320, check_distance) {
            panic!("check distance {check_distance}: {m:?}");
        }
        // The sync test's ride is the ride: its last frame is what a plain run of the same pads
        // (two frames late, for the input delay) comes to.
        let mut plain = players::start();
        for f in 0..320 {
            let pads = if f < 2 { [Input::default(); PLAYERS] } else { players::inputs(f - 2) };
            players::step(&mut plain, &pads, &mut Seen::default());
        }
        assert_eq!(players::hash(&game.players), players::hash(&plain), "check distance {check_distance}");
        let s = game.seen;
        seen.airborne |= s.airborne;
        seen.tricks |= s.tricks;
        seen.landed |= s.landed;
        seen.wrecks |= s.wrecks;
        seen.respawns |= s.respawns;
    }
    assert!(seen.airborne && seen.tricks && seen.landed && seen.wrecks && seen.respawns, "the script missed something: {seen:?}");
}

/// The control: a game with one thing outside its saved state — a counter that every advance bumps
/// and nothing saves — which nudges a rider's pad. The sync test has to catch it, or its silence above
/// proves nothing.
#[test]
fn the_sync_test_catches_state_that_is_not_saved() {
    struct Leaky {
        ride: Ride,
        outside: u32,
    }
    impl Game for Leaky {
        type State = Vec<Snapshot>;
        fn save(&self) -> (Vec<Snapshot>, u64) {
            self.ride.save()
        }
        fn load(&mut self, s: &Vec<Snapshot>) {
            self.ride.load(s)
        }
        fn advance(&mut self, inputs: &[Input]) {
            self.outside += 1;
            let mut inputs = inputs.to_vec();
            inputs[0].stick.0 = self.outside as i8;
            self.ride.advance(&inputs)
        }
    }
    let mut game = Leaky { ride: Ride { players: players::start(), seen: Seen::default() }, outside: 0 };
    let m = sync_test(&mut game, 120, 2).expect_err("a leak the sync test did not see");
    assert!(!m.mismatched_frames.is_empty() && m.current_frame > 2, "{m:?}");
}
