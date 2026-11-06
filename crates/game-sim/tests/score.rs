//! The points (S79), against the original two ways:
//!
//! 1. **the card**, every operation on it run in the interpreter on the human's own score card in
//!    the first recording's RAM — the rider's trick-start, trick-stop, trick-abort and wreck hooks,
//!    the manager's tick, and the push, finish, hold and combo end the rider's tick calls — beside
//!    the port's, on a random sequence of them, and the card compared field for field after each;
//! 2. **the ride**, recording2's playback captured with the card's words in it
//!    (`watch/s79-score.toml`): the port's points driven by the captured bike — its wheels, its air,
//!    its speed, its wreck and its attitude — and by the trick clocks' starts and stops, compared row
//!    for row, and the banks the capture shows asserted by name: two tricks and a backflip, a trick
//!    alone, a backflip out of a trick, a wheelie, a stoppie, and **a trick landed into a wreck that
//!    scores nothing**.

use game_sim::collide::{Bundle, Record};
use game_sim::phy::Phy;
use game_sim::replay;
use game_sim::ride::Controls;
use game_sim::rider::{Rider, Tables};
use game_sim::score::{self, Node, Score, Sense};
use game_sim::trick::{Set, Trick};
use game_sim::world::World;
use game_sim::{Sim, stock};
use ps2_float::{F, Vf};
use std::sync::Arc;
use ps2_oracle::{Ee, Kit, addr, skip};

/// The human's RAM, which has the session's score manager and four cards on it, all empty.
const STATE: &str = "first_recording.p2m2_SaveState.eeMemory.bin";
/// recording2's playback with the score columns (`watch/s79-score.toml`), under `capture/s79/`.
const CAPTURE: &str = "recording2-score.csv";

/// The rider's words the hooks read and write, by offset.
mod at {
    pub const FRONT: u32 = 0x7b0;
    pub const REAR: u32 = 0x7b4;
    pub const CONTACT: u32 = 0xdc;
    pub const WRECK: u32 = 0x748;
    pub const CARD: u32 = 0xf9c;
    pub const MANAGER: u32 = 0xfa0;
    pub const WHEELIE: u32 = 0x13f0;
    pub const STOPPIE: u32 = 0x13f4;
    pub const FLIP: u32 = 0x1400;
}

const BUDGET: u64 = 2_000_000;

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
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}

/// The card as the original holds it, read into the port's shape.
fn card_of(ee: &Ee, rider: u32) -> Score {
    let card = ee.u32_at(rider + at::CARD);
    let flags = ee.u32_at(card + 8);
    let mut nodes = Vec::new();
    let mut n = ee.u32_at(card + 0xc8);
    while n != 0 && nodes.len() < 1000 {
        let f = |o: u32| F(ee.f32_at(n + o));
        nodes.push(Node {
            flags: ee.u32_at(n),
            id: ee.u32_at(n + 4) as u8,
            points: f(0x8),
            base: f(0xc),
            rate: f(0x10),
            mult: f(0x14),
            extra: f(0x18),
            held: f(0x1c),
            granularity: f(0x20),
        });
        n = ee.u32_at(n + 0x24);
    }
    assert_eq!(ee.u32_at(card + 0xcc) as usize, nodes.len(), "the card's count is not its list's length");
    Score {
        nodes,
        combo: flags & 0x20 != 0,
        scoring: flags & 0x40 != 0,
        growing: flags & 0x80 != 0,
        combo_time: F(ee.f32_at(card + 0xd8)),
        total: F(ee.f32_at(card + 0xdc)),
        combo_points: F(ee.f32_at(card + 0xe0)),
        wheelie: F(ee.f32_at(rider + at::WHEELIE)),
        stoppie: F(ee.f32_at(rider + at::STOPPIE)),
        flip: ee.u32_at(rider + at::FLIP) as u8,
        shut: {
            let mgr = ee.u32_at(ee.u32_at(rider + at::MANAGER) + 0x5c);
            mgr & 1 == 0 || mgr & 8 != 0 || flags & 2 == 0
        },
        out: flags & 1 != 0,
        // The start-gate penalty (P158): 0 outside a race.
        gate_penalty: F(ee.f32_at(card + 0x1c0)),
        // The port's own crash lockout (P243): the original has none, and banks at once.
        ..Score::default()
    }
}

/// A float's bits, so that the comparison is of the bits and `-0.0` is not `0.0`.
fn bits(s: &Score) -> String {
    let n: Vec<_> = s
        .nodes
        .iter()
        .map(|n| {
            format!(
                "{:x}/{} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x} {:08x}",
                n.flags,
                n.id,
                n.points.to_bits(),
                n.base.to_bits(),
                n.rate.to_bits(),
                n.mult.to_bits(),
                n.extra.to_bits(),
                n.held.to_bits(),
                n.granularity.to_bits()
            )
        })
        .collect();
    format!(
        "combo {} scoring {} growing {} time {:08x} total {:08x} points {:08x} wheel {:08x}/{:08x} flip {} nodes {n:?}",
        s.combo,
        s.scoring,
        s.growing,
        s.combo_time.to_bits(),
        s.total.to_bits(),
        s.combo_points.to_bits(),
        s.wheelie.to_bits(),
        s.stoppie.to_bits(),
        s.flip
    )
}

/// The interpreter on the human's RAM, the human's rider, and the functions by name.
fn card_machine() -> Option<(Ee, u32, [u32; 10])> {
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
    let names = [
        "rider_score_trick_start",
        "rider_score_trick_stop",
        "rider_score_trick_abort",
        "jump_record_end",
        "score_grow",
        "score_combo_start",
        "score_node_push",
        "score_finish",
        "score_hold",
        "score_combo_end",
    ];
    let mut found = [0; 10];
    for (k, n) in names.iter().enumerate() {
        match addr(n) {
            Ok(a) => found[k] = a,
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    let (gm, gp) = match (addr("game_manager"), addr("boot_gp")) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(why), _) | (_, Err(why)) => {
            skip(&why);
            return None;
        }
    };
    let mut ee = Ee::from_ram(&path).unwrap_or_else(|e| panic!("{e}"));
    ee.set(28, u64::from(gp));
    // The actors' list off the game manager, and the one rider a human drives (controller kind 1).
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
    assert_ne!(ee.u32_at(rider + at::CARD), 0, "the human has no score card");
    assert_eq!(ee.u32_at(ee.u32_at(rider + at::MANAGER) + 0xac), 1.0f32.to_bits(), "the total's granularity");
    Some((ee, rider, found))
}

/// **The card, operation for operation.** 20 runs of 500 random operations on the human's card,
/// each run from the state's empty card: the four hooks, the manager's tick at the frame's dt and
/// at others, a wheelie or stoppie's node, the bank, and the five card functions with random
/// numbers — rounding down, to nearest and not at all, granularities at and below zero, multipliers
/// of every sign. After every operation the original's card, its nodes and the rider's three words
/// are the port's, bit for bit.
#[test]
fn the_card_scores_like_the_original() {
    let Some((fresh, rider, f)) = card_machine() else { return };
    let [start, stop, abort, wreck, grow, open, push, finish, hold, close] = f;
    let card = fresh.u32_at(rider + at::CARD);
    let mgr = fresh.u32_at(rider + at::MANAGER);
    let (front, rear) = (fresh.u32_at(rider + at::FRONT), fresh.u32_at(rider + at::REAR));
    let empty = card_of(&fresh, rider);
    assert_eq!(empty, Score::default(), "the state's card is not empty");
    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    let (runs, ops) = (20, 500);
    let mut seen = [0usize; 15];
    let mut most = 0;
    for run in 0..runs {
        // A fresh machine each run: the interpreter's RAM cannot be copied.
        let (mut ee, _, _) = card_machine().expect("the state went away");
        let mut port = Score::default();
        for op in 0..ops {
            let s = Sense {
                front: pick.below(3) == 0,
                rear: pick.below(3) == 0,
                wrecked: pick.below(8) == 0,
                ..Sense::default()
            };
            ee.set_u32(front + at::CONTACT, u32::from(s.front));
            ee.set_u32(rear + at::CONTACT, u32::from(s.rear));
            ee.set_u32(rider + at::WRECK, if s.wrecked { 12 } else { 0 });
            let kind = pick.below(15) as usize;
            seen[kind] += 1;
            let what = match kind {
                0 | 1 | 2 => {
                    let id = if pick.below(4) == 0 { score::BACKFLIP } else { pick.below(24) as u8 };
                    ee.call(start, &[u64::from(rider), u64::from(id)], &[], BUDGET);
                    port.start(id, &s);
                    format!("start {id} {s:?}")
                }
                3 => {
                    ee.call(stop, &[u64::from(rider)], &[], BUDGET);
                    port.finished();
                    "stop".into()
                }
                4 => {
                    ee.call(abort, &[u64::from(rider)], &[], BUDGET);
                    port.aborted();
                    "abort".into()
                }
                5 => {
                    // The wreck hook zeroes the rider's words; give them something to zero.
                    let (w, st) = (pick.range(0.0, 2.0), pick.range(0.0, 2.0));
                    ee.set_f32(rider + at::WHEELIE, w);
                    ee.set_f32(rider + at::STOPPIE, st);
                    ee.set_u32(rider + at::FLIP, 3);
                    (port.wheelie, port.stoppie, port.flip) = (F(w), F(st), 3);
                    ee.call(wreck, &[u64::from(rider)], &[], BUDGET);
                    port.wrecked();
                    "wreck".into()
                }
                6 | 7 | 8 => {
                    let dt = match pick.below(3) {
                        0 => 0.033_33,
                        1 => 1.0 / 60.0,
                        _ => pick.range(0.0, 0.5),
                    };
                    ee.call(grow, &[u64::from(mgr)], &[dt], BUDGET);
                    port.grow(F(dt));
                    format!("grow {dt}")
                }
                9 => {
                    // A second on one wheel: the rider's tick, by the card functions it calls.
                    let id = if pick.below(2) == 0 { score::WHEELIE } else { score::STOPPIE };
                    let flags = ee.u32_at(card + 8);
                    if flags & 0x20 == 0 {
                        ee.call(open, &[u64::from(mgr), u64::from(card)], &[], BUDGET);
                    }
                    if ee.u32_at(card + 8) & 0x40 == 0 {
                        let args = [u64::from(mgr), u64::from(card), u64::from(id), 0xc];
                        ee.call(push, &args, &[0.0, 0.0, 1000.0, 1.0, 10.0], BUDGET);
                        ee.call(hold, &[u64::from(mgr), u64::from(card), 1], &[], BUDGET);
                    }
                    port.one_wheel(id);
                    format!("one wheel {id}")
                }
                10 => {
                    // Down on both wheels: the rider's tick's bank, by the card functions it calls.
                    if ee.u32_at(card + 8) & 0x40 != 0 {
                        ee.call(finish, &[u64::from(mgr), u64::from(card), 1], &[1.0], BUDGET);
                        ee.call(hold, &[u64::from(mgr), u64::from(card), 0], &[], BUDGET);
                    }
                    if ee.u32_at(card + 8) & 0x20 != 0 {
                        ee.call(close, &[u64::from(mgr), u64::from(card), 1], &[1.0], BUDGET);
                    }
                    if port.scoring {
                        port.finish(F::ONE, true);
                        port.hold(false);
                    }
                    if port.combo {
                        port.close(F::ONE, true);
                    }
                    "bank".into()
                }
                11 => {
                    let v = |p: &mut Pick| match p.below(4) {
                        0 => 0.0,
                        1 => p.range(-50.0, 50.0),
                        2 => p.range(-1.0e5, 1.0e5),
                        _ => (p.below(9) as f32) * 10.0,
                    };
                    let floats = [v(&mut pick), v(&mut pick), v(&mut pick), v(&mut pick), v(&mut pick)];
                    let (id, flags) = (pick.below(27) as u8, pick.below(8) as u32);
                    let args = [u64::from(mgr), u64::from(card), u64::from(id), u64::from(flags)];
                    ee.call(push, &args, &floats, BUDGET);
                    let [b, e, r, m, g] = floats.map(F);
                    port.push(b, e, r, m, g, id, flags);
                    format!("push {floats:?} {id} {flags}")
                }
                12 => {
                    let (mult, ok) = (pick.range(-4.0, 30.0), pick.below(2));
                    ee.call(finish, &[u64::from(mgr), u64::from(card), ok], &[mult], BUDGET);
                    port.finish(F(mult), ok != 0);
                    format!("finish {mult} {ok}")
                }
                13 => {
                    let on = pick.below(2);
                    ee.call(hold, &[u64::from(mgr), u64::from(card), on], &[], BUDGET);
                    port.hold(on != 0);
                    format!("hold {on}")
                }
                _ => {
                    let (scale, ok) = (pick.range(-2.0, 3.0), pick.below(2));
                    ee.call(close, &[u64::from(mgr), u64::from(card), ok], &[scale], BUDGET);
                    port.close(F(scale), ok != 0);
                    format!("close {scale} {ok}")
                }
            };
            let want = card_of(&ee, rider);
            most = most.max(want.nodes.len());
            assert_eq!(bits(&port), bits(&want), "run {run}, operation {op} ({what}): the port's card is not the game's");
        }
    }
    assert!(seen.iter().all(|&n| n > 0), "an operation never ran: {seen:?}");
    println!("score card: {} operations in {runs} runs bit-exact, up to {most} nodes in a combo", runs * ops);
}

/// **The event's gate (S92).** The card's operations as above, with the gate's words thrown about
/// between them: the manager running or not, the card taking part or not, its time out or not, and
/// the backflip watch at 3 (round) or not — the original's combo start refuses a card that is not
/// in a running event or whose time is out, and its trick-start hook opens a combo anyway on a card
/// out of time with a round flip. 20 runs of 500, the card and its gates the port's after each.
#[test]
fn the_event_gate_opens_like_the_original() {
    let Some((fresh, rider, f)) = card_machine() else { return };
    let [start, stop, _, wreck, grow, open, push, finish, hold, close] = f;
    let card = fresh.u32_at(rider + at::CARD);
    let mgr = fresh.u32_at(rider + at::MANAGER);
    let (front, rear) = (fresh.u32_at(rider + at::FRONT), fresh.u32_at(rider + at::REAR));
    let mut pick = Pick(0x51ed_270b_9a3c_e461);
    let (runs, ops) = (20, 500);
    let mut seen = [0usize; 9];
    let (mut opened_out, mut refused) = (0, 0);
    for run in 0..runs {
        let (mut ee, _, _) = card_machine().expect("the state went away");
        let mut port = Score::default();
        assert_eq!(card_of(&ee, rider), port, "the state's card is not an open freeride card");
        for op in 0..ops {
            let s = Sense { front: pick.below(3) == 0, rear: pick.below(3) == 0, ..Sense::default() };
            ee.set_u32(front + at::CONTACT, u32::from(s.front));
            ee.set_u32(rear + at::CONTACT, u32::from(s.rear));
            ee.set_u32(rider + at::WRECK, 0);
            let kind = pick.below(9) as usize;
            seen[kind] += 1;
            let before = port.combo;
            let what = match kind {
                0 | 1 => {
                    // The gate: manager running, card taking part, card out, the flip watch.
                    let (on, part, out, round) =
                        (pick.below(4) != 0, pick.below(4) != 0, pick.below(3) == 0, pick.below(2) == 0);
                    let m = ee.u32_at(mgr + 0x5c) & !1;
                    ee.set_u32(mgr + 0x5c, m | u32::from(on));
                    let c = ee.u32_at(card + 8) & !3;
                    ee.set_u32(card + 8, c | u32::from(out) | (u32::from(part) << 1));
                    let watch = if round { 3 } else { pick.below(3) as u8 };
                    ee.set_u32(rider + at::FLIP, u32::from(watch));
                    (port.shut, port.out, port.flip) = (!on || !part, out, watch);
                    format!("gate running {on} taking part {part} out {out} watch {watch}")
                }
                2 | 3 => {
                    let id = if pick.below(4) == 0 { score::BACKFLIP } else { pick.below(24) as u8 };
                    ee.call(start, &[u64::from(rider), u64::from(id)], &[], BUDGET);
                    port.start(id, &s);
                    if !before && port.combo && port.out {
                        opened_out += 1;
                    }
                    if !before && !port.combo && (port.shut || port.out) {
                        refused += 1;
                    }
                    format!("start {id} {s:?}")
                }
                4 => {
                    ee.call(stop, &[u64::from(rider)], &[], BUDGET);
                    port.finished();
                    "stop".into()
                }
                5 => {
                    // The manager's tick calls its grow only while it runs.
                    if ee.u32_at(mgr + 0x5c) & 1 != 0 {
                        ee.call(grow, &[u64::from(mgr)], &[0.033_33], BUDGET);
                    }
                    port.grow(F(0.033_33));
                    "grow".into()
                }
                6 => {
                    let id = if pick.below(2) == 0 { score::WHEELIE } else { score::STOPPIE };
                    if ee.u32_at(card + 8) & 0x20 == 0 {
                        ee.call(open, &[u64::from(mgr), u64::from(card)], &[], BUDGET);
                    }
                    if ee.u32_at(card + 8) & 0x40 == 0 {
                        let args = [u64::from(mgr), u64::from(card), u64::from(id), 0xc];
                        ee.call(push, &args, &[0.0, 0.0, 1000.0, 1.0, 10.0], BUDGET);
                        ee.call(hold, &[u64::from(mgr), u64::from(card), 1], &[], BUDGET);
                    }
                    port.one_wheel(id);
                    format!("one wheel {id}")
                }
                7 => {
                    if ee.u32_at(card + 8) & 0x40 != 0 {
                        ee.call(finish, &[u64::from(mgr), u64::from(card), 1], &[1.0], BUDGET);
                        ee.call(hold, &[u64::from(mgr), u64::from(card), 0], &[], BUDGET);
                    }
                    if ee.u32_at(card + 8) & 0x20 != 0 {
                        ee.call(close, &[u64::from(mgr), u64::from(card), 1], &[1.0], BUDGET);
                    }
                    if port.scoring {
                        port.finish(F::ONE, true);
                        port.hold(false);
                    }
                    if port.combo {
                        port.close(F::ONE, true);
                    }
                    "bank".into()
                }
                _ => {
                    // The wreck hook zeroes the flip watch; the gate's watch goes with it.
                    ee.call(wreck, &[u64::from(rider)], &[], BUDGET);
                    port.wrecked();
                    "wreck".into()
                }
            };
            let want = card_of(&ee, rider);
            assert_eq!(bits(&port), bits(&want), "run {run}, operation {op} ({what}): the port's card is not the game's");
            assert_eq!((port.shut, port.out), (want.shut, want.out), "run {run}, operation {op} ({what}): the gates");
        }
    }
    assert!(seen.iter().all(|&n| n > 0), "an operation never ran: {seen:?}");
    assert!(opened_out > 20 && refused > 100, "the gate's two cases ran too rarely: {opened_out} opened out of time, {refused} refused");
    println!(
        "event gate: {} operations in {runs} runs bit-exact; {refused} trick starts refused a combo, {opened_out} opened one out of time on a round flip",
        runs * ops
    );
}

/// **recording2's points.** The capture's own bike words drive the port's [`Score`], in the rider
/// tick's order — the trick clocks' starts and stops, the wreck, the rider's scoring section, the
/// manager's tick — and every clean row's card is the port's after the same tick.
///
/// The clocks' events are read off the capture's trick columns: a start where the latched id moves
/// to a trick or its clock goes back, a stop where a trick stops running. Rows the capture read
/// across a frame boundary (the manager's clock moved by other than one tick either side of them)
/// are left out of the row compare and counted.
#[test]
fn recording2s_points() {
    let Ok(kit) = Kit::load() else {
        skip("no kit.toml: recording2's points are unchecked");
        return;
    };
    let path = kit.ctx("capture").join("s79").join(CAPTURE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!(
            "no {} — capture recording2's playback with watch/s79-score.toml (the S79 journal's recipe)",
            path.display()
        ));
        return;
    };
    let cap = replay::Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let col = |name: &str| {
        cap.columns
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("no column {name} in {}", path.display()))
    };
    let cell = |k: usize, c: usize| cap.rows[k][c].as_str();
    let int = |k: usize, c: usize| -> i64 { cell(k, c).parse().unwrap_or_else(|_| panic!("row {k}: {:?}", cell(k, c))) };
    let float = |k: usize, c: usize| -> f32 { cell(k, c).parse().unwrap_or_else(|_| panic!("row {k}: {:?}", cell(k, c))) };
    let [frame, dt, wreck, airborne] = ["frame", "dt", "wreck_reason", "airborne"].map(|n| col(n));
    let [id, running, elapsed] = ["trick_id", "trick_running", "trick_elapsed"].map(|n| col(n));
    let [front, rear, speed, hang, up, fwd] =
        ["front_contact", "rear_contact", "contact_speed", "hang_time", "body_up_y", "body_fwd_y"].map(|n| col(n));
    let [flip, wheelie, stoppie, flags, count, combo_time, total, combo] = [
        "flip_stage",
        "wheelie_time",
        "stoppie_time",
        "card_flags",
        "card_count",
        "card_combo_time",
        "card_total",
        "card_combo",
    ]
    .map(|n| col(n));
    let [node_id, node_points, node_time, clock] = ["node_id", "node_points", "node_time", "mgr_clock"].map(|n| col(n));

    let n = cap.rows.len();
    // PINE reads from its own thread, so a row is read at one of three places in its frame: after
    // the frame (most rows), between the rider's tick and the manager's (the manager's clock and
    // everything its tick writes still one tick back), or before the frame's ticks (the row repeats
    // the last frame, as in the lag runs). A row is compared with the port at all three; the
    // events are taken from the trick columns, which read the same way, so they too can come a row
    // late or early.
    // What the card shows after a tick, as one line per row: the capture's and the port's.
    let shown = |k: usize| {
        let node = if cell(k, node_id).is_empty() {
            "-".to_string()
        } else {
            format!("{}:{}:{}", int(k, node_id), float(k, node_points), float(k, node_time))
        };
        format!(
            "flags {:x} count {} time {} total {} combo {} head {node} flip {} wheel {}/{}",
            int(k, flags) & 0xe0,
            int(k, count),
            float(k, combo_time),
            float(k, total),
            float(k, combo),
            int(k, flip),
            float(k, wheelie),
            float(k, stoppie)
        )
    };
    let ours = |s: &Score| {
        let node = s.nodes.first().map_or("-".to_string(), |h| format!("{}:{}:{}", h.id, h.points.0, h.held.0));
        let f = u32::from(s.combo) * 0x20 | u32::from(s.scoring) * 0x40 | u32::from(s.growing) * 0x80;
        format!(
            "flags {f:x} count {} time {} total {} combo {} head {node} flip {} wheel {}/{}",
            s.nodes.len(),
            s.combo_time.0,
            s.total.0,
            s.combo_points.0,
            s.flip,
            s.wheelie.0,
            s.stoppie.0
        )
    };

    // The first row is taken as the port's starting card: empty in this capture.
    assert_eq!(float(0, total), 0.0, "the capture does not start on an empty card");
    let mut port = Score::default();
    let (mut early, mut late, mut stale, mut torn) = (0, 0, 0, Vec::new());
    let mut banks = Vec::new();
    let mut wrecks = Vec::new();
    for k in 1..n {
        let d = F(float(k, dt));
        let sense = |wrecked: bool| Sense {
            front: int(k, front) != 0,
            rear: int(k, rear) != 0,
            airborne: int(k, airborne) != 0,
            hang_time: F(float(k, hang)),
            slow: float(k - 1, speed) < 5.0,
            wrecked,
            up_y: F(float(k, up)),
            forward_y: F(float(k, fwd)),
        };
        let (was, now) = (int(k - 1, wreck) != 0, int(k, wreck) != 0);
        // The clocks.
        let trick = int(k, id);
        let started = trick < 24 && (trick != int(k - 1, id) || float(k, elapsed) < float(k - 1, elapsed));
        if started && int(k - 1, running) != 0 {
            port.finished();
        }
        if started {
            port.start(trick as u8, &sense(was));
        }
        // A trick's clock stops at its duration and stays there: the frame ticked (the manager's clock
        // moved), the trick's did not, and it never moves again while the trick is up. The running
        // flag only drops a frame or more later. (A row read mid-tick can freeze it for one row.)
        let held = !started
            && trick < 24
            && trick == int(k - 1, id)
            && float(k, elapsed) == float(k - 1, elapsed)
            && float(k, clock) != float(k - 1, clock)
            && (k + 1..n)
                .take_while(|&j| int(j, id) == trick && int(j, running) != 0)
                .all(|j| float(j, elapsed) == float(k, elapsed));
        if int(k - 1, running) != 0 && (held || int(k, running) == 0) && int(k - 1, id) < 24 {
            port.finished();
        }
        // The wreck entry.
        if now && !was {
            wrecks.push((int(k, frame), port.combo_points.0, port.nodes.len(), port.total.0));
            port.wrecked();
        }
        // The rider's tick, and the manager's.
        let before = ours(&port);
        let banked = port.total;
        port.frame(d, &sense(now));
        let mid = ours(&port);
        port.grow(d);
        if port.total != banked {
            banks.push((int(k, frame), port.total.0));
        }
        let want = shown(k);
        match () {
            _ if want == ours(&port) => early += 1,
            _ if want == mid => late += 1,
            _ if want == before => stale += 1,
            _ => torn.push(format!("frame {}:\n  game {want}\n  port {}\n  mid  {mid}", int(k, frame), ours(&port))),
        }
    }
    println!(
        "recording2's points: {early} rows the port's after their frame, {late} between the rider's and the manager's tick, \
         {stale} before the frame, {} torn:\n{}",
        torn.len(),
        torn.join("\n")
    );
    println!("banks (frame, total): {banks:?}");
    println!("wrecks (frame, combo points, nodes, total) as they came: {wrecks:?}");
    // Three: two read inside one of the card's functions (a node's first push at 2543, before its
    // hold; the combo's close at 2595, before the nodes are freed), and the trick stop at 2593
    // taken a row early off the trick clock. No more than that.
    assert!(torn.len() <= 4, "{} rows match the port at neither phase:\n{}", torn.len(), torn.join("\n"));
    assert_eq!(early + late + stale + torn.len(), n - 1);
    assert!(late > 0 && early > 1800, "{early} rows after their tick, {late} between");
    // The banks, frame and total: a trick and a backflip off it (the flip times the combo's two
    // nodes); a trick alone; a trick then a backflip landed on the rear wheel only, slow; a wheelie
    // on its own; a trick; a trick through a one-wheel touch and back into the air.
    assert_eq!(
        banks,
        [(2595, 24160.0), (2716, 27900.0), (3417, 55060.0), (3452, 55190.0), (3869, 60020.0), (4034, 64350.0)]
    );
    // The wrecks: two with nothing in the air, and at 3562 a trick landed into a wreck with 3330
    // points on it — which the total never sees (55190 before, 55190 at the next bank's base).
    assert_eq!(wrecks, [(3015, 0.0, 0, 27900.0), (3562, 3330.0, 1, 55190.0), (4124, 0.0, 0, 64350.0)]);
}

/// The same trick twice, no data needed: landed on both wheels it banks its points, landed into a
/// wreck it scores nothing, and the combo's points show zero, not the lost points.
#[test]
fn a_trick_landed_into_a_wreck_scores_nothing() {
    let dt = F(0.03333);
    let air = Sense { airborne: true, hang_time: F(0.5), up_y: F(1.0), ..Sense::default() };
    let ride = |s: &mut Score, ticks: usize| {
        s.start(0, &air);
        for _ in 0..ticks {
            s.frame(dt, &air);
            s.grow(dt);
        }
        s.finished();
        assert!(s.combo_points > F::ZERO, "the trick grew no points");
    };
    let landed = Sense { front: true, rear: true, up_y: F(1.0), ..Sense::default() };
    let mut clean = Score::default();
    ride(&mut clean, 30);
    let points = clean.combo_points;
    clean.frame(dt, &landed);
    clean.grow(dt);
    assert_eq!(clean.total, points, "a clean landing banks the combo");

    let mut crashed = Score::default();
    ride(&mut crashed, 30);
    crashed.wrecked();
    let wreck = Sense { front: true, rear: true, wrecked: true, up_y: F(1.0), ..Sense::default() };
    for _ in 0..60 {
        crashed.frame(dt, &wreck);
        crashed.grow(dt);
    }
    assert_eq!(crashed.total, F::ZERO, "a trick landed into a wreck kept its points");
    assert_eq!(crashed.combo_points, F::ZERO);
    assert!(crashed.nodes.is_empty() && !crashed.combo);
}

// ---------------------------------------------------------------------------------------------------
// S250: a flip scores when the watch comes round, as the original's does
// ---------------------------------------------------------------------------------------------------
//
// S234 put a flip on the line the frame the bike started turning it over; the user had it taken out
// again (2026-11-02). The original's rider tick (./kit fn 0x27c890) calls the trick-start hook with
// 0x18 only once the attitude watch (+0x1400, `Score::flip`) reaches 3 — the bike over and back the
// right way up, no more than 0.79 nose down — so a flip landed before then scores nothing, as C8's
// film shows. The app's riders have P243's lockout up, so these cards do too.

const DT: F = F(0.03333);

/// In the air at a pitch `deg` turned back (a backflip's way) from level.
fn aloft(deg: f32) -> Sense {
    let r = deg.to_radians();
    Sense { airborne: true, hang_time: F(0.5), up_y: F(r.cos()), forward_y: F(r.sin()), ..Sense::default() }
}

/// Down on both wheels at a pitch `deg`.
fn landed_at(deg: f32) -> Sense {
    let r = deg.to_radians();
    Sense { front: true, rear: true, up_y: F(r.cos()), forward_y: F(r.sin()), ..Sense::default() }
}

/// A backflip turned from level through `to` degrees, 10 a tick, landed at `to` and ridden out past
/// the lockout's window: the card, the tick the flip first showed on the line, and the tick the
/// watch first read 3 (counted from the first turned tick).
fn backflip(to: f32) -> (Score, Option<usize>, Option<usize>) {
    let mut s = Score { lockout: true, ..Score::default() };
    let (mut shown, mut round) = (None, None);
    let mut k = 0;
    let mut deg = 0.0f32;
    let mut sense = aloft(0.0);
    loop {
        s.frame(DT, &sense);
        s.grow(DT);
        if shown.is_none() && s.nodes.iter().any(|n| n.id == score::BACKFLIP) {
            shown = Some(k);
        }
        if round.is_none() && s.flip == 3 {
            round = Some(k);
        }
        if deg >= to {
            break;
        }
        k += 1;
        deg = (deg + 10.0).min(to);
        sense = aloft(deg);
    }
    for _ in 0..90 {
        s.frame(DT, &landed_at(to));
        s.grow(DT);
    }
    (s, shown, round)
}

/// Landed at 300° (60° nose down) the watch never came round: nothing on the line, nothing banked —
/// C8's card. A full 360 shows the flip the tick after the watch reads 3 (the rider's tick asks
/// before it steps the watch, as the original's does), and banks 10 000 once.
#[test]
fn a_flip_scores_when_the_watch_comes_round() {
    let (s, shown, round) = backflip(300.0);
    assert_eq!((shown, round), (None, None), "a flip landed at 300° went on the line");
    assert_eq!(s.total, F::ZERO, "a flip landed at 300° banked {}", s.total.0);

    let (s, shown, round) = backflip(360.0);
    let round = round.expect("the watch never came round on a full flip");
    assert_eq!(shown, Some(round + 1), "the flip went on the line before the watch came round");
    assert!(round > 30, "the watch came round at tick {round}, before the bike had turned over");
    assert_eq!(s.total, score::BACKFLIP_POINTS, "a full flip banked {}", s.total.0);
}

/// The air's numbers and wreck limits past a full turn, as `tests/rotation_modifier.rs` has them —
/// the test's own, not the disc's.
const AIR: &str = "
[Airborne Control]
AirborneDamping = 400
AirborneControlForce = 40
AirborneControlMoment = -400.0, 1500.0, -100.0
BrakeGyroTorque = 300
UnrollGain = 3
UnrollTorque = 50
UnyawGain = 3
UnyawTorque = 400
[Backflip]
MaxBackflipTorque = 1400
MinBackflipTorque = 800
BackflipTorqueRateIncrease = 500
BackflipTorqueRateDecrease = 2000
MinBackflipTime = 0
MaxBackflipTime = 0
[Wrecks]
MaxWreckTime = 2
CannonballWreckTime = 5
FrontWreckSpeedLimit = 400
BackWreckSpeedLimit = 400
SideWreckSpeedLimit = 400
RacingDownWreckSpeedLimit = 40
FreestyleDownWreckSpeedLimit = 400
GroundPitchDownWreckAngle = 60
GroundPitchUpWreckAngle = 60
AirbornePitchDownWreckAngle = 400
AirbornePitchUpWreckAngle = 400
AirborneRollWreckAngle = 400
NoHanderWreckSpeedLimit = 400
NoHanderMaxRollAngle = 45
JoustWreckHeight = 2
CrossupWreck = 1
CrossupWreckTime = 0.3
";

/// Flat ground up to a ramp of slope 1/2, then a drop to flat ground [`DROP`] ft below.
struct Jump;

const RAMP_AT: f32 = 108.0;
const RAMP_LONG: f32 = 12.0;
const DROP: f32 = 60.0;

impl World for Jump {
    fn ground(&mut self, at: Vf) -> (Vf, Vf) {
        let t = at.z() - RAMP_AT;
        if t < 0.0 {
            (Vf::new(at.x(), 0.0, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
        } else if t <= RAMP_LONG {
            let n = 1.0 / (1.0f32 + 0.25).sqrt();
            (Vf::new(at.x(), t * 0.5, at.z(), 1.0), Vf::new(0.0, n, -0.5 * n, 0.0))
        } else {
            (Vf::new(at.x(), -DROP, at.z(), 1.0), Vf::new(0.0, 1.0, 0.0, 0.0))
        }
    }

    fn solid(&mut self, _bundle: &Bundle, _record: &mut Record) -> bool {
        false
    }
}

/// The stand-in bike with [`AIR`] and a drag low enough to make the lip's 30 ft/s.
fn sim() -> Sim {
    let tables = Arc::new(Tables::build());
    let mut text = stock::BIKE.to_string();
    assert!(text.contains("Drag = 0.5"), "game_sim::stock::BIKE no longer has `Drag = 0.5`");
    text = text.replace("Drag = 0.5", "Drag = 0.037");
    let phy = Phy::read_bytes(format!("{text}{AIR}").as_bytes(), &tables.ride.curve);
    let geom = Phy::read_bytes(stock::BIKE.as_bytes(), &tables.ride.curve);
    let rider = Rider::new(&phy, stock::mounts(&geom), stock::SPAWN, stock::FACING, Set::default());
    let mut sim = Sim::with_rider(0, tables, phy, rider);
    sim.rider.sense.wrecks_enabled = true;
    sim
}

/// One ride off [`Jump`] flat out with the backflip's gesture on the lip, the app's lockout up. The
/// ticks the bike first turned a flip over, the watch first read 3, the line first had the flip, and
/// a backflip's start was told; the total ridden out.
fn flip_ride() -> ([Option<usize>; 4], F) {
    let mut s = sim();
    s.rider.score.lockout = true;
    let (mut started, mut round, mut shown, mut told) = (None, None, None, None);
    let (mut lip_at, mut aloft, mut landed) = (None, false, false);
    for tick in 0..1200 {
        let ride = &s.rider.ride;
        let lip = ride.air.on_ground && ride.chassis.vel.y() > 5.0 && ride.air.contact_vel.z() > 30.0;
        if lip_at.is_none() && lip {
            lip_at = Some(tick);
        }
        let y = match lip_at {
            Some(l) if tick < l + 2 => -1.0,
            Some(_) if !aloft || s.rider.ride.air.flipping => 1.0,
            _ => 0.0,
        };
        let c = Controls { gas: F::ONE, stick: (F::ZERO, F(y)), ..Controls::default() };
        let frame = s.step(DT, &c, &mut Jump);
        assert!(frame.wreck.is_none(), "the flip wrecked at tick {tick}");
        if frame.started == Some(Trick::BACKFLIP) && told.is_none() {
            told = Some(tick);
        }
        if s.rider.ride.air.flipping && started.is_none() {
            started = Some(tick);
        }
        if s.rider.score.flip == 3 && round.is_none() {
            round = Some(tick);
        }
        if shown.is_none() && s.rider.score.nodes.iter().any(|n| n.id == score::BACKFLIP) {
            shown = Some(tick);
        }
        aloft |= !s.rider.ride.air.on_ground && started.is_some();
        landed |= aloft && s.rider.ride.air.on_ground;
        // Ridden out past the landing and its lockout: banked or not by now.
        if landed && s.rider.ride.air.on_ground && !s.rider.score.combo && tick > lip_at.unwrap_or(0) + 200 {
            break;
        }
    }
    ([started, round, shown, told], s.rider.score.total)
}

/// On a whole ride off a ramp the line has no flip while the bike turns it over: it goes on the tick
/// after the watch comes round, never at the start, and is never told as a start; landed, it banks
/// 10 000.
#[test]
fn a_ridden_flip_shows_only_once_the_watch_comes_round() {
    let ([started, round, shown, told], total) = flip_ride();
    println!("started {started:?}, round {round:?}, shown {shown:?}, told {told:?}, total {}", total.0);
    let started = started.expect("the bike never turned a flip over");
    let round = round.expect("the watch never came round");
    assert!(started + 10 < round, "the watch came round at {round}, the flip started at {started}");
    assert_eq!(shown, Some(round + 1), "the flip went on the line at {shown:?}, not after the watch");
    assert_eq!(told, None, "a backflip's start was told: the original's rider tells none");
    assert_eq!(total, score::BACKFLIP_POINTS, "the landed flip banked {}", total.0);
}

/// What one of R35's captures (`capture/r35/`, research/r35's tapes: recording2's first backflip with
/// its torque bled off by a forward stick from a chosen field) did, and what the port's card did
/// beside it, driven by the captured wheels, air, attitude, wreck and trick clocks as
/// [`recording2s_points`] drives it.
struct Partial {
    rows: usize,
    /// Rows that match the port at none of the three read phases.
    torn: Vec<String>,
    /// The highest backflip watch stage: the capture's, the port's.
    stage: (i64, u8),
    /// The banks, (frame, total).
    banks: Vec<(i64, f32)>,
    /// Did a backflip node (24) ever go on the card: the capture's, the port's.
    flipped: (bool, bool),
}

fn partial(name: &str) -> Option<Partial> {
    let Ok(kit) = Kit::load() else {
        skip("no kit.toml: R35's partial backflips are unchecked");
        return None;
    };
    let path = kit.ctx("capture").join("r35").join(name);
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {} — play research/r35's tape for it (research/r35/README.md)", path.display()));
        return None;
    };
    let cap = replay::Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let col = |name: &str| {
        cap.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no column {name} in {}", path.display()))
    };
    let cell = |k: usize, c: usize| cap.rows[k][c].as_str();
    let int = |k: usize, c: usize| -> i64 { cell(k, c).parse().unwrap_or_else(|_| panic!("row {k}: {:?}", cell(k, c))) };
    let float = |k: usize, c: usize| -> f32 { cell(k, c).parse().unwrap_or_else(|_| panic!("row {k}: {:?}", cell(k, c))) };
    let [frame, dt, wreck, airborne] = ["frame", "dt", "wreck_reason", "airborne"].map(|n| col(n));
    let [id, running, elapsed] = ["trick_id", "trick_running", "trick_elapsed"].map(|n| col(n));
    let [front, rear, speed, hang, up, fwd] =
        ["front_contact", "rear_contact", "contact_speed", "hang_time", "body_up_y", "body_fwd_y"].map(|n| col(n));
    let [flip, flags, count, total, combo] =
        ["flip_stage", "card_flags", "card_count", "card_total", "card_combo"].map(|n| col(n));
    let [node_id, node_points, clock] = ["node_id", "node_points", "mgr_clock"].map(|n| col(n));
    let n = cap.rows.len();
    for k in 1..n {
        assert_eq!(int(k, frame), int(k - 1, frame) + 1, "{}: a gap before row {k}: re-record it slower", path.display());
    }
    let shown = |k: usize| {
        let node =
            if cell(k, node_id).is_empty() { "-".to_string() } else { format!("{}:{}", int(k, node_id), float(k, node_points)) };
        format!(
            "flags {:x} count {} total {} combo {} head {node} flip {}",
            int(k, flags) & 0xe0,
            int(k, count),
            float(k, total),
            float(k, combo),
            int(k, flip)
        )
    };
    let ours = |s: &Score| {
        let node = s.nodes.first().map_or("-".to_string(), |h| format!("{}:{}", h.id, h.points.0));
        let f = u32::from(s.combo) * 0x20 | u32::from(s.scoring) * 0x40 | u32::from(s.growing) * 0x80;
        format!(
            "flags {f:x} count {} total {} combo {} head {node} flip {}",
            s.nodes.len(),
            s.total.0,
            s.combo_points.0,
            s.flip
        )
    };
    assert_eq!(float(0, total), 0.0, "{}: the capture does not start on an empty card", path.display());
    let mut port = Score::default();
    let mut out = Partial { rows: n, torn: Vec::new(), stage: (0, 0), banks: Vec::new(), flipped: (false, false) };
    for k in 1..n {
        let d = F(float(k, dt));
        let sense = |wrecked: bool| Sense {
            front: int(k, front) != 0,
            rear: int(k, rear) != 0,
            airborne: int(k, airborne) != 0,
            hang_time: F(float(k, hang)),
            slow: float(k - 1, speed) < 5.0,
            wrecked,
            up_y: F(float(k, up)),
            forward_y: F(float(k, fwd)),
        };
        let (was, now) = (int(k - 1, wreck) != 0, int(k, wreck) != 0);
        // The trick clocks, read off the trick columns as recording2s_points reads them.
        let trick = int(k, id);
        let started = trick < 24 && (trick != int(k - 1, id) || float(k, elapsed) < float(k - 1, elapsed));
        if started && int(k - 1, running) != 0 {
            port.finished();
        }
        if started {
            port.start(trick as u8, &sense(was));
        }
        let held = !started
            && trick < 24
            && trick == int(k - 1, id)
            && float(k, elapsed) == float(k - 1, elapsed)
            && float(k, clock) != float(k - 1, clock)
            && (k + 1..n)
                .take_while(|&j| int(j, id) == trick && int(j, running) != 0)
                .all(|j| float(j, elapsed) == float(k, elapsed));
        if int(k - 1, running) != 0 && (held || int(k, running) == 0) && int(k - 1, id) < 24 {
            port.finished();
        }
        if now && !was {
            port.wrecked();
        }
        let before = ours(&port);
        let banked = port.total;
        port.frame(d, &sense(now));
        let mid = ours(&port);
        port.grow(d);
        if port.total != banked {
            out.banks.push((int(k, frame), port.total.0));
        }
        out.stage = (out.stage.0.max(int(k, flip)), out.stage.1.max(port.flip));
        out.flipped.0 |= !cell(k, node_id).is_empty() && int(k, node_id) == i64::from(score::BACKFLIP);
        out.flipped.1 |= port.nodes.iter().any(|n| n.id == score::BACKFLIP);
        let want = shown(k);
        if want != ours(&port) && want != mid && want != before {
            out.torn.push(format!("frame {}:\n  game {want}\n  port {}\n  mid  {mid}", int(k, frame), ours(&port)));
        }
    }
    Some(out)
}

/// R35 (the user, 2026-11-05: 'a backflip stopped before its actually a backflip gives no points,
/// check what the original does'): it gives none either. recording2's first backflip, its torque
/// bled off by a forward stick from field 272 (the bike tips back but never past level), 300 (the
/// watch reaches 1, the bike comes back the right way up in the air and the watch drops to 0, then
/// it lands into a wreck) and 330 (the watch reaches 2, upside down, and it lands on its back into a
/// wreck): the original's card has no backflip node, no combo points and banks nothing from it, and
/// the port's card is the original's row for row. Beside them the flip left to come round (field
/// 360, the human's Taunt kept) banks recording2's 24 160, at 2605 rather than recording2's 2595: the
/// stick pushed from field 360 brings it down later.
#[test]
fn a_backflip_stopped_short_scores_nothing_as_the_original() {
    // (capture, the watch's highest stage, the banks)
    let cases: [(&str, i64, &[(i64, f32)]); 4] = [
        ("stage0.csv", 0, &[]),
        ("stage1.csv", 1, &[]),
        ("stage2.csv", 2, &[]),
        ("round.csv", 3, &[(2605, 24160.0)]),
    ];
    for (name, stage, banks) in cases {
        let Some(p) = partial(name) else { continue };
        println!("{name}: {} rows, stage {:?}, banks {:?}, {} torn:\n{}", p.rows, p.stage, p.banks, p.torn.len(), p.torn.join("\n"));
        // Rows read inside one of the card's functions (a push before its hold, a close before the
        // nodes are freed) or a trick stop taken a row early off its clock, as in recording2s_points.
        assert!(p.torn.len() <= 3, "{name}: {} rows match the port at no phase:\n{}", p.torn.len(), p.torn.join("\n"));
        assert_eq!(p.stage, (stage, stage as u8), "{name}: the watch's highest stage, the game's and the port's");
        assert_eq!(p.flipped, (stage == 3, stage == 3), "{name}: a backflip went on the card (game, port)");
        // Stage 0's ride lands clean; its later cancan banks only past the capture's end.
        assert_eq!(p.banks, banks, "{name}: the banks");
    }
}
