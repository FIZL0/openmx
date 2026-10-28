//! The freestyle event (S92), against the original:
//!
//! 1. **the clock** — the score manager's tick run in the interpreter on a manager of the event's own
//!    making (the human's freeride manager copied, its listener the stunt competition's own countdown
//!    listener, its cards the state's), from the countdown's start to every card out, beside
//!    [`Competition::tick`]: the countdown's words, the time limit, each card's time, its *taking
//!    part* and *out* flags and the manager's *all done* and *timed out*, after every tick, at the
//!    game's dt and at random ones, one card and two;
//! 2. **the frame** — the activity's leader, tie, warning, target and verdict rules, by example
//!    (not run against the original: its frame reaches into the HUD and the session's player list);
//! 3. **a live event** (C9) — free09's stunt competition ridden in PCSX2, its activity, manager and
//!    card read every frame, beside the port joined, started and ticked on the capture's totals: the
//!    join's saved total, the countdown, the clock, the leader, the end and the total put back.

use game_sim::event::{self, Competition, Verdict};
use game_sim::score::Score;
use ps2_float::F;
use ps2_oracle::{Ee, Kit, addr, skip};

/// The human's RAM: the session's score manager and four cards, all empty.
const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
const BUDGET: u64 = 4_000_000;
/// A block of the state's RAM nobody uses: the fake activity, manager, listener and stub code.
const BLOCK: u32 = 0x01e0_0000;

struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * ((self.next() >> 40) as f32 / (1u64 << 24) as f32)
    }
}

/// The machine, a fake event manager on it with `seats` of the state's cards, and the cards.
///
/// The activity is laid out as the stunt competition's: its manager at +0x50, its listener at
/// +0x160 (a word pointing back at the manager, the listener vtable at +0x1c), the activity's rider
/// list at +0x14 empty (the listener's HUD half walks it). Every other listener call lands on a
/// `jr ra`, and the cards' own listeners are cut, so the manager's work is all that runs.
fn machine(seats: usize) -> Option<(Ee, u32, Vec<u32>)> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let path = kit.ctx("states").join(STATE);
    if !path.exists() {
        skip(&format!("{} is not there: `./kit state ram` of the human's first_recording .p2s", path.display()));
        return None;
    }
    let (gm, gp, start) = match (addr("game_manager"), addr("boot_gp"), addr("score_countdown_start")) {
        (Ok(a), Ok(b), Ok(c)) => (a, b, c),
        (Err(why), ..) | (_, Err(why), _) | (.., Err(why)) => {
            skip(&why);
            return None;
        }
    };
    let listener = match addr("stunt_countdown_listener") {
        Ok(a) => a,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(gp));
    for a in (BLOCK..BLOCK + 0x1000).step_by(4) {
        assert_eq!(ee.u32_at(a), 0, "the block at {BLOCK:#x} is in use in the state");
    }
    // The human's rider, as the S79 test finds it, and its manager.
    let mut node = ee.u32_at(ee.u32_at(ee.u32_at(gm) + 0x34) + 0x34);
    let mut human = None;
    for _ in 0..64 {
        if node == 0 {
            break;
        }
        let item = ee.u32_at(node);
        let ctl = ee.u32_at(item + 0x20);
        if ee.u32_at(item) == 2 && ctl != 0 && ee.u32_at(ctl + 0x34) == 1 {
            human = Some(item);
        }
        node = ee.u32_at(node + 8);
    }
    let rider = human.expect("no human rider in the state");
    let real = ee.u32_at(rider + 0xfa0);
    let mut cards = Vec::new();
    let mut c = ee.u32_at(real + 0x34);
    while c != 0 && cards.len() < seats {
        cards.push(c);
        c = ee.u32_at(c + 0x1c8);
    }
    assert_eq!(cards.len(), seats, "the state has fewer cards");

    let activity = BLOCK;
    let mgr = activity + 0x50;
    let listen = activity + 0x160;
    let vtable = BLOCK + 0x400;
    let stub = BLOCK + 0x800;
    ee.set_u32(stub, 0x03e0_0008); // jr ra
    ee.set_u32(stub + 4, 0);
    for k in 0..0x40 {
        ee.set_u32(vtable + k * 4, stub);
    }
    ee.set_u32(vtable + 0x5c, listener);
    for k in (0..0x110).step_by(4) {
        let w = ee.u32_at(real + k);
        ee.set_u32(mgr + k, w);
    }
    ee.set_u32(activity + 0x14, 0);
    ee.set_u32(listen, mgr);
    ee.set_u32(listen + 4, 0);
    ee.set_u32(listen + 0x1c, vtable);
    ee.set_u32(mgr + 0x24, 0); // no children
    ee.set_u32(mgr + 0x30, listen);
    ee.set_u32(mgr + 0x34, cards[0]);
    ee.set_u32(mgr + 0x5c, 0x4000); // the constructor's ranking flag: not running, not counting
    ee.set_u32(mgr + 0x64, 3);
    ee.set_f32(mgr + 0xa0, 0.0);
    ee.set_f32(mgr + 0xf8, 0.0);
    for (k, &c) in cards.iter().enumerate() {
        ee.set_u32(c + 4, 0);
        ee.set_u32(c + 0x1c4, 0);
        ee.set_u32(c + 8, 0x80804); // the join's flags
        ee.set_f32(c + 0xb4, 0.0);
        ee.set_f32(c + 0xc4, 0.0); // the start's
        ee.set_u32(c + 0x1c8, cards.get(k + 1).copied().unwrap_or(0));
    }
    // The start's countdown: its length and interval, then the manager's countdown start.
    ee.set_f32(mgr + 0xbc, event::COUNTDOWN.0);
    ee.set_f32(mgr + 0xc0, event::INTERVAL.0);
    ee.call(start, &[u64::from(mgr)], &[], BUDGET);
    Some((ee, mgr, cards))
}

/// The manager's and the cards' words, the original's.
fn game(ee: &Ee, mgr: u32, cards: &[u32]) -> String {
    let m = ee.u32_at(mgr + 0x5c);
    let mut s = format!(
        "counting {} running {} over {} timed out {} ended {} elapsed {:08x} tally {:08x} limit {:08x} clock {:08x}",
        m & 0x80 != 0,
        m & 1 != 0,
        m & 2 != 0,
        m & 0x10 != 0,
        m & 0x2000 != 0,
        ee.u32_at(mgr + 0xc4),
        ee.u32_at(mgr + 0xc8),
        ee.u32_at(mgr + 0xa0),
        ee.u32_at(mgr + 0xf8),
    );
    for &c in cards {
        let f = ee.u32_at(c + 8);
        s += &format!(
            " | left {:08x} ridden {:08x} part {} out {}",
            ee.u32_at(c + 0xc4),
            ee.u32_at(c + 0xb4),
            f & 2 != 0,
            f & 1 != 0
        );
    }
    s
}

/// The same words, the port's.
fn port(c: &Competition) -> String {
    let mut s = format!(
        "counting {} running {} over {} timed out {} ended {} elapsed {:08x} tally {:08x} limit {:08x} clock {:08x}",
        c.counting,
        c.running,
        c.over,
        c.timed_out,
        c.ended,
        c.elapsed.to_bits(),
        c.tally.to_bits(),
        c.limit.to_bits(),
        c.clock.to_bits(),
    );
    for seat in &c.seats {
        s += &format!(
            " | left {:08x} ridden {:08x} part {} out {}",
            seat.left.to_bits(),
            seat.ridden.to_bits(),
            seat.taking_part,
            seat.out
        );
    }
    s
}

/// **The clock, tick for tick.** Six runs — one card and two, at the game's 30 Hz, at 60 Hz and at
/// random dts — each from the countdown's start to ten ticks past the end, a card's clock held now
/// and then: the original's manager and cards and the port's after every tick, bit for bit.
#[test]
fn the_event_clock_runs_like_the_original() {
    let Ok(tick) = addr("score_manager_tick") else {
        skip("score_manager_tick is not in research/oracle.txt");
        return;
    };
    let mut pick = Pick(0x0dd_ba11_c0ff_ee42);
    let mut ticks = 0;
    let mut checked = [0usize; 4]; // countdown ticks, running ticks, outs, held ticks
    for run in 0..6 {
        let seats = 1 + run % 2;
        let Some((mut ee, mgr, cards)) = machine(seats) else { return };
        let mut comp = Competition::new(0);
        let mut owned: Vec<Score> = (0..seats).map(|_| Score::default()).collect();
        for card in &mut owned {
            comp.join(card);
        }
        comp.start();
        assert_eq!(port(&comp), game(&ee, mgr, &cards), "run {run}: the start");
        let mut after = 0;
        let mut t = 0;
        while after < 10 {
            let dt = match run / 2 {
                0 => 0.033_333_335,
                1 => 1.0 / 60.0,
                _ => match pick.below(4) {
                    0 => pick.range(0.0, 0.5),
                    1 => pick.range(0.0, 0.02),
                    _ => 0.033_333_335,
                },
            };
            // Now and then a card's clock is held (the card's pause flag) for a while.
            if comp.running && pick.below(200) == 0 {
                let k = pick.below(seats as u64) as usize;
                let held = !comp.seats[k].paused;
                comp.seats[k].paused = held;
                let f = ee.u32_at(cards[k] + 8);
                ee.set_u32(cards[k] + 8, if held { f | 0x40000 } else { f & !0x40000 });
            }
            if comp.seats.iter().any(|s| s.paused) {
                checked[3] += 1;
            }
            let outs = comp.seats.iter().filter(|s| s.out).count();
            ee.call(tick, &[u64::from(mgr)], &[dt], BUDGET);
            let mut refs: Vec<&mut Score> = owned.iter_mut().collect();
            comp.tick(F(dt), &mut refs);
            checked[if comp.running { 1 } else { 0 }] += 1;
            checked[2] += comp.seats.iter().filter(|s| s.out).count() - outs;
            assert_eq!(port(&comp), game(&ee, mgr, &cards), "run {run}, tick {t} (dt {dt}): the port's clock is not the game's");
            for (k, card) in owned.iter().enumerate() {
                assert_eq!(card.shut, !comp.running, "run {run}, tick {t}: seat {k}'s gate");
                assert_eq!(card.out, comp.seats[k].out, "run {run}, tick {t}: seat {k}'s out");
            }
            if comp.ended {
                after += 1;
            }
            t += 1;
            assert!(t < 100_000, "run {run}: the event never ended");
        }
        ticks += t;
        // A paused card can stay in after the others are out; the run ended, so none did.
        assert!(comp.seats.iter().all(|s| s.out), "run {run}: a card never ran out");
    }
    assert!(checked.iter().all(|&n| n > 0), "a case never ran: {checked:?}");
    println!(
        "event clock: {ticks} manager ticks bit-exact in 6 runs ({} counting down, {} running, {} cards out, {} with a clock held)",
        checked[0], checked[1], checked[2], checked[3]
    );
}

/// The countdown at the game's 30 Hz: the clock's limit lands on the tick the countdown's elapsed
/// time passes 3.33, the manager runs from the next, and 120 seconds are 3600 ticks and one: the
/// tick that takes the clock to zero or under is the one that puts the card out.
#[test]
fn the_countdown_at_thirty_hertz() {
    let mut comp = Competition::new(100_000);
    let mut card = Score { total: F(1234.75), ..Score::default() };
    comp.join(&mut card);
    assert_eq!((card.total, comp.seats[0].saved), (F::ZERO, 1234), "the join puts the total aside");
    assert!(card.shut, "no combo opens before the event runs");
    comp.start();
    let dt = F(0.033_333_335);
    let mut limit_at = None;
    let mut run_at = None;
    let mut out_at = None;
    for t in 0..4000 {
        comp.tick(dt, &mut [&mut card]);
        if limit_at.is_none() && comp.limit == event::LIMIT {
            limit_at = Some(t);
        }
        if run_at.is_none() && comp.running {
            run_at = Some(t);
        }
        if out_at.is_none() && comp.seats[0].out {
            out_at = Some(t);
        }
    }
    let (limit_at, run_at, out_at) = (limit_at.unwrap(), run_at.unwrap(), out_at.unwrap());
    println!("limit set at tick {limit_at}, running from {run_at}, out at {out_at}");
    assert_eq!(limit_at, run_at, "the limit and the start of the clock land on one tick");
    assert!((3590..3620).contains(&(out_at - run_at)), "120 s at 30 Hz: {} ticks", out_at - run_at);
    assert!(card.out && !card.shut, "out of time, the event still running");
}

/// The frame's rules: the leader is whoever first passed the best whole number, alone the rider wins
/// above the target and not at it, two riders tie on equal totals, and the end puts the freeride
/// total back with the event's on top.
#[test]
fn the_verdicts() {
    let finish = |target: i32, totals: &[f32]| {
        let mut comp = Competition::new(target);
        let mut cards: Vec<Score> = totals.iter().map(|_| Score { total: F(500.0), ..Score::default() }).collect();
        for c in &mut cards {
            comp.join(c);
        }
        comp.start();
        for (c, &t) in cards.iter_mut().zip(totals) {
            c.total = F(t);
        }
        let mut refs: Vec<&mut Score> = cards.iter_mut().collect();
        comp.tick(F(0.033_333_335), &mut refs);
        let reached = comp.seats[0].reached;
        let out: Vec<_> = (0..totals.len()).map(|k| comp.finish(k, &mut cards[k])).collect();
        (out, reached, comp.leader, cards[0].total)
    };
    let (v, reached, _, total) = finish(1000, &[1000.0]);
    assert_eq!(v[0], (Verdict::Lose, 1500), "at the target is not above it");
    assert!(reached, "the announcer speaks at the target");
    assert_eq!(total, F(1500.0), "the freeride total is back, with the event's on top");
    assert_eq!(finish(1000, &[1000.5]).0[0].0, Verdict::Win);
    assert_eq!(finish(1000, &[999.0]).1, false);
    let (v, _, leader, _) = finish(0, &[100.5, 100.7]);
    assert_eq!((leader, v[0].0, v[1].0), (Some(1), Verdict::Lose, Verdict::Win), "100.7 passes the whole 100");
    let (v, _, leader, _) = finish(0, &[100.5, 100.5]);
    assert_eq!((leader, v[0].0, v[1].0), (Some(1), Verdict::Tie, Verdict::Tie), "an equal 100.5 passes 100 too");
    let (_, _, leader, _) = finish(0, &[100.0, 100.0]);
    assert_eq!(leader, Some(0), "a whole total equal to the best does not pass it");
    let (v, _, leader, _) = finish(0, &[300.0, 200.0]);
    assert_eq!((leader, v[0].0, v[1].0), (Some(0), Verdict::Win, Verdict::Lose), "two riders: no target");
}

/// The track's target out of its `hit.dat`.
#[test]
fn the_target_from_hit_dat() {
    let dat = "[ASZ 1]\nStuntCompetitionScore = 5\n[HitData]\nType=Hit\nStuntCompetitionScore = 400000\n";
    assert_eq!(event::target(dat), Some(400_000));
    assert_eq!(event::target("[HitData]\nType=Hit\n"), None);
    assert_eq!(event::target("[hitdata]\nstuntcompetitionscore=150000 ; free10\n"), Some(150_000));
}

/// One row of a C9 capture, by column name; an empty cell (a null pointer chain) is `None`.
struct Row<'a> {
    cols: &'a [&'a str],
    cells: Vec<&'a str>,
}

impl Row<'_> {
    fn get(&self, name: &str) -> Option<u32> {
        let i = self.cols.iter().position(|c| *c == name)?;
        self.cells.get(i).filter(|c| !c.is_empty()).map(|c| c.parse().unwrap_or_else(|e| panic!("{name} = {c}: {e}")))
    }
    fn word(&self, name: &str) -> u32 {
        self.get(name).unwrap_or_else(|| panic!("frame {:?}: no {name}", self.get("frame_counter")))
    }
}

/// The words a capture row and the port share, in the original's encoding: the manager's flags
/// (running 0x1, all done 0x2, timed out 0x10, counting 0x80, ended 0x2000), the countdown's
/// elapsed, accumulator, length and interval, the limit, the clock, the card's flags (out 0x1,
/// taking part 0x2), time left and seconds ridden, a leader, the tie and the target-reached latch.
const WORDS: [&str; 14] = [
    "mgr_flags", "mgr_elapsed", "mgr_tally", "mgr_length", "mgr_interval", "mgr_limit", "mgr_clock", "c0_flags",
    "c0_left", "c0_ridden", "act_leader", "act_tie", "p0_reached", "act_players",
];
/// The manager's and the card's flag bits the port models.
const MGR_BITS: u32 = 0x1 | 0x2 | 0x10 | 0x80 | 0x2000;
const CARD_BITS: u32 = 0x1 | 0x2;

fn words_of(row: &Row) -> [u32; 14] {
    let mut w = WORDS.map(|n| row.word(n));
    w[0] &= MGR_BITS;
    w[7] &= CARD_BITS;
    w[10] = u32::from(w[10] != 0);
    w
}

fn words(comp: &Competition) -> [u32; 14] {
    let s = &comp.seats[0];
    let mgr = u32::from(comp.running)
        | u32::from(comp.over) << 1
        | u32::from(comp.timed_out) << 4
        | u32::from(comp.counting) << 7
        | u32::from(comp.ended) << 13;
    [
        mgr,
        comp.elapsed.to_bits(),
        comp.tally.to_bits(),
        comp.length.to_bits(),
        comp.interval.to_bits(),
        comp.limit.to_bits(),
        comp.clock.to_bits(),
        u32::from(s.out) | u32::from(s.taking_part) << 1,
        s.left.to_bits(),
        s.ridden.to_bits(),
        u32::from(comp.leader == Some(0)),
        u32::from(comp.tie),
        u32::from(s.reached),
        comp.seats.len() as u32,
    ]
}

/// **A live event** (C9): free09's stunt competition ridden in PCSX2 from the zone's prompt to the
/// end, its activity, manager and card read every frame (`context/capture/c9/run*.csv`, spec
/// `research/journal/2026-10-28-c9-freestyle-event-capture/c9-event.toml`), beside
/// [`Competition`] joined, started and ticked at the capture's dt on the capture's totals.
///
/// A row is not a tick: the game sits still while the instructions are up, and PINE's reads land
/// between ticks, two ticks apart or in the middle of one. So every row must equal the port after
/// some tick, never going back — or, read mid-tick, every word must be that tick's or the next's.
/// The card's seconds ridden are not reset by the join (the original carries the freeride
/// manager's), so the seat is given the join row's. At the end the original takes the rider out
/// (the player list empties): the port must have ended, and its finish must put the freeride total
/// back as the original did.
#[test]
fn a_live_event_runs_like_the_port() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let dir = kit.ctx("capture/c9");
    let mut runs = 0;
    for name in ["run1.csv", "run2.csv"] {
        let path = dir.join(name);
        let Ok(text) = std::fs::read_to_string(&path) else {
            skip(&format!("{} is not there: C9's capture (see its journal)", path.display()));
            continue;
        };
        let mut lines = text.lines();
        let cols: Vec<&str> = lines.next().expect("a header").split(',').collect();
        let rows: Vec<Row> = lines.map(|l| Row { cols: &cols, cells: l.split(',').collect() }).collect();
        let join = rows.iter().position(|r| r.get("act_players") == Some(1)).expect("nobody joined");
        let gone = join + rows[join..].iter().position(|r| r.get("act_players") == Some(0)).expect("the event never ended");
        let target = rows[join].word("act_target") as i32;
        assert_eq!(target, 100_000, "{name}: free09's target");

        // The freeride card before the join, when the capture has it (run2 seeded it by hand).
        let before = rows[join - 1].get("free_total").map_or(F::ZERO, F::from_bits);
        let mut card = Score { total: before, ..Score::default() };
        let mut comp = Competition::new(target);
        comp.join(&mut card);
        comp.start();
        assert_eq!(card.total, F::ZERO, "{name}: the join puts the total aside");
        if let Some(saved) = rows[join].get("p0_saved") {
            assert_eq!(comp.seats[0].saved, saved as i32, "{name}: the join's saved total");
        }
        comp.seats[0].ridden = F::from_bits(rows[join].word("c0_ridden"));

        // The port after 0, 1, 2 … ticks, made as the rows ask for them. A tick made ahead takes
        // this row's total: alone and under the target, only the leader reads it (set on tick one).
        let mut after = vec![words(&comp)];
        let mut k = 0;
        let (mut exact, mut torn, mut still, mut double) = (0, 0, 0, 0);
        // How often each word was read a tick ahead of the rest.
        let mut ahead = [0; 14];
        for (i, row) in rows[join..gone].iter().enumerate() {
            let want = words_of(row);
            let dt = F::from_bits(row.word("dt"));
            let total = F::from_bits(row.word("c0_total"));
            let mut tick = |after: &mut Vec<[u32; 14]>, comp: &mut Competition, card: &mut Score| {
                card.total = total;
                comp.tick(dt, &mut [card]);
                after.push(words(comp));
            };
            // A whole tick first, then a row read mid-tick, a few ticks on at most.
            let from = k;
            while after.len() < from + 6 {
                tick(&mut after, &mut comp, &mut card);
            }
            let found = if let Some(j) = (from..from + 5).find(|&j| after[j] == want) {
                k = j;
                exact += 1;
                true
            } else if let Some(j) =
                (from..from + 4).find(|&j| (0..14).all(|w| want[w] == after[j][w] || want[w] == after[j + 1][w]))
            {
                k = j;
                torn += 1;
                for w in (0..14).filter(|&w| want[w] != after[j][w]) {
                    ahead[w] += 1;
                }
                true
            } else {
                false
            };
            assert!(
                found,
                "{name}, frame {}: no tick from {from} on matches\n row  {:08x?}\n port {:08x?}\n next {:08x?}",
                row.word("frame_counter"),
                want,
                after[from],
                after[from + 1]
            );
            match k - from {
                0 if i > 0 => still += 1,
                2.. => double += 1,
                _ => {}
            }
        }
        // The row the rider is gone on: the manager has ended (all done, timed out), the clock on the
        // tick that ended it. The port has looked a few ticks ahead; the card is out, so those ticks
        // moved nothing but the clock.
        let end = &rows[gone];
        let ended = (k..after.len()).find(|&j| after[j][0] & 0x2000 != 0).expect("the port never ended");
        assert!(ended - k <= 2, "{name}: the port ends {} ticks after the last row", ended - k);
        assert!(comp.seats[0].out, "{name}: the card is not out at the end");
        assert_eq!(end.word("mgr_flags") & MGR_BITS, after[ended][0], "{name}: the manager's flags when the rider leaves");
        assert_eq!(end.word("mgr_clock"), after[ended][6], "{name}: the clock when the rider leaves");
        let event = card.total;
        let (verdict, profile) = comp.finish(0, &mut card);
        assert_eq!(verdict, Verdict::Lose, "{name}: {event:?} is under the target");
        if let Some(after_end) = end.get("free_total") {
            assert_eq!(card.total.to_bits(), after_end, "{name}: the freeride total put back");
        }
        println!(
            "{name}: {} rows, {ended} event ticks: {exact} rows a tick, {torn} read mid-tick, {still} with no tick, {double} two or more; \
             total {event:?} + saved {} = {:?} ({profile} to the profile), {verdict:?}; read ahead: {:?}",
            gone - join,
            comp.seats[0].saved,
            card.total,
            WORDS.iter().zip(ahead).filter(|(_, n)| *n > 0).collect::<Vec<_>>()
        );
        runs += 1;
    }
    if runs == 0 {
        skip("no C9 capture");
    }
}
