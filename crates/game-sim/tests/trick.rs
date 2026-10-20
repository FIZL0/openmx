//! The trick machine against the original.
//!
//! Ten proofs: eight covering each of the four pieces, one (P3) running the whole machine on a bike,
//! and one (S30) running it on a human's recording against the original's own fields.
//!
//! 1. **the table**, read off the disc through the same archive reader the app layer uses, so the 57
//!    records and the default gesture map are the ones the game ships rather than ones typed in here.
//! 2. **the arc-cosine table**, all 30,001 entries, against the game's own double-precision chain run
//!    in the oracle: float to double, its software `acos`, and back to a single. The lookup is read by
//!    ~70 places in the image, so this is the entry it all stands on.
//! 3. **the stick classifier**, over ten thousand generated deflections, against the original's own
//!    function with the port's table injected into its global — so a last-bit difference in the table
//!    could not hide a difference in the decode.
//! 4. **the request machine**: every combination of the gate's six conditions and the switch's thirty
//!    five group-and-direction pairs, then ten thousand generated overlap triples through the branch
//!    that lets a queued trick take over the end of the running one.
//! 5. **the AI's choice**, over ten thousand air times and generated tables, with the original's own
//!    generator stepped alongside the port's.
//! 6. **the clocks**, over ten thousand generated frames: every field the per-frame machine writes,
//!    bit for bit, plus which of the motion library's three entry points it called and with what. The
//!    original's clock section ends in animation work this task does not own, so the call is stopped
//!    at the join where that begins and the rider's vtable is a table of hooked addresses.
//! 7. **the landing**, every combination of the clause's six inputs against the original's own test,
//!    with the attitude limits in front of it set where nothing can reach them.
//! 8. **the idle request**, every state of the three flags that decide what a rider with no gesture
//!    asks for.
//! 9. **the machine on a bike**, which is not an oracle proof but a fixture: a scripted pad over the
//!    ported ride, landing one trick and wrecking another. It proves the *wiring* P3 added — that a
//!    gesture reaches the request, the clocks run on the stunt the disc's table names, and the
//!    verdict reaches the ride state machine — and nothing about the clauses themselves.
//! 10. **the human's tricks**: recording2's pad through the gesture, the request and the clocks,
//!    driven by the capture's own ground and wreck columns and compared with the original's trick
//!    fields on every tick of the playback — and, for every gesture in it, whether the shell's
//!    keyboard can make it.
//!
//! Three things the original does that these stand in for: the queue's heap nodes, the library's
//! `strcmp` (hand-written with quadword compares the interpreter does not implement), and the engine's
//! shared smooth noise. What is not proved here, and why, is in the journal entry's last section.

use game_sim::archive::{Psr, Res};
use game_sim::input::{Control, Report};
use game_sim::phy::Phy;
use game_sim::ride::Controls;
use game_sim::rider;
use game_sim::replay;
use game_sim::rng::Rng;
use game_sim::trick::{
    AngleTable, Bike, Dir, Params, SLOTS, STUNTS, Set, Stunt, Stunts, Trick, Tricks, look, slot_of,
    stick,
};
use game_sim::world::Flat;
use ps2_float::{F, Vf, fpu};
use ps2_oracle::{DONE, Ee, Kit, addr, game_or_skip, skip};
use std::cell::Cell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

/// Instruction budget per call. The software `acos` is the long one at about nine thousand.
const BUDGET: u64 = 200_000;

/// The archive the table lives in, and its name inside it.
const ARCHIVE: &str = "appstart.res";
const TABLE: &str = "rider\\anim\\stunts.ini";

/// Where this test puts things in the oracle's RAM: well above the program image, and above the heap
/// the original would have allocated from had anything started it.
const OBJ: u32 = 0x0100_0000;
const RECORDS: u32 = 0x0104_0000;
const VALUES: u32 = 0x0110_0000;
/// The rider's own fields, by the offsets the original uses. Named here because a test may say where
/// things live; the port may not.
const GROUND: u32 = 0x574;
const CONTACTS: u32 = 0x7d0;
const FRESH: u32 = 0x750;
const GROUP: u32 = 0x9fc;
const DIR: u32 = 0xa00;
const SEQ: u32 = 0xa04;
const IN_PROGRESS: u32 = 0xa0c;
const ARMED: u32 = 0xa18;
const ELAPSED: u32 = 0xa1c;
const OVERLAP: u32 = 0xa20;
const STUNT_ARRAY: u32 = 0xa34;
const QUEUE: u32 = 0xa38;
const CURRENT: u32 = 0xa78;
const REQUESTED: u32 = 0xa7c;
const RESTART: u32 = 0xaf8;
/// Inside one 0x54-byte stunt record.
const RECORD: u32 = 0x54;
const DURATION: u32 = 0x48;

// ---------------------------------------------------------------------------------------------------
// the disc
// ---------------------------------------------------------------------------------------------------

fn container() -> Option<File> {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => {
            skip(&why);
            return None;
        }
    };
    let at = kit.ctx("iso").join("MX.PSR");
    match File::open(&at) {
        Ok(f) => Some(f),
        Err(_) => {
            skip(&format!("no {} — `./kit extract MX.PSR` writes it", at.display()));
            None
        }
    }
}

/// The table as the disc stores it: one member of one archive, inflated the way the game does it.
fn table_bytes() -> Option<Vec<u8>> {
    let mut f = container()?;
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    let e = psr.find(ARCHIVE).unwrap_or_else(|| panic!("{ARCHIVE} is not in MX.PSR"));
    f.seek(SeekFrom::Start(e.at)).expect("seek");
    let mut archive = vec![0u8; e.size as usize];
    f.read_exact(&mut archive).expect("read");
    let res = Res::parse(&archive).unwrap_or_else(|why| panic!("{ARCHIVE}: {why}"));
    let member = res
        .entries
        .iter()
        .find(|m| m.name.eq_ignore_ascii_case(TABLE))
        .unwrap_or_else(|| panic!("{TABLE} is not in {ARCHIVE}"));
    Some(res.read(&archive, member).unwrap_or_else(|why| panic!("{TABLE}: {why}")))
}

// ---------------------------------------------------------------------------------------------------
// the oracle
// ---------------------------------------------------------------------------------------------------

/// Resolve names from `research/oracle.txt`, or say which one is missing and skip.
fn named(names: &[&str]) -> Option<Vec<u32>> {
    let mut out = Vec::new();
    for n in names {
        match addr(n) {
            Ok(a) => out.push(a),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    Some(out)
}

/// A tiny xorshift, so every generated input is the same on every machine and a failure is
/// reproducible from the seed in this file alone.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    /// A float in `-span..=span`, with the exponent spread so small and large magnitudes both appear.
    fn float(&mut self, span: f32) -> f32 {
        let r = self.next();
        let unit = (r >> 8) as f32 / (1u32 << 24) as f32;
        let sign = if r & 1 == 0 { 1.0 } else { -1.0 };
        sign * span * unit
    }
    fn upto(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// Put the port's angle table where the original's lookup reads it from, with the scale the builder
/// stores — which is the whole of the original's start-up as far as the lookup is concerned.
fn inject_table(ee: &mut Ee, table: &AngleTable, ptr: u32, scale: u32) {
    let mut bytes = Vec::with_capacity(AngleTable::LEN * 4);
    for v in table.values() {
        bytes.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    ee.load(VALUES, &bytes);
    ee.set_u32(ptr, VALUES);
    ee.set_u32(scale, F(AngleTable::SCALE).to_bits());
}

/// Lay 24 stunt records out in RAM the way the rider's own copy is laid out, and point the object's
/// array field at them.
fn write_records(ee: &mut Ee, obj: u32, stunts: &[Stunt]) {
    for (k, s) in stunts.iter().enumerate() {
        let at = RECORDS + k as u32 * RECORD;
        let mut name = [0u8; 0x20];
        let bytes = s.name.as_bytes();
        name[..bytes.len().min(0x1f)].copy_from_slice(&bytes[..bytes.len().min(0x1f)]);
        ee.load(at, &name);
        ee.set_f32(at + 0x40, s.safe_time.to_f32());
        ee.set_f32(at + 0x44, s.hold_time.to_f32());
        ee.set_f32(at + DURATION, s.duration.to_f32());
        ee.set_u32(at + 0x4c, u32::from(s.for_bike));
        ee.set_u32(at + 0x50, s.index as u32);
    }
    ee.set_u32(obj + STUNT_ARRAY, RECORDS);
}

// ---------------------------------------------------------------------------------------------------
// 1 — the table on the disc
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_stunt_table_is_the_one_on_the_disc() {
    let Some(bytes) = table_bytes() else { return };
    let stunts = Stunts::parse(&bytes).expect("stunts.ini");
    assert_eq!(stunts.stunts.len(), STUNTS);

    // The first 24 sections are the gesture order, four directions per button group, which is the
    // whole answer to "which gesture starts which trick" for a player who has not remapped anything.
    let gestures = [
        (1, Dir::Up, "taunt"),
        (1, Dir::Down, "lazyboy"),
        (1, Dir::Left, "cancan"),
        (1, Dir::Right, "catwalk"),
        (2, Dir::Up, "nohandedlander"),
        (2, Dir::Down, "pendulum"),
        (2, Dir::Left, "nacnac"),
        (2, Dir::Right, "flatliner"),
        (3, Dir::Up, "hartattack"),
        (3, Dir::Down, "indianair"),
        (3, Dir::Left, "barhop"),
        (3, Dir::Right, "supercan"),
        (4, Dir::Up, "kod"),
        (4, Dir::Down, "superman"),
        (4, Dir::Left, "saranwrap"),
        (4, Dir::Right, "stalelin"),
        (5, Dir::Up, "tsunami"),
        (5, Dir::Down, "onehandindianair"),
        (5, Dir::Left, "shoalin"),
        (5, Dir::Right, "cordova"),
        (6, Dir::Up, "deadbody"),
        (6, Dir::Down, "rigamortis"),
        (6, Dir::Left, "supercanindianair"),
        (6, Dir::Right, "stripper"),
    ];
    let set = Set::new(stunts.selection(0));
    for (group, dir, name) in gestures {
        let slot = slot_of(group, dir).expect("a group and a direction name a slot");
        let stunt = set.stunt(Trick(slot as u8)).expect("a slot is a stunt");
        assert!(
            stunt.name.eq_ignore_ascii_case(name),
            "group {group} + {dir:?} is slot {slot}, which the disc says is {:?}, not {name:?}",
            stunt.name
        );
    }

    // The two the rest of the machine looks for by name. The no-hander is in the default 24 — it is
    // the one trick that becomes safe at its hold time, and the one with a landing speed limit — and
    // the throw-away is not, so by default no gesture can throw the rider off on purpose.
    assert_eq!(set.no_hander, 4);
    assert_eq!(set.throwaway, Set::ABSENT);
    assert_eq!(stunts.stunts[30].name, "throwaway");
    assert_eq!(stunts.stunts[56].name, "backflip");
    // One record's clocks in full, so the three keys cannot be read into the wrong fields: the
    // no-hander holds at a third of a second, is safe at one, and is over at one. The `0.33` is the
    // loader's, which truncates to the single below the nearest one (S30).
    let no_hander = &set.stunts[set.no_hander];
    assert_eq!(no_hander.display_name, "No Hander");
    assert_eq!(no_hander.hold_time.to_bits(), 0.33f32.to_bits() - 1);
    assert_eq!(no_hander.safe_time, F::ONE);
    assert_eq!(no_hander.duration, F::ONE);
    assert!(!no_hander.for_bike, "the rider leaves the bike alone for this one");
    // A stunt the file gives no UsedForBike line takes the loader's default of true.
    assert!(stunts.stunts[0].for_bike);

    // Every record is a stunt: a name, a duration a clock can divide by, and its own index.
    for (k, s) in stunts.stunts.iter().enumerate() {
        assert!(!s.name.is_empty(), "stunt {} has no name", k + 1);
        assert!(s.duration > F::ZERO, "stunt {} has no duration", k + 1);
        assert_eq!(s.index, k as i32);
        assert!(s.safe_time <= s.duration, "stunt {} is never safe", k + 1);
    }

    // Both players start on the identity map, so a rider's 24 are the table's first 24 in order.
    for player in 0..2 {
        assert_eq!(stunts.map(player), &Stunts::IDENTITY);
        let theirs = Set::new(stunts.selection(player));
        assert_eq!(theirs.stunts.len(), SLOTS);
        for (slot, s) in theirs.stunts.iter().enumerate() {
            assert_eq!(s, &stunts.stunts[slot]);
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// 2 — the shared arc-cosine table
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_angle_table_is_the_games_own() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["float_to_double", "acos_double", "double_to_float"]) else {
        return;
    };
    let (f2d, acos, d2f) = (at[0], at[1], at[2]);

    let table = AngleTable::build();
    let mut bad = 0usize;
    let mut first = None;
    for i in 0..AngleTable::GRID {
        // The grid the builder walks, including its clamp at the top end.
        let mut x = F(-1.0) + F(fpu::from_i32(i as i32)) * F(AngleTable::STEP);
        if !(x <= F::ONE) {
            x = F::ONE;
        }
        ee.call(f2d, &[], &[x.to_f32()], BUDGET);
        let wide = ee.v0();
        ee.call(acos, &[wide], &[], BUDGET);
        let angle = ee.v0();
        ee.call(d2f, &[angle], &[], BUDGET);
        let want = ee.f0();
        let got = table.values()[i].to_f32();
        if want.to_bits() != got.to_bits() {
            bad += 1;
            first.get_or_insert((i, x.to_f32(), want, got));
        }
    }
    assert_eq!(
        table.values()[AngleTable::GRID].to_bits(),
        table.values()[AngleTable::GRID - 1].to_bits(),
        "the last entry is a copy of the one below it"
    );
    if let Some((i, x, want, got)) = first {
        panic!(
            "{bad} of {} entries differ. First at {i}: acos({x}) is {want:e} ({:#010x}) in the \
             game and {got:e} ({:#010x}) in the port",
            AngleTable::GRID,
            want.to_bits(),
            got.to_bits()
        );
    }
    println!("the angle table: {} entries, bit-exact", AngleTable::GRID);
}

// ---------------------------------------------------------------------------------------------------
// 3 — the stick classifier
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_stick_classifier_matches_over_ten_thousand_deflections() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["stick_direction", "angle_table_ptr", "angle_table_scale"]) else {
        return;
    };
    let (classify, ptr, scale) = (at[0], at[1], at[2]);

    let table = AngleTable::build();
    inject_table(&mut ee, &table, ptr, scale);

    // The corners first: centre, the four axes, the four exact diagonals, and a deflection past the
    // edge of the stick's range — then generated ones, over and under the dead zone's radius.
    let d = 0.707_106_77_f32;
    let mut cases: Vec<(f32, f32)> = vec![
        (0.0, 0.0),
        (1.0, 0.0),
        (-1.0, 0.0),
        (0.0, 1.0),
        (0.0, -1.0),
        (d, d),
        (d, -d),
        (-d, d),
        (-d, -d),
        (0.5, 0.0),
        (-0.5, 0.0),
        (2.0, 2.0),
        (1e-30, -1e-30),
    ];
    let mut pick = Pick(0x243f_6a88_85a3_08d3);
    while cases.len() < 10_000 {
        let span = if cases.len() % 4 == 0 { 0.5 } else { 1.5 };
        cases.push((pick.float(span), pick.float(span)));
    }

    for (k, &(x, y)) in cases.iter().enumerate() {
        ee.call(classify, &[u64::from(OBJ)], &[x, y], BUDGET);
        let want_dir = ee.v0() as u8;
        let want_angle = ee.f32_at(OBJ + 0x634);
        let want_reach = ee.f32_at(OBJ + 0x638);

        let got = look(&table, stick(F(x), F(y), [false; 4]));
        assert_eq!(
            got.dir as u8, want_dir,
            "case {k} ({x:e}, {y:e}): the game says direction {want_dir}, the port {:?}",
            got.dir
        );
        assert_eq!(
            got.angle.to_bits(),
            want_angle.to_bits(),
            "case {k} ({x:e}, {y:e}): the game's angle is {want_angle:e}, the port's {:e}",
            got.angle.to_f32()
        );
        assert_eq!(
            got.reach.to_bits(),
            want_reach.to_bits(),
            "case {k} ({x:e}, {y:e}): the game's deflection is {want_reach:e}, the port's {:e}",
            got.reach.to_f32()
        );
    }
    println!("the stick classifier: {} deflections, bit-exact", cases.len());
}

// ---------------------------------------------------------------------------------------------------
// 4 — the request machine
// ---------------------------------------------------------------------------------------------------

/// A direction by its number, so a test can walk 0..=4 including the numbers the decoder never emits.
fn dir_of(k: u32) -> Dir {
    match k {
        1 => Dir::Up,
        2 => Dir::Down,
        3 => Dir::Left,
        4 => Dir::Right,
        _ => Dir::None,
    }
}

/// A rider object with nothing set: the contact list pointer aimed at a cleared count, and every flag
/// off. Each case sets what it cares about.
fn clear_object(ee: &mut Ee) {
    ee.load(OBJ, &vec![0u8; 0xb00]);
    ee.set_u32(OBJ + CONTACTS, OBJ + 0x200);
    ee.set_u32(OBJ + 0x200 + 0x40, 0);
}

#[test]
fn the_request_gate_and_the_slot_switch_match() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["stunt_request", "stunt_queue_push"]) else {
        return;
    };
    let (request, push) = (at[0], at[1]);

    // The push allocates a heap node, which nothing in this image has set up: stand in for it and
    // record what it was handed, which is the only thing the port's queue can be compared against.
    let pushed = Rc::new(Cell::new(-1i64));
    let seen = Rc::clone(&pushed);
    ee.hooks.insert(push, Box::new(move |ee| seen.set(ee.r(5) as i64)));

    let stunts: Vec<Stunt> = (0..SLOTS)
        .map(|i| Stunt {
            name: format!("stunt{i}"),
            duration: F(1.0),
            index: i as i32,
            ..Stunt::default()
        })
        .collect();
    let set = Set::new(stunts.clone());
    let params = Params { overlap: F::ZERO, no_hander_speed: F::ZERO };

    let mut cases = 0;
    for ground in 0..2u32 {
        for contact in 0..2u32 {
            for armed in 0..2u32 {
                for fresh in 0..2u32 {
                    for running in 0..2u32 {
                        for group in 0..=6u32 {
                            for dir in 0..=4u32 {
                                clear_object(&mut ee);
                                write_records(&mut ee, OBJ, &stunts);
                                ee.set_u32(OBJ + GROUND, ground);
                                ee.set_u32(OBJ + 0x200 + 0x40, contact);
                                ee.set_u32(OBJ + ARMED, armed);
                                ee.set_u32(OBJ + FRESH, fresh);
                                ee.set_u32(OBJ + IN_PROGRESS, running);
                                ee.set_u32(OBJ + GROUP, group);
                                ee.set_u32(OBJ + DIR, dir);
                                // A trick that is running but not yet near its end, so the overlap
                                // branch cannot fire and this case is only about the gate.
                                ee.set_u32(OBJ + CURRENT, 0);
                                ee.set_f32(OBJ + ELAPSED, 0.0);
                                ee.set_f32(OBJ + OVERLAP, 0.0);
                                ee.set_u32(OBJ + REQUESTED, u32::from(Trick::NONE.0));
                                pushed.set(-1);

                                ee.call(request, &[u64::from(OBJ), 0xff], &[], BUDGET);
                                let want = (
                                    ee.v0() != 0,
                                    ee.u32_at(OBJ + REQUESTED),
                                    ee.u32_at(OBJ + RESTART) != 0,
                                    ee.i32_at(OBJ + SEQ),
                                    ee.u32_at(OBJ + ARMED) != 0,
                                    pushed.get(),
                                );

                                let mut t = Tricks::default();
                                t.gesture.group = group as u8;
                                t.gesture.dir = dir_of(dir);
                                t.gesture.armed = armed != 0;
                                t.in_progress = running != 0;
                                t.current = Trick(0);
                                t.requested = Trick::NONE;
                                let bike = Bike {
                                    on_ground: ground != 0,
                                    contact: contact != 0,
                                    wrecked: false,
                                    fresh: fresh != 0,
                                    speed: F::ZERO,
                                };
                                let got_any = t.request(&set, bike, params);
                                let got = (
                                    got_any,
                                    u32::from(t.requested.0),
                                    t.restart,
                                    t.seq,
                                    t.gesture.armed,
                                    t.queue.front().map(|s| i64::from(s.0)).unwrap_or(-1),
                                );
                                assert_eq!(
                                    got, want,
                                    "ground {ground} contact {contact} armed {armed} fresh {fresh} \
                                     running {running} group {group} dir {dir}: \
                                     (any, requested, restart, seq, armed, queued)"
                                );
                                cases += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    println!("the request gate: {cases} combinations, exact");
}

#[test]
fn the_overlap_test_matches_over_ten_thousand_triples() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["stunt_request", "stunt_queue_pop"]) else {
        return;
    };
    let (request, pop) = (at[0], at[1]);

    // The pop frees a heap node; stand in for it and hand back a slot nothing else would produce.
    const POPPED: u8 = 19;
    let popped = Rc::new(Cell::new(false));
    let seen = Rc::clone(&popped);
    ee.hooks.insert(
        pop,
        Box::new(move |ee| {
            seen.set(true);
            ee.set(2, u64::from(POPPED));
        }),
    );

    let mut pick = Pick(0xb7e1_5162_8aed_2a6a);
    let mut cases = 0;
    for k in 0..10_000 {
        // Durations and clocks in the range the table uses, plus the corners: zero, equal, and a long
        // overlap that the halving has to cap.
        let (duration, overlap, elapsed) = match k {
            0 => (1.0, 0.0, 1.0),
            1 => (1.0, 0.5, 0.5),
            2 => (1.0, 10.0, 0.5),
            3 => (0.0, 0.0, 0.0),
            4 => (1.0, 0.25, 0.749_999_9),
            _ => (pick.float(4.0).abs(), pick.float(4.0), pick.float(4.0)),
        };
        let mut stunts: Vec<Stunt> = (0..SLOTS)
            .map(|i| Stunt { name: format!("stunt{i}"), duration: F(1.0), index: i as i32, ..Stunt::default() })
            .collect();
        let slot = pick.upto(SLOTS as u32) as usize;
        stunts[slot].duration = F(duration);
        let set = Set::new(stunts.clone());
        let queued = pick.upto(2);

        clear_object(&mut ee);
        write_records(&mut ee, OBJ, &stunts);
        // No gesture, so the gate falls through to the branch this test is about.
        ee.set_u32(OBJ + IN_PROGRESS, 1);
        ee.set_u32(OBJ + CURRENT, slot as u32);
        ee.set_u32(OBJ + REQUESTED, slot as u32);
        ee.set_f32(OBJ + ELAPSED, elapsed);
        ee.set_f32(OBJ + OVERLAP, overlap);
        ee.set_u32(OBJ + QUEUE, queued);
        ee.set_u32(OBJ + SEQ, 3);
        popped.set(false);

        ee.call(request, &[u64::from(OBJ), 0xff], &[], BUDGET);
        let want = (
            ee.v0() != 0,
            ee.u32_at(OBJ + REQUESTED),
            ee.u32_at(OBJ + RESTART) != 0,
            ee.i32_at(OBJ + SEQ),
            popped.get(),
        );

        let mut t = Tricks::default();
        t.in_progress = true;
        t.current = Trick(slot as u8);
        t.requested = Trick(slot as u8);
        t.elapsed = F(elapsed);
        t.seq = 3;
        if queued != 0 {
            t.queue.push_back(Trick(POPPED));
        }
        let bike = Bike { on_ground: true, contact: true, wrecked: false, fresh: true, speed: F::ZERO };
        let any = t.request(&set, bike, Params { overlap: F(overlap), no_hander_speed: F::ZERO });
        let got = (any, u32::from(t.requested.0), t.restart, t.seq, t.queue.is_empty() && queued != 0);
        assert_eq!(
            got, want,
            "case {k}: duration {duration:e} overlap {overlap:e} elapsed {elapsed:e} queued {queued}: \
             (any, requested, restart, seq, popped)"
        );
        cases += 1;
    }
    println!("the overlap test: {cases} triples, exact");
}

// ---------------------------------------------------------------------------------------------------
// 5 — the AI's choice
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_ai_choice_matches_over_ten_thousand_air_times() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["stunt_ai_choice", "rand_state", "strcmp"]) else {
        return;
    };
    let (choose, rand_state, strcmp) = (at[0], at[1], at[2]);

    // The library's `strcmp` is hand-written with the R5900's quadword compares, two of which this
    // interpreter does not implement yet, so stand in for it. What it computes is not in question —
    // the test is about which slots the chooser collects and which one it picks.
    ee.hooks.insert(
        strcmp,
        Box::new(|ee| {
            let (mut a, mut b) = (ee.r(4) as u32, ee.r(5) as u32);
            loop {
                let (x, y) = (ee.u8_at(a), ee.u8_at(b));
                if x != y || x == 0 {
                    ee.set(2, (i64::from(x) - i64::from(y)) as u64);
                    return;
                }
                a += 1;
                b += 1;
            }
        }),
    );

    let mut pick = Pick(0x9e37_79b9_7f4a_7c15);
    for k in 0..10_000 {
        // A table with a spread of durations, the throw-away somewhere in it, and an air time that
        // sometimes fits nothing and sometimes fits everything.
        let throwaway = pick.upto(SLOTS as u32 + 1) as usize;
        let stunts: Vec<Stunt> = (0..SLOTS)
            .map(|i| Stunt {
                name: if i == throwaway { "throwaway".to_owned() } else { format!("stunt{i}") },
                duration: F(pick.float(3.0).abs()),
                index: i as i32,
                ..Stunt::default()
            })
            .collect();
        let airtime = match k % 7 {
            0 => 0.0,
            1 => 100.0,
            _ => pick.float(3.0),
        };

        clear_object(&mut ee);
        write_records(&mut ee, OBJ, &stunts);
        let seed = u64::from(pick.next()) << 32 | u64::from(pick.next());
        ee.write(rand_state, 8, u128::from(seed));
        ee.set_u32(OBJ + 0x300, 0xdead_beef); // the out-parameter, so an untouched one is visible
        ee.call(choose, &[u64::from(OBJ), u64::from(OBJ + 0x300)], &[airtime], BUDGET);
        let want = (ee.v0() != 0, ee.u32_at(OBJ + 0x300));

        let mut rng = Rng::from_state(seed);
        let set = Set::new(stunts.clone());
        let choice = set.ai_choice(F(airtime), &mut rng);
        let got = (choice.is_some(), choice.map(|s| s as u32).unwrap_or(0xdead_beef));
        assert_eq!(got, want, "case {k}: air time {airtime:e}, throw-away at {throwaway}");
        assert_eq!(rng.state(), ee.read(rand_state, 8) as u64, "case {k}: the generator moved on differently");
    }
    println!("the AI's choice: 10,000 air times, exact");
}

// ---------------------------------------------------------------------------------------------------
// 6 — the clocks
// ---------------------------------------------------------------------------------------------------

/// The clock fields, by the offsets the original uses.
const LATCHED: u32 = 0x864;
/// The latched request count the restart test reads — a byte, next to the rest of the packed state.
const LATCHED_SEQ: u32 = 0x8a0;
const PREV_SEQ: u32 = 0xa08;
const FROZEN: u32 = 0x3a0;
const SAFE: u32 = 0xa10;
const HOLD_DONE: u32 = 0xa14;
const HOLD_ELAPSED: u32 = 0xa24;
const WIGGLE_TIME: u32 = 0xa28;
const PROGRESS: u32 = 0xa2c;
const HELD: u32 = 0xa30;
const NO_HANDER_FLAG: u32 = 0xa44;
const NO_HANDER_ID: u32 = 0xa80;
/// The two pointers the clock section goes through: the animation state it writes a few blend values
/// into, and the vtable it reaches the motion library by.
const ANIM: u32 = 0x7a0;
const ANIM_STATE: u32 = 0x7a4;
const VTABLE: u32 = 0x48;
/// The three vtable slots the clock section calls: set the hand-hold pose, stop the animation, start
/// the one a trick id names.
const SET_HAND_HOLD: u32 = 0x118;
const STOP_ANIM: u32 = 0x110;
const START_ANIM: u32 = 0x10c;

/// Where this test puts the things the object points at. The animation state is written through with
/// a quadword store, so it is aligned.
const ANIM_OBJ: u32 = 0x0108_0000;
const VTABLE_OBJ: u32 = 0x0108_8000;
/// Addresses that are not code: each one is hooked, so the vtable can be a table of them.
const HOOK_HAND_HOLD: u32 = 0x0070_0000;
const HOOK_STOP: u32 = 0x0070_0010;
const HOOK_START: u32 = 0x0070_0020;

/// A stand-in for the engine's smooth noise, chosen so that both sides get the identical single from
/// the identical argument without either of them doing any arithmetic. The real sampler reads tables
/// built at start-up from the RNG; what matters here is that the clocks multiply whatever it returns
/// by the hold's amplitude, and `0.0` would hide that.
fn fake_noise(x: f32) -> f32 {
    const WAVE: [f32; 8] = [0.0, 1.0, -1.0, 0.5, -0.25, 0.937_5, -0.812_5, 0.125];
    WAVE[(x.to_bits() >> 3) as usize & 7]
}

/// What the clock section did that is not a field: which of the motion library's three entry points it
/// called, and with what.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
struct Calls {
    hand_hold: Option<bool>,
    stopped: bool,
    started: Option<u32>,
}

#[test]
fn the_clocks_match_over_ten_thousand_frames() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["stunt_clocks", "smooth_noise", "anim_rewind", "clock_section_end"]) else {
        return;
    };
    let (clocks, noise, rewind, section_end) = (at[0], at[1], at[2], at[3]);

    // The function does not end where the clocks do: past this point it picks an animation, blends it
    // and skins the rider through VU0, none of which is this task's. The hook ends the call there —
    // every field this test reads has been written by then — by pointing the return at the sentinel
    // `Ee::call` waits for.
    ee.hooks.insert(section_end, Box::new(|ee| ee.set(31, u64::from(DONE))));
    // The engine's noise, and the animation rewind a restart does. The noise is read once per held
    // frame and nowhere else, so counting the calls counts how often the hold branch was reached.
    let holds = Rc::new(Cell::new(0usize));
    let counted = Rc::clone(&holds);
    ee.hooks.insert(noise, Box::new(move |ee| {
        let x = ee.f(12);
        counted.set(counted.get() + 1);
        ee.set_f(0, fake_noise(x));
    }));
    ee.hooks.insert(rewind, Box::new(|_| {}));
    // The three vtable entries, which the object's vtable pointer is aimed at below.
    let calls = Rc::new(Cell::new(Calls::default()));
    for (hook, slot) in [(HOOK_HAND_HOLD, 0u32), (HOOK_STOP, 1), (HOOK_START, 2)] {
        let seen = Rc::clone(&calls);
        ee.hooks.insert(
            hook,
            Box::new(move |ee| {
                let mut c = seen.get();
                match slot {
                    0 => c.hand_hold = Some(ee.r(5) != 0),
                    1 => c.stopped = true,
                    _ => c.started = Some(ee.r(5) as u32),
                }
                seen.set(c);
            }),
        );
    }

    let mut pick = Pick(0x2545_f491_4f6c_dd1d);
    let (mut cases, mut eases, mut restarts, mut held_frames) = (0, 0, 0, 0);
    for k in 0..10_000 {
        // A table whose clocks reach every branch: holds that are off (zero) as well as on, safe
        // times either side of the duration, and durations short enough for a frame to overshoot.
        let no_hander = pick.upto(SLOTS as u32) as usize;
        let stunts: Vec<Stunt> = (0..SLOTS)
            .map(|i| Stunt {
                name: if i == no_hander { "nohandedlander".to_owned() } else { format!("stunt{i}") },
                safe_time: F(pick.float(2.0).abs()),
                hold_time: if pick.upto(4) == 0 { F::ZERO } else { F(pick.float(1.5).abs()) },
                duration: F(pick.float(2.0).abs()),
                index: i as i32,
                ..Stunt::default()
            })
            .collect();
        let set = Set::new(stunts.clone());

        // The state the frame starts from. The ids go past the 24 mapped slots on purpose: 24..=28 are
        // the backflip, the wheelie, the stoppie and "nothing", which the switch leaves alone.
        let id = match k % 9 {
            0 => 28,
            1 => 25,
            2 => 24,
            3 => 26,
            _ => pick.upto(SLOTS as u32),
        };
        let latched = if pick.upto(4) == 0 { pick.upto(29) } else { id };
        let hold = stunts[(id as usize).min(SLOTS - 1)].hold_time.to_f32();
        // Elapsed is aimed at the hold's two windows — `[hold, hold + 0.1]` holds, the tenth of a
        // second in front of it eases — because a uniform draw would almost never land in either.
        let elapsed = match pick.upto(8) {
            0 => hold,
            1 => hold + 0.05,
            2 => hold + 0.1,
            3 => hold - 0.05,
            4 => hold - 0.000_01,
            5 => hold + 0.100_001,
            _ => pick.float(2.0),
        };
        let (seq, prev) = match pick.upto(4) {
            0 => (0u32, 0u32),
            1 => (1, 1),
            2 => (3, 3),
            _ => (pick.upto(5), pick.upto(5)),
        };
        let restart = pick.upto(3) == 0;
        let frozen = pick.upto(8) == 0;
        let group = if pick.upto(3) == 0 { 0 } else { pick.upto(7) };
        let ground = pick.upto(2);
        let hold_done = pick.upto(4) == 0;
        let held = match pick.upto(3) {
            0 => 0.0,
            1 => pick.float(0.5).abs(),
            _ => pick.float(0.5),
        };
        let hold_elapsed = pick.float(0.6).abs();
        let wiggle_time = pick.float(2.0).abs();
        let dt = match pick.upto(4) {
            0 => 1.0 / 60.0,
            1 => 1.0 / 30.0,
            2 => 0.0,
            _ => pick.float(0.2).abs(),
        };

        clear_object(&mut ee);
        write_records(&mut ee, OBJ, &stunts);
        ee.load(ANIM_OBJ, &vec![0u8; 0x400]);
        ee.load(VTABLE_OBJ, &vec![0u8; 0x200]);
        ee.set_u32(VTABLE_OBJ + SET_HAND_HOLD, HOOK_HAND_HOLD);
        ee.set_u32(VTABLE_OBJ + STOP_ANIM, HOOK_STOP);
        ee.set_u32(VTABLE_OBJ + START_ANIM, HOOK_START);
        ee.set_u32(OBJ + VTABLE, VTABLE_OBJ);
        ee.set_u32(OBJ + ANIM, ANIM_OBJ);
        ee.set_u32(OBJ + ANIM_STATE, ANIM_OBJ);
        ee.set_u32(OBJ + CURRENT, id);
        ee.set_u32(OBJ + LATCHED, latched);
        ee.set_u32(OBJ + REQUESTED, latched);
        ee.set_u32(OBJ + RESTART, u32::from(restart));
        ee.load(OBJ + LATCHED_SEQ, &[seq as u8]);
        ee.set_u32(OBJ + PREV_SEQ, prev);
        ee.set_u32(OBJ + FROZEN, u32::from(frozen));
        ee.set_u32(OBJ + GROUP, group);
        ee.set_u32(OBJ + GROUND, ground);
        ee.set_u32(OBJ + NO_HANDER_ID, no_hander as u32);
        ee.set_u32(OBJ + IN_PROGRESS, u32::from(pick.upto(2) == 1));
        ee.set_u32(OBJ + HOLD_DONE, u32::from(hold_done));
        ee.set_f32(OBJ + ELAPSED, elapsed);
        ee.set_f32(OBJ + HELD, held);
        ee.set_f32(OBJ + HOLD_ELAPSED, hold_elapsed);
        ee.set_f32(OBJ + WIGGLE_TIME, wiggle_time);
        ee.set_f32(OBJ + PROGRESS, 0.0);
        ee.set_u32(OBJ + SAFE, 1); // set, so that the clearing is visible
        calls.set(Calls::default());

        // a1 = 0: the motion prologue reads the rider's skeleton, which a fabricated object does not
        // have, and it touches none of the clocks.
        ee.call(clocks, &[u64::from(OBJ), 0], &[dt], BUDGET);
        let want = (
            ee.u32_at(OBJ + CURRENT),
            ee.f32_at(OBJ + ELAPSED).to_bits(),
            ee.f32_at(OBJ + PROGRESS).to_bits(),
            ee.f32_at(OBJ + HOLD_ELAPSED).to_bits(),
            ee.f32_at(OBJ + WIGGLE_TIME).to_bits(),
            ee.f32_at(OBJ + HELD).to_bits(),
            ee.u32_at(OBJ + HOLD_DONE) != 0,
            ee.u32_at(OBJ + IN_PROGRESS) != 0,
            ee.u32_at(OBJ + SAFE) != 0,
            ee.u32_at(OBJ + NO_HANDER_FLAG) != 0,
            ee.u32_at(OBJ + RESTART) != 0,
            calls.get().hand_hold,
        );

        // The switch runs on the *latched* id, so that is what decides whether the motion library is
        // called at all.
        let mapped = Trick(latched as u8).slot().is_some();
        let mut t = Tricks::default();
        t.gesture.group = group as u8;
        t.current = Trick(id as u8);
        t.requested = Trick(latched as u8);
        t.restart = restart;
        t.seq = seq as i32;
        t.prev_seq = prev as i32;
        t.in_progress = ee.u32_at(OBJ + IN_PROGRESS) != 0; // read back: the call may have changed it
        t.hold_done = hold_done;
        t.elapsed = F(elapsed);
        t.held = F(held);
        t.hold_elapsed = F(hold_elapsed);
        t.wiggle_time = F(wiggle_time);
        t.safe = true;
        let bike = Bike { on_ground: ground != 0, ..Bike::default() };
        t.update(F(dt), &set, bike, frozen, |x| F(fake_noise(x.to_f32())));
        let got = (
            u32::from(t.current.0),
            t.elapsed.to_bits(),
            t.progress.to_bits(),
            t.hold_elapsed.to_bits(),
            t.wiggle_time.to_bits(),
            t.held.to_bits(),
            t.hold_done,
            t.in_progress,
            t.safe,
            t.no_hander,
            t.restart,
            // The pose is not a field on the original: it is the argument of a vtable call, which a
            // frame on an unmapped id does not make at all.
            if mapped { Some(t.hand_hold) } else { None },
        );
        assert_eq!(
            got, want,
            "case {k}: id {id} latched {latched} seq {seq}/{prev} restart {restart} frozen {frozen} \
             group {group} ground {ground} elapsed {elapsed:e} hold {hold:e} held {held:e} dt {dt:e}: \
             (current, elapsed, progress, hold_elapsed, wiggle, held, hold_done, in_progress, safe, \
             no_hander, restart, hand_hold)"
        );
        // The restart test, read off the two calls the original makes to start an animation over. The
        // prediction is written out here rather than taken from the port, so that a frame whose
        // clocks were already zero still proves the test rather than passing by coincidence.
        let restarted = !frozen && (latched != id || restart || (seq >= 2 && seq != prev));
        assert_eq!(
            calls.get().started,
            if mapped && restarted { Some(latched) } else { None },
            "case {k}: id {id} latched {latched} seq {seq}/{prev} restart {restart} \
             frozen {frozen}: whether the trick started over"
        );
        assert_eq!(
            calls.get().stopped,
            mapped && ((restarted && restart) || !t.in_progress),
            "case {k}: id {id} latched {latched} restart {restart} in progress {}: whether the \
             animation was stopped",
            t.in_progress
        );
        if calls.get().started.is_some() {
            restarts += 1;
        }
        if t.held.to_bits() != F(held).to_bits() && holds.get() == held_frames {
            eases += 1;
        }
        held_frames = holds.get();
        cases += 1;
    }
    // A generated case set is only a proof of what it reaches: the two branches that are hard to land
    // on are the ones this says out loud, so a later change to the generator cannot quietly stop
    // covering them.
    assert!(holds.get() > 100, "only {} frames held", holds.get());
    assert!(eases > 100, "only {eases} frames eased into a hold");
    assert!(restarts > 100, "only {restarts} frames started a trick over");
    println!(
        "the clocks: {cases} frames, bit-exact ({} held, {eases} eased, {restarts} restarted)",
        holds.get()
    );
}

// ---------------------------------------------------------------------------------------------------
// 7 — the landing
// ---------------------------------------------------------------------------------------------------

/// The landing test's own fields: the attitude values it compares, the limits it compares them
/// against, and the two trick fields the clause at the end of it reads.
const ALIVE: u32 = 0x570;
const WRECKING: u32 = 0x748;
const SPEED: u32 = 0x424;
const NO_HANDER_SPEED: u32 = 0x780;
const THROWAWAY_ID: u32 = 0xa84;
/// The vtable slot the wreck goes through, and the address this test aims it at.
const WRECK: u32 = 0xfc;
const HOOK_WRECK: u32 = 0x0070_0030;

/// Everything the attitude tests in front of the trick's clause read, set so that all of them pass:
/// the rider upright and still, with limits nothing can reach.
fn upright(ee: &mut Ee) {
    ee.set_u32(OBJ + ALIVE, 0); // skip the collision block in front: it needs a world
    ee.set_u32(OBJ + WRECKING, 0);
    ee.set_u32(OBJ + 0x4f4, 0); // not the case that wrecks on a contact
    ee.set_u32(OBJ + 0x57c, 0); // not the case with its own pitch window
    ee.set_u32(OBJ + 0x580, 0); // not the case that wrecks on a roll fraction
    ee.set_u32(OBJ + 0x4f8, 0);
    for v in [0x550u32, 0x554, 0x558] {
        ee.set_f32(OBJ + v, 0.0);
    }
    for limit in [0x754u32, 0x758, 0x75c, 0x760, 0x764, 0x768, 0x76c, 0x770, 0x774, 0x778, 0x77c] {
        ee.set_f32(OBJ + limit, 1.0e9);
    }
    // The animation state: the attitude the tests read, and the heading the throw-away branch
    // normalises before it throws the rider off.
    ee.load(ANIM_OBJ, &vec![0u8; 0x400]);
    ee.set_f32(ANIM_OBJ + 0x90, 0.0);
    ee.set_f32(ANIM_OBJ + 0x98, 0.0);
    ee.set_f32(ANIM_OBJ + 0x260, 3.0);
    ee.set_f32(ANIM_OBJ + 0x264, 4.0);
    ee.set_f32(ANIM_OBJ + 0x268, 0.0);
    ee.set_f32(ANIM_OBJ + 0x26c, 1.0);
    ee.set_u32(OBJ + ANIM_STATE, ANIM_OBJ);
}

#[test]
fn the_landing_clause_matches_every_combination() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["stunt_landing", "wrecks_enabled"]) else {
        return;
    };
    let (landing, enabled) = (at[0], at[1]);
    ee.set_u32(enabled, 1);

    // The wreck itself is a vtable call that hands a reason to the rider's state machine; the port
    // answers with the same reason, so the hook records it.
    let reason = Rc::new(Cell::new(None));
    let seen = Rc::clone(&reason);
    ee.hooks.insert(HOOK_WRECK, Box::new(move |ee| seen.set(Some(ee.r(5) as u32 as u8))));

    // Three tables: no throw-away at all (which is what the disc ships, and leaves the sentinel 24 —
    // the backflip's own id — in the field the clause compares against), and one at either end.
    let (mut cases, mut clean, mut thrown, mut wrecked) = (0, 0, 0, 0);
    for throwaway in [Set::ABSENT, 0, SLOTS - 1] {
        let stunts: Vec<Stunt> = (0..SLOTS)
            .map(|i| Stunt {
                name: if i == throwaway { "throwaway".to_owned() } else { format!("stunt{i}") },
                duration: F::ONE,
                index: i as i32,
                ..Stunt::default()
            })
            .collect();
        let set = Set::new(stunts.clone());
        assert_eq!(set.throwaway, throwaway, "the table says where the throw-away is");

        for ground in 0..2u32 {
            for contact in 0..2u32 {
                for running in 0..2u32 {
                    for no_hander in 0..2u32 {
                        for safe in 0..2u32 {
                            for id in [0u32, 23, 24, 25, 28, throwaway as u32] {
                                for progress in [0.0f32, 0.95, 0.950_000_1, 1.0] {
                                    for slow in 0..2u32 {
                                        clear_object(&mut ee);
                                        write_records(&mut ee, OBJ, &stunts);
                                        upright(&mut ee);
                                        ee.set_u32(VTABLE_OBJ, 0);
                                        ee.set_u32(VTABLE_OBJ + WRECK, HOOK_WRECK);
                                        ee.set_u32(OBJ + VTABLE, VTABLE_OBJ);
                                        ee.set_u32(OBJ + GROUND, ground);
                                        ee.set_u32(OBJ + 0x200 + 0x40, contact);
                                        ee.set_u32(OBJ + IN_PROGRESS, running);
                                        ee.set_u32(OBJ + NO_HANDER_FLAG, no_hander);
                                        ee.set_u32(OBJ + SAFE, safe);
                                        ee.set_u32(OBJ + CURRENT, id);
                                        ee.set_u32(OBJ + THROWAWAY_ID, throwaway as u32);
                                        ee.set_f32(OBJ + PROGRESS, progress);
                                        ee.set_f32(OBJ + SPEED, if slow == 1 { 1.0 } else { 9.0 });
                                        ee.set_f32(OBJ + NO_HANDER_SPEED, 5.0);
                                        reason.set(None);

                                        ee.call(landing, &[u64::from(OBJ), 0], &[], BUDGET);
                                        let want = reason.get();

                                        let mut t = Tricks::default();
                                        t.current = Trick(id as u8);
                                        t.in_progress = running != 0;
                                        t.no_hander = no_hander != 0;
                                        t.safe = safe != 0;
                                        t.progress = F(progress);
                                        let bike = Bike {
                                            on_ground: ground != 0,
                                            contact: contact != 0,
                                            speed: F(if slow == 1 { 1.0 } else { 9.0 }),
                                            ..Bike::default()
                                        };
                                        let params = Params {
                                            overlap: F::ZERO,
                                            no_hander_speed: F(5.0),
                                        };
                                        let got = t.landing(&set, bike, params).map(|w| w.0);
                                        assert_eq!(
                                            got, want,
                                            "throw-away {throwaway} id {id} ground {ground} \
                                             contact {contact} running {running} no-hander \
                                             {no_hander} safe {safe} progress {progress} \
                                             slow {slow}: the wreck reason"
                                        );
                                        match want {
                                            None => clean += 1,
                                            Some(7) => thrown += 1,
                                            _ => wrecked += 1,
                                        }
                                        cases += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // All three verdicts have to appear, or the combinations are not reaching the clause.
    assert!(clean > 0 && thrown > 0 && wrecked > 0, "{clean} clean, {thrown} thrown, {wrecked} wrecked");
    println!("the landing: {cases} combinations, exact ({clean} clean, {thrown} thrown on purpose, {wrecked} wrecked)");
}

// ---------------------------------------------------------------------------------------------------
// 8 — what is asked for when nothing is
// ---------------------------------------------------------------------------------------------------

/// The vtable slot that answers "is an animation playing", and the address this test aims it at.
const PLAYING: u32 = 0x120;
const HOOK_PLAYING: u32 = 0x0070_0040;

#[test]
fn the_idle_request_matches_every_state() {
    let Some(mut ee) = game_or_skip() else { return };
    let Some(at) = named(&["stunt_idle"]) else { return };
    let idle = at[0];

    // The original asks the motion library whether an animation is playing and does nothing while one
    // is. The port leaves that question to its caller, so the hook answers both ways: with "no" the
    // two have to agree, and with "yes" the original has to leave every field alone — which is what
    // makes the caller's gate the only difference between them.
    let playing = Rc::new(Cell::new(0u64));
    let answer = Rc::clone(&playing);
    ee.hooks.insert(HOOK_PLAYING, Box::new(move |ee| {
        let a = answer.get();
        ee.set(2, a);
    }));

    for busy in 0..2u64 {
        for wrecked in 0..2u32 {
            for ground in 0..2u32 {
                for fresh in 0..2u32 {
                    clear_object(&mut ee);
                    ee.set_u32(VTABLE_OBJ, 0);
                    ee.set_u32(VTABLE_OBJ + PLAYING, HOOK_PLAYING);
                    ee.set_u32(OBJ + VTABLE, VTABLE_OBJ);
                    ee.set_u32(OBJ + WRECKING, wrecked);
                    ee.set_u32(OBJ + GROUND, ground);
                    ee.set_u32(OBJ + FRESH, fresh);
                    ee.set_u32(OBJ + REQUESTED, 0xff);
                    ee.set_u32(OBJ + SEQ, 7);
                    playing.set(busy);

                    ee.call(idle, &[u64::from(OBJ)], &[], BUDGET);
                    let want =
                        (ee.u32_at(OBJ + REQUESTED), ee.u32_at(OBJ + FRESH) != 0, ee.i32_at(OBJ + SEQ));

                    if busy == 1 && wrecked == 0 {
                        assert_eq!(
                            want,
                            (0xff, fresh != 0, 7),
                            "wrecked {wrecked} ground {ground}: an animation is playing, so nothing \
                             should have been asked for"
                        );
                        continue;
                    }
                    let mut t = Tricks::default();
                    t.requested = Trick(0xff);
                    t.seq = 7;
                    let mut bike = Bike {
                        on_ground: ground != 0,
                        wrecked: wrecked != 0,
                        fresh: fresh != 0,
                        ..Bike::default()
                    };
                    t.idle(&mut bike);
                    let got = (u32::from(t.requested.0), bike.fresh, t.seq);
                    assert_eq!(
                        got, want,
                        "busy {busy} wrecked {wrecked} ground {ground} fresh {fresh}: \
                         (requested, fresh, seq)"
                    );
                }
            }
        }
    }
    println!("the idle request: 16 states, exact");
}

// ---------------------------------------------------------------------------------------------------
// (9) the whole machine on a bike: a scripted pad lands one trick and wrecks another
// ---------------------------------------------------------------------------------------------------

/// The bike the two flights below are flown on: the freeride test bike with the wreck limits the
/// rider needs. Hand-written, not the disc's — see the file's own head.
const RIDER_BIKE: &str = include_str!("trick-bike.phy.txt");

/// The frame the fixture runs at: the engine's own clamped step.
const FRAME: F = F(1.0 / 60.0);

/// How high the bike is dropped from. High enough for a stunt to run its whole duration before the
/// wheels come down, which is what makes a landed trick and a wrecked one two scripts and not two
/// bikes.
const DROP: f32 = 30.0;

/// The buttons that make a group: the six combinations [`game_sim::trick::group`] numbers, by group.
fn buttons_for(group: u8) -> [bool; 3] {
    match group {
        1 => [true, false, false],
        2 => [false, true, false],
        3 => [true, true, false],
        4 => [true, false, true],
        5 => [false, true, true],
        6 => [true, true, true],
        _ => [false, false, false],
    }
}

/// The d-pad that names a direction, in the order the decoder polls it: left, right, up, down.
fn dpad_for(dir: Dir) -> [bool; 4] {
    let mut out = [false; 4];
    match dir {
        Dir::Left => out[0] = true,
        Dir::Right => out[1] = true,
        Dir::Up => out[2] = true,
        Dir::Down => out[3] = true,
        Dir::None => {}
    }
    out
}

/// The gesture that names a slot: the first group-and-direction pair that maps to it.
fn gesture_for(slot: usize) -> (u8, Dir) {
    for group in 1..=6u8 {
        for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
            if slot_of(group, dir) == Some(slot) {
                return (group, dir);
            }
        }
    }
    panic!("no gesture names slot {slot}");
}

/// What one scripted flight produced.
struct Flown {
    /// The rider at the end of it.
    rider: rider::Rider,
    /// The trick the gesture started, the trick that was landed, and the reason the rider came off.
    started: Option<Trick>,
    landed: Option<Trick>,
    wreck: Option<game_sim::trick::Wreck>,
    /// The furthest the stunt's clock got, which is what says whether it ran out in the air.
    elapsed: F,
    /// The frame the wheels first touched, or the length of the script if they never did.
    ground: usize,
}

/// Fly one drop, asking for a stunt on the frame the script says and holding the gesture for that
/// frame only, so the clock runs free instead of stopping in the hold.
///
/// `ask_at` past the end of the script is a drop with nobody touching the pad.
fn flight(p: &Phy, t: &rider::Tables, set: &Set, ask_at: usize, gesture: (u8, Dir), frames: usize) -> Flown {
    let mounts = (p.front.body_tire_force_position, p.rear.body_tire_force_position);
    let at = Vf::new(0.0, DROP, 0.0, 1.0);
    let mut r = rider::Rider::new(p, mounts, at, Vf::new(0.0, 0.0, 1.0, 0.0), set.clone());
    let mut world = Flat { y: F::ZERO, normal: Vf::new(0.0, 1.0, 0.0, 0.0) };
    let mut out = Flown {
        rider: rider::Rider::new(p, mounts, at, Vf::new(0.0, 0.0, 1.0, 0.0), set.clone()),
        started: None,
        landed: None,
        wreck: None,
        elapsed: F::ZERO,
        ground: frames,
    };
    for k in 0..frames {
        let hands = if k == ask_at {
            Controls { trick: buttons_for(gesture.0), dpad: dpad_for(gesture.1), ..Controls::default() }
        } else {
            Controls::default()
        };
        let f = r.frame(p, t, &hands, FRAME, &mut world);
        out.started = out.started.or(f.started);
        out.landed = out.landed.or(f.landed);
        out.wreck = out.wreck.or(f.wreck);
        if r.tricks.elapsed > out.elapsed {
            out.elapsed = r.tricks.elapsed;
        }
        if r.ride.air.on_ground && out.ground == frames {
            out.ground = k;
        }
        if out.wreck.is_some() {
            break;
        }
    }
    out.rider = r;
    out
}

/// **P3.** The trick machine, the airborne wreck test and the ride state machine, wired onto the
/// ported ride and driven by a pad.
///
/// Two drops onto flat ground off the same bike, with the same stunt and the same gesture — only the
/// frame the gesture is made on differs:
///
/// - asked for early, the stunt runs its whole duration in the air and the wheels come down with
///   nothing in progress: the rider lands it and stays on the bike.
/// - asked for two frames before the ground, the stunt is still running and nowhere near its safe
///   time when the wheels touch: the wreck test's trick clause fires and the state machine puts the
///   rider down, with its reason, its clock and its counter.
///
/// Every clause of the wreck test in front of the trick ones is set out of reach in the fixture's
/// `.phy` (see its head), so what this proves is the *wiring*: that a pad gesture reaches the
/// request, that the clocks run on the stunt the disc's table names for that gesture, that
/// `in_progress` reaches the wreck test through [`game_sim::air::Loads`], and that the verdict
/// reaches the state machine. The clauses themselves are proved against the original in (7) above
/// and in `tests/air.rs`, and the stunt table in (1).
#[test]
fn a_scripted_pad_lands_one_trick_and_wrecks_another() {
    let Some(bytes) = table_bytes() else { return };
    let stunts = Stunts::parse(&bytes).expect("the stunt table");
    let set = Set::new(stunts.selection(0));
    assert_eq!(set.stunts.len(), SLOTS, "a rider's selection is 24 slots");

    let tables = rider::Tables::build();
    let p = Phy::read_bytes(RIDER_BIKE.as_bytes(), &tables.ride.curve);

    // The stunt to do: the shortest of the rider's 24, so that it fits inside one drop with room to
    // spare either side. Which one that is comes off the disc, not out of this file.
    let (slot, stunt) = set
        .stunts
        .iter()
        .enumerate()
        .filter(|(k, s)| {
            // Not the no-hander and not the throw-away: the wreck test has a clause of its own for
            // each, and what this fixture is for is the ordinary ending.
            *k != set.no_hander && *k != set.throwaway && s.duration > F::ZERO && s.safe_time > F::ZERO
        })
        .min_by(|a, b| a.1.duration.0.total_cmp(&b.1.duration.0))
        .map(|(k, s)| (k, s.clone()))
        .expect("a stunt to do");
    let gesture = gesture_for(slot);
    let span = (stunt.duration.0 / FRAME.0).ceil() as usize;
    println!(
        "slot {slot} is {:?}: {} s long ({span} frames), safe at {} s; its gesture is group {} {:?}",
        stunt.name, stunt.duration.0, stunt.safe_time.0, gesture.0, gesture.1
    );

    // The drop with nobody touching the pad, which is what both scripts are timed against and the
    // control for everything the two of them assert.
    let plain = flight(&p, &tables, &set, usize::MAX, gesture, 600);
    assert_eq!(plain.wreck, None, "an untouched drop onto flat ground does not wreck");
    assert_eq!(plain.landed, None, "and lands no trick, because none was asked for");
    let ground = plain.ground;
    assert!(ground > span + 8, "the drop ({ground} frames) has to outlast the stunt ({span})");
    println!("the drop reaches the ground on frame {ground}");

    // (a) asked for early: the stunt runs out in the air and the rider lands it.
    let a = flight(&p, &tables, &set, 2, gesture, ground + 30);
    assert_eq!(a.started, Some(Trick(slot as u8)), "the gesture started the stunt it names");
    assert!(a.elapsed >= stunt.duration, "the stunt ran its whole duration: {}", a.elapsed.0);
    assert_eq!(a.wreck, None, "a stunt finished in the air is landed, not wrecked");
    assert_eq!(a.landed, Some(Trick(slot as u8)), "the landed trick is the one that was started");
    assert_eq!(a.rider.phase(), game_sim::state::Phase::Riding, "still on the bike");
    assert!(!a.rider.tricks.in_progress, "the stunt is over by the time the wheels are down");
    assert_eq!(a.rider.state.wrecks, 0, "nothing to count");
    assert_eq!(a.ground, ground, "the stunt did not move the bike: the same landing frame");

    // (b) asked for two frames before the ground: still running, and nowhere near safe.
    let late = ground - 2;
    let b = flight(&p, &tables, &set, late, gesture, ground + 30);
    assert_eq!(b.started, Some(Trick(slot as u8)), "the same gesture started the same stunt");
    assert!(b.elapsed < stunt.safe_time, "the stunt is not safe to land: {}", b.elapsed.0);
    assert_eq!(b.landed, None, "there is no landing to count when the rider comes off");
    assert_eq!(
        b.wreck,
        Some(game_sim::trick::Wreck::TRICK),
        "landing on a stunt that is still running is the trick clause's own reason"
    );
    assert_eq!(b.rider.phase(), game_sim::state::Phase::Down, "off the bike");
    assert_eq!(b.rider.state.wrecks, 1, "one wreck, counted once");
    assert_eq!(b.rider.tricks.requested, Trick::NONE, "the wreck stopped the trick");
    assert!(b.rider.tricks.queue.is_empty(), "and emptied the queue");

    println!("a scripted pad: {:?} landed from frame 2, wrecked from frame {late}", stunt.name);
}

// ---------------------------------------------------------------------------------------------------
// S30: the human's thumbs
// ---------------------------------------------------------------------------------------------------

/// The recording whose tricks are compared, and the counter in its own save state (read with
/// `./kit state extract`): the capture's first row is the counter after it.
const RECORDING2: &str = "recording2.p2m2";
const RECORDING2_START: u64 = 2_373;

/// The captures of that recording's playback with the trick columns in them, in the order they are
/// looked for: C6's full index first, then the one S30 took for its own columns. Both are C6's watch
/// spec, so either serves.
const RECORDING2_CAPTURES: [&str; 2] = ["recording2-freeride.csv", "recording2-tricks.csv"];

/// The bike the recording was ridden on, which is where the overlap comes from (D7: the freeride
/// boot's Race 125).
const RECORDING2_PHY: &str = "race125p.phy";

/// The bike's `.phy` off the disc, by the leaf name, out of whichever `Bike\*.res` carries it.
fn recording2_phy(tables: &rider::Tables) -> Option<Phy> {
    let mut f = container()?;
    let mut front = vec![0u8; 64 << 10];
    f.read_exact(&mut front).expect("read the front of MX.PSR");
    let psr = Psr::parse(&front).expect("MX.PSR's directory");
    for e in &psr.entries {
        let n = e.name.to_ascii_lowercase();
        if !(n.starts_with("bike\\") && n.ends_with(".res")) {
            continue;
        }
        f.seek(SeekFrom::Start(e.at)).expect("seek");
        let mut archive = vec![0u8; e.size as usize];
        f.read_exact(&mut archive).expect("read");
        let Ok(res) = Res::parse(&archive) else { continue };
        let Some(m) = res.entries.iter().find(|m| m.name.to_ascii_lowercase().ends_with(RECORDING2_PHY)) else {
            continue;
        };
        let bytes = res.read(&archive, m).expect("the bike's .phy");
        return Some(Phy::read_bytes(&bytes, &tables.ride.curve));
    }
    None
}

/// One row's trick state, in the port's terms: the nine words the capture has a column for.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Row {
    group: u8,
    dir: u8,
    armed: bool,
    requested: u8,
    current: u8,
    seq: i32,
    running: bool,
    safe: bool,
    elapsed: u32,
}

/// The capture's names for [`Row`]'s fields, in its order.
const ROW_COLUMNS: [&str; 9] = [
    "trick_group",
    "trick_dir",
    "trick_armed",
    "trick_req",
    "trick_id",
    "trick_count",
    "trick_running",
    "trick_safe",
    "trick_elapsed",
];

impl Row {
    fn of(t: &Tricks) -> Row {
        Row {
            group: t.gesture.group,
            dir: t.gesture.dir as u8,
            armed: t.gesture.armed,
            requested: t.requested.0,
            current: t.current.0,
            seq: t.seq,
            running: t.in_progress,
            safe: t.safe,
            elapsed: t.elapsed.0.to_bits(),
        }
    }

    /// The columns that differ.
    fn differs(&self, want: &Row) -> Vec<&'static str> {
        let same = [
            self.group == want.group,
            self.dir == want.dir,
            self.armed == want.armed,
            self.requested == want.requested,
            self.current == want.current,
            self.seq == want.seq,
            self.running == want.running,
            self.safe == want.safe,
            self.elapsed == want.elapsed,
        ];
        ROW_COLUMNS.iter().zip(same).filter(|(_, s)| !s).map(|(n, _)| *n).collect()
    }
}

/// **S30. The tricks under a human's thumbs**: recording2's tape, read by I4's reader, through
/// [`Gesture::read`](game_sim::trick::Gesture::read), [`Tricks::request`] and [`Tricks::update`],
/// against the original's own trick fields captured from the same recording played back.
///
/// The trick layer is driven by the capture's own on-the-ground, wreck and fresh columns rather than
/// the port's physics, so what is compared is the trick machine and nothing upstream of it: the slot
/// each gesture names, the tick it is asked for, safe and over, and which touchdowns
/// [`Tricks::landing`] turns into a wreck. Four things about the *caller* had to be read off the
/// capture to drive it, and are the order this test calls the machine in (the S30 journal entry has
/// the rows that show each):
///
/// - **the pad**: a row's pad is video field `2k - 2` of the recording, every row — the first field
///   of the tick, on this playback;
/// - **the request runs on the gesture read the tick before**: the original reads the pad after the
///   clocks, so a gesture made on tick *k* is asked for on tick *k + 1*. `rider.rs` reads it first;
/// - **the stick is the pad's own**, through the input layer but not the `[Joystick]` rate limit;
/// - **a wreck keeps the trick running until it ends**, and a rider who gets back on without a
///   respawn has the trick reset on that tick. `rider.rs` resets it when the wreck starts.
///
/// One difference is left and is allowed for by name: on twelve ticks the stick is half way between
/// centre and the stop and the original's gesture reads no direction where the port's reads one. No
/// trick button is down on any of them, so no slot differs; why is not known (the journal says what
/// was ruled out). Skipped with a line where the recording, the capture or the disc is not here.
#[test]
fn the_humans_tricks() {
    let Ok(kit) = Kit::load() else {
        skip("no kit.toml: the human's tricks are unchecked");
        return;
    };
    let rpath = kit.ctx("recordings").join(RECORDING2);
    let Ok(bytes) = std::fs::read(&rpath) else {
        println!("no {} — copy it from the main checkout's context/recordings/", rpath.display());
        return;
    };
    let dir = kit.ctx("capture").join("acceptance");
    let Some((cpath, text)) = RECORDING2_CAPTURES.iter().find_map(|n| {
        let p = dir.join(n);
        let text = std::fs::read_to_string(&p).ok()?;
        text.lines().next()?.split(',').any(|c| c == "trick_req").then_some((p, text))
    }) else {
        println!(
            "no capture of {RECORDING2}'s playback with the trick columns in {} — the recipe is in the \
             S30 journal entry",
            dir.display()
        );
        return;
    };
    let Some(table) = table_bytes() else { return };
    let tables = rider::Tables::build();
    let Some(phy) = recording2_phy(&tables) else {
        println!("no {RECORDING2_PHY} in any bike archive on this disc: the human's tricks are unchecked");
        return;
    };
    let cap = replay::Capture::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", cpath.display()));
    let col = |name: &str| {
        cap.columns
            .iter()
            .position(|c| c == name)
            .unwrap_or_else(|| panic!("no column {name} in {}", cpath.display()))
    };
    let int = |row: usize, at: usize| -> i64 {
        cap.rows[row][at].parse().unwrap_or_else(|_| panic!("row {row}: {:?}", cap.rows[row][at]))
    };
    let float = |row: usize, at: usize| -> f32 {
        cap.rows[row][at].parse().unwrap_or_else(|_| panic!("row {row}: {:?}", cap.rows[row][at]))
    };
    let (dt_at, ground_at, wreck_at, fresh_at) = (col("dt"), col("on_ground"), col("wreck_reason"), col("fresh"));
    let at = ROW_COLUMNS.map(|n| col(n));
    let want = |k: usize| Row {
        group: int(k, at[0]) as u8,
        dir: int(k, at[1]) as u8,
        armed: int(k, at[2]) != 0,
        requested: int(k, at[3]) as u8,
        current: int(k, at[4]) as u8,
        seq: int(k, at[5]) as i32,
        running: int(k, at[6]) != 0,
        safe: int(k, at[7]) != 0,
        elapsed: float(k, at[8]).to_bits(),
    };
    let wreck = |k: usize| int(k, wreck_at);

    // Where the capture sits in the recording, and the proof that its pad is the human's: row k's pad
    // bytes are the recording's video field 2k - 2, buttons and both sticks, on every row. Under the
    // reader's phases that is `First`, tick k - 1.
    let first = int(0, col("frame_counter")) as u64;
    assert_eq!(first, RECORDING2_START + 1, "the capture does not start on the recording's first tick");
    let movie = replay::P2m2::parse(&bytes).unwrap_or_else(|e| panic!("{}: {e}", rpath.display()));
    let tape = movie.tape(replay::TickPhase::First);
    let from_ram = replay::PadTape::from_capture(&cap).unwrap_or_else(|e| panic!("{}: {e}", cpath.display()));
    let rows = cap.rows.len().min(tape.len() + 1);
    let off: Vec<usize> = (1..rows)
        .filter(|&k| from_ram.frames[k - 1].reports[0].0[2..8] != tape.frames[k - 1].reports[0].0[2..8])
        .collect();
    assert!(off.is_empty(), "rows whose pad is not field 2k - 2: {:?}", &off[..off.len().min(10)]);

    let stunts = Stunts::parse(&table).expect("the stunt table");
    let set = Set::new(stunts.selection(0));
    let params = Params {
        overlap: phy.animation.stunt_overlap_time,
        no_hander_speed: phy.air.no_hander_wreck_speed_limit,
    };
    let angles = AngleTable::build();

    let mut manager = game_sim::input::Manager::connected();
    let mut tricks = Tricks::default();
    let mut fresh = int(0, fresh_at) != 0;
    let mut bad: std::collections::BTreeMap<&str, Vec<usize>> = Default::default();
    let mut halfway = Vec::new();
    // The index: every request, with the gesture that made it; the first tick each became safe and
    // the tick it was over; and every tick the landing clause called.
    let mut asked: Vec<(usize, u8, Dir, Trick)> = Vec::new();
    let (mut queued, mut dpad_ticks) = (0usize, 0usize);
    let mut gestures: std::collections::BTreeMap<(u8, u8), usize> = Default::default();
    let mut landings: Vec<(usize, Option<game_sim::trick::Wreck>, i64)> = Vec::new();
    let mut lines = Vec::new();
    for k in 0..rows {
        let dt = F(float(k, dt_at));
        let frame = if k == 0 { replay::PadFrame::default() } else { tape.frames[k - 1] };
        manager.tick(dt, &frame.reports);
        let hands = Controls::from_pad(manager.pad(0));
        dpad_ticks += usize::from(hands.dpad.iter().any(|&d| d));
        let was_wrecked = k > 0 && wreck(k - 1) != 0;
        let respawned = k > 0 && int(k, fresh_at) != 0 && int(k - 1, fresh_at) == 0;
        let before = Row::of(&tricks);
        let gesture = tricks.gesture;
        let waiting = tricks.queue.len();

        // The update phase, on the gesture the last tick read.
        let mut bike = Bike {
            on_ground: int(k, ground_at) != 0,
            contact: false,
            wrecked: was_wrecked,
            fresh,
            speed: F::ZERO,
        };
        if bike.wrecked || !tricks.request(&set, bike, params) {
            tricks.idle(&mut bike);
        }
        fresh = bike.fresh;
        tricks.update(dt, &set, bike, false, game_sim::ride::noise);
        if tricks.requested != Trick::NONE && (tricks.requested.0 != before.requested || tricks.seq != before.seq) {
            asked.push((k, gesture.group, gesture.dir, tricks.requested));
        }
        queued += tricks.queue.len().saturating_sub(waiting);
        // The wreck test, while the rider is still on: the trick's clause after the attitude ones.
        if !was_wrecked {
            let verdict = tricks.landing(&set, bike, params);
            if verdict.is_some() || wreck(k) != 0 {
                landings.push((k, verdict, wreck(k)));
            }
        }
        // Back on without a respawn: the trick the wreck interrupted goes.
        if was_wrecked && wreck(k) == 0 && !respawned {
            tricks.reset();
        }
        // The input phase, after all of it: the pad this tick, and the rider still down on the tick a
        // respawn puts it back.
        let buttons = game_sim::trick::Buttons { trick: hands.trick, dpad: hands.dpad };
        tricks.gesture.read(&angles, hands.stick, buttons, wreck(k) != 0 || respawned);
        if tricks.gesture.group != 0 && tricks.gesture.dir != Dir::None {
            *gestures.entry((tricks.gesture.group, tricks.gesture.dir as u8)).or_default() += 1;
        }

        let (got, wanted) = (Row::of(&tricks), want(k));
        let mut d = got.differs(&wanted);
        // The one difference allowed for: no direction in the original, one in the port, with no trick
        // button down — so nothing could be asked for with it — and the stick moving through the
        // half-way ring (the stick's bytes differ from the tick before).
        let moving = k > 1 && tape.frames[k - 1].reports[0].0[6..8] != tape.frames[k - 2].reports[0].0[6..8];
        if d == ["trick_dir"] && wanted.dir == 0 && wanted.group == 0 && moving {
            halfway.push(k);
            d.clear();
        }
        if !d.is_empty() && lines.len() < 12 {
            lines.push(format!("row {k}: {d:?}\n  port     {got:?}\n  original {wanted:?}"));
        }
        for n in d {
            bad.entry(n).or_default().push(k);
        }
    }

    // The index, printed whatever the verdict.
    println!(
        "{}: {rows} rows, the pad field 2k - 2 on every one; {queued} queued",
        cpath.display()
    );
    let wheelies: Vec<usize> = asked.iter().filter(|a| a.3 == Trick::WHEELIE).map(|a| a.0).collect();
    println!("  the idle wheelie, one tick each: rows {wheelies:?}");
    for &(k, group, dir, trick) in asked.iter().filter(|a| a.3 != Trick::WHEELIE) {
        let name = set.stunt(trick).map(|s| s.name.as_str()).unwrap_or("?");
        let over = (k + 1..rows).find(|&j| int(j, at[6]) == 0);
        let safe = (k..over.unwrap_or(rows)).find(|&j| int(j, at[7]) != 0);
        println!(
            "  row {k}: group {group} {dir:?} -> slot {} {name:?}, safe at {safe:?}, over at {over:?}",
            trick.0
        );
    }
    for &(k, verdict, reason) in &landings {
        println!("  row {k}: the trick clause says {verdict:?}, the original's reason is {reason}");
    }
    println!("  the stick half way with no button down, no direction in the original: rows {halfway:?}");

    if !bad.is_empty() {
        let counts: Vec<String> = bad.iter().map(|(n, ks)| format!("{n} on {} rows", ks.len())).collect();
        panic!("the port's trick machine left the original's: {}\n{}", counts.join(", "), lines.join("\n"));
    }
    let slots: Vec<u8> = asked.iter().filter(|a| a.3 != Trick::WHEELIE).map(|a| a.3.0).collect();
    assert_eq!(slots, [0, 2, 1, 0, 15, 2], "the human's six tricks, in order");
    assert!(halfway.len() <= 12, "more half-way stick ticks than the twelve S30 measured: {halfway:?}");
    // The landing clause and the original agree about every touchdown it was asked about: it says a
    // trick wreck exactly where the original's reason turns 12, and nothing where the rider stayed on.
    for &(k, verdict, reason) in &landings {
        let new = k > 0 && wreck(k - 1) == 0 && reason != 0;
        let trick = new && reason == i64::from(game_sim::trick::Wreck::TRICK.0);
        assert_eq!(
            verdict.is_some(),
            trick,
            "row {k}: the trick clause says {verdict:?} and the original's reason is {reason}"
        );
    }

    // The human's gestures, and whether the shell's keyboard can make each. The keyboard binds no
    // d-pad: the arrows are the left stick, ramped to the stop (0 or 255) in under a fifth of a
    // second, and the three trick buttons are keys of their own. So a gesture is the keyboard's if
    // the group's buttons and that stick at its stop read as the same slot.
    assert_eq!(dpad_ticks, 0, "the human used the d-pad, which the keyboard does not have");
    let mut cannot = Vec::new();
    for (&(group, dir), &ticks) in &gestures {
        // The trick buttons the group is made of, in the decoder's order, and the controls the
        // shell's pad layer reads them from ([`Controls::from_pad`]).
        let held = (0u8..8)
            .map(|b| [b & 1 != 0, b & 2 != 0, b & 4 != 0])
            .find(|&t| game_sim::trick::group(t) == group)
            .expect("every group is some buttons");
        let controls: Vec<Control> = [Control::Circle, Control::Triangle, Control::R1]
            .into_iter()
            .zip(held)
            .filter_map(|(c, on)| on.then_some(c))
            .collect();
        let mask = game_sim::input::mask_of(&controls);
        let mut report = Report::from_buttons(mask);
        let (x, y) = match dir {
            1 => (127, 0),
            2 => (127, 255),
            3 => (0, 127),
            _ => (255, 127),
        };
        report.0[6] = x;
        report.0[7] = y;
        let mut keys = game_sim::input::Manager::connected();
        keys.tick(replay::CAPPED_DT, &[report, Report::missing()]);
        let c = Controls::from_pad(keys.pad(0));
        let mut g = game_sim::trick::Gesture::default();
        g.read(&angles, c.stick, game_sim::trick::Buttons { trick: c.trick, dpad: c.dpad }, false);
        let same = g.group == group && g.dir as u8 == dir;
        println!(
            "  gesture group {group} x {:?}: {ticks} ticks, slot {:?}; the keyboard {}",
            dir_of(u32::from(dir)),
            slot_of(group, dir_of(u32::from(dir))),
            if same { "makes it" } else { "cannot" }
        );
        if !same {
            cannot.push((group, dir));
        }
    }
    assert!(cannot.is_empty(), "gestures the keyboard cannot make: {cannot:?}");
}
