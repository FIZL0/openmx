//! MD107 — **MD39a's five car drives replayed bit-exact** through SLUS-21104's car frame.
//!
//! Each drive starts from its RAM image (`context/mxvatv-ps2/md39a/<car>.ee.bin`: the buggy's and
//! baja's are row 0 of the capture word for word, the other three's the frame before) and runs
//! SLUS-21104's per-frame sim update in the interpreter, the frame that steps the car (MD106), with
//! the car's input step's words fed from the tape (`context/recordings/md39a/drive.p2m2.ticks.csv`)
//! through the port's own input step ([`atv_car::input`], proved on the game's by
//! [`the_car_input_step_is_the_ports`]) at the drive's row lag (two for the buggy and baja, one for
//! the others; with the image's frame that is tape tick `j - 2` for frame `j` on all five). Nothing
//! of the capture's is laid on the machine. Each row's car object, body, engine and four tires must
//! come out word for word, but for those the sim frame never writes (the put-back spot, a position
//! breadcrumb and bit 0x400 of +0x44, all laid outside it). A row the recorder read while the frame
//! ran (torn) must hold every word at some moment about its frame: a frame's end, or the start or end
//! of the vehicle's force, a tire's step or one of its parts, the aero, or a force or moment add.
//!
//! Skips without the images, the captures or the tape.

use game_sim::atv_car::{self, Input, InputKeys};
use game_sim::bindings::Command;
use ps2_float::F;
use ps2_oracle::Ee;
use std::collections::HashMap;

#[path = "mxvatv_car_replay/drive.rs"]
mod drive;
use drive::*;

/// The blocks compared, as (name, address in the image, bytes): the car object, its body, engine and
/// the tires' raw blocks.
fn blocks(ee: &Ee, car: u32) -> Vec<(String, u32, u32)> {
    let p = |o: u32| ee.u32_at(car + o);
    let mut v = vec![("obj".to_string(), car, 0x1800), ("body".to_string(), p(0xb8), 640), ("engine".to_string(), p(0x1250), 512)];
    for k in 0..4 {
        v.push((format!("tire{k}.raw"), p(0x1254 + 4 * k), 768));
    }
    v
}

fn row_block(row: &Row, name: &str) -> Vec<u32> {
    if name == "obj" {
        return hex_words(&row["obj0"]).into_iter().chain(hex_words(&row["obj1"])).collect();
    }
    hex_words(&row[name])
}

/// The words that differ from the row, as (block, offset, machine, capture), less those the frame
/// never writes.
fn diff(ee: &Ee, car: u32, row: &Row) -> Vec<(String, u32, u32, u32)> {
    let mut out = Vec::new();
    for (name, at, len) in blocks(ee, car) {
        let want = row_block(row, &name);
        for i in 0..len / 4 {
            let (off, got, w) = (4 * i, ee.u32_at(at + 4 * i), want[i as usize]);
            let outside = name == "obj" && (SPOT.contains(&off) || CRUMB.contains(&off) || (off == FLAGS && (got ^ w) == FLAG));
            if got != w && !outside {
                out.push((name.clone(), off, got, w));
            }
        }
    }
    out
}

/// The car's words the sim frame never writes: the put-back spot and its orientation (laid by the
/// put-back outside the frame), a breadcrumb of the car's position (dropped every so far driven,
/// outside the frame), and a flag word's bit 0x400.
const SPOT: std::ops::Range<u32> = 0xd38..0xd70;
const CRUMB: std::ops::Range<u32> = 0xd90..0xd9c;
const FLAGS: u32 = 0x44;
const FLAG: u32 = 0x400;


// --- the input step ---------------------------------------------------------------------------------

/// SLUS-21104's car input step (a0 the car, a1 the player's record: the player, the control
/// records, stick x, stick y), the gate it asks first (the player record's "is this pad live"), the
/// binding lookup and the front end it is handed (research/oracle.txt `# MD104`, `# MD107`).
const INPUT_STEP: u32 = 0x004c_a010;
const LIVE: u32 = 0x0024_9330;
const BINDING: u32 = 0x0011_8e10;
const FRONT_END: u32 = 0x003e_ab60;
/// The commands the step looks up: gas, brake, hand brake, clutch, the d-pad.
const ASKED: [u8; 8] = [0x0e, 0x0f, 0x18, 0x10, 0, 1, 2, 3];

/// A deterministic generator for the cases (tests/biplane.rs's).
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
    /// A stick: often a signed zero or an end, else anywhere in -1..1, now and then a tiny one.
    fn stick(&mut self) -> f32 {
        match self.next() % 10 {
            0 => 0.0,
            1 => -0.0,
            2 => 1.0,
            3 => -1.0,
            4 => (self.unit() - 0.5) * 1e-12,
            _ => 2.0 * self.unit() - 1.0,
        }
    }
    /// A control's strength: mostly off or full, sometimes between, sometimes -0 or negative.
    fn strength(&mut self) -> f32 {
        match self.next() % 9 {
            0..=3 => 0.0,
            4 | 5 => 1.0,
            6 => -0.0,
            7 => -self.unit(),
            _ => self.unit(),
        }
    }
    /// A key or a measure: a signed zero, the image's own, or something either side of it.
    fn near(&mut self, own: f32) -> f32 {
        match self.next() % 8 {
            0 => 0.0,
            1 => -0.0,
            2 => own,
            3 => -own,
            4 => 1e-30,
            _ => own * 3.0 * (self.unit() - 0.25),
        }
    }
}

/// **SLUS-21104's car input step is [`atv_car::input`]**: ten thousand cases on the buggy's image,
/// each a player (0 or 1), sticks with signed zeros, the ends and tiny values, a strength in every
/// control record the binding lookup can name and in the pad word's, the speed either side of
/// `DigitalThrottleSpeed`, the slip either side of `DigitalSideSlipAngle` (both signs),
/// `MaxAnalogThrottle` with signed zeros, and the words the step keeps (the d-pad flag, the first
/// gas, the stick) on or off. The image's own binding table answers the lookup; the nine words the
/// step writes must equal the port's bit for bit.
#[test]
fn the_car_input_step_is_the_ports() {
    let Some(mut ee) = image(BUGGY.name) else { return };
    let car = BUGGY.car;
    ee.hooks.insert(LIVE, Box::new(|ee: &mut Ee| ee.set(2, 1)));
    ee.set_u32(car + at::LOCAL, 1);
    let front = ee.u32_at(ee.u32_at(FRONT_END));
    let own = keys_of(&ee, car);
    let (record, controls) = (0x01f8_0000u32, 0x01f8_0100u32);
    let mut pick = Pick(0x6d64_3130_37);
    for case in 0..10_000 {
        let player = (pick.next() % 2) as u32;
        let (x, y) = (pick.stick(), pick.stick());
        // Every slot the lookup can land on, a miss (-1) included, and the pad word's (+0x31c).
        let strengths: Vec<f32> = (0..35).map(|_| pick.strength()).collect();
        for (k, &s) in strengths.iter().enumerate() {
            ee.set_f32(controls + 0x18 * k as u32 + 4, s);
        }
        for (k, v) in [player, controls + 0x18, x.to_bits(), y.to_bits()].into_iter().enumerate() {
            ee.set_u32(record + 4 * k as u32, v);
        }
        let keys = InputKeys {
            max_analog_throttle: F(pick.near(own.max_analog_throttle.0)),
            digital_throttle_speed: F(pick.near(own.digital_throttle_speed.0)),
            digital_side_slip_angle: F(pick.near(own.digital_side_slip_angle.0)),
        };
        let speed = F(pick.near(own.digital_throttle_speed.0));
        let side_slip = F(pick.near(own.digital_side_slip_angle.0));
        let before = Input {
            stick: F(pick.stick()),
            lean: F(pick.stick()),
            pad_word: F(pick.stick()),
            gas_pedal: F(pick.strength()),
            vehicle_brake: F(pick.strength()),
            hand_brake: F(pick.strength()),
            clutch: F(pick.strength()),
            dpad: pick.next() % 2 == 0,
            gassed: pick.next() % 2 == 0,
        };
        for (o, v) in [
            (at::MAX_ANALOG_THROTTLE, keys.max_analog_throttle),
            (at::DIGITAL_THROTTLE_SPEED, keys.digital_throttle_speed),
            (at::DIGITAL_SIDE_SLIP_ANGLE, keys.digital_side_slip_angle),
            (at::SPEED, speed),
            (at::SIDE_SLIP, side_slip),
        ] {
            ee.set_u32(car + o, v.to_bits());
        }
        put_input(&mut ee, car, &before);
        let slots: HashMap<u8, i32> = ASKED
            .into_iter()
            .map(|c| {
                ee.set(28, GP);
                ee.call(BINDING, &[u64::from(front), u64::from(c), 2, 10, u64::from(player)], &[], BUDGET);
                (c, ee.r(2) as u32 as i32)
            })
            .collect();
        let value = |c: Command| {
            let k = *slots.get(&c.0).unwrap_or_else(|| panic!("the port asks for command {c:?}"));
            assert!((-1..33).contains(&k), "case {case}: {c:?} looks up control {k}");
            F(strengths[(k + 1) as usize])
        };
        let mut want = before;
        atv_car::input(&keys, &mut want, speed, side_slip, (F(x), F(y)), F(strengths[34]), value);
        ee.set(28, GP);
        ee.call(INPUT_STEP, &[u64::from(car), u64::from(record)], &[], BUDGET);
        let got = input_of(&ee, car);
        assert_eq!(
            format!("{got:?}"),
            format!("{want:?}"),
            "case {case} (stick {x:e}, {y:e}, player {player}, speed {speed:?}, slip {side_slip:?}, keys {keys:?}, before {before:?}): the game, then the port"
        );
        for (name, g, w) in [
            ("stick", got.stick, want.stick),
            ("lean", got.lean, want.lean),
            ("pad word", got.pad_word, want.pad_word),
            ("gas", got.gas_pedal, want.gas_pedal),
            ("brake", got.vehicle_brake, want.vehicle_brake),
            ("hand brake", got.hand_brake, want.hand_brake),
            ("clutch", got.clutch, want.clutch),
        ] {
            assert_eq!(g.to_bits(), w.to_bits(), "case {case}: {name} {g:?} in the game, {w:?} in the port");
        }
    }
}

// --- the drives -------------------------------------------------------------------------------------


/// Every word compared off the machine, the two the frame never writes masked.
fn words(ee: &Ee, car: u32) -> Vec<u32> {
    blocks(ee, car).iter().flat_map(|(n, at, len)| (0..len / 4).map(move |i| masked(n, 4 * i, ee.u32_at(at + 4 * i)))).collect()
}

fn row_words(ee: &Ee, car: u32, row: &Row) -> Vec<u32> {
    blocks(ee, car).iter().flat_map(|(n, _, len)| row_block(row, n).into_iter().take((len / 4) as usize).enumerate().map(move |(i, w)| masked(n, 4 * i as u32, w))).collect()
}

fn masked(name: &str, off: u32, w: u32) -> u32 {
    match (name, off) {
        ("obj", o) if SPOT.contains(&o) || CRUMB.contains(&o) => 0,
        ("obj", FLAGS) => w & !FLAG,
        _ => w,
    }
}

/// What a replay came to: the rows that agreed with their own frame, the stale ones (the frame
/// before's), the late ones (the frame after's), the torn ones (read while a frame ran: every word
/// that is not its own frame's is one the game held at some point from the frame before's end to the
/// frame after's, a substep's start included, the recorder's poll landing mid-way through the game's
/// frame), and the first row that was none of these, with its differences against its own frame. No
/// word of the capture's is ever laid on the machine, so a row equal again after a torn one shows the
/// machine never left the drive.
#[derive(Default)]
struct Verdict {
    rows: usize,
    agreed: usize,
    stale: Vec<usize>,
    late: Vec<usize>,
    torn: Vec<usize>,
    /// The words torn rows held from another moment than their frame's end, as (word, rows).
    torn_words: std::collections::BTreeMap<String, usize>,
    /// Rows read inside a tire's step after it cleared a word and before it wrote it again.
    cleared: Vec<usize>,
    parted: Option<(usize, String)>,
}


/// The Force body's calls the words are kept about, each substep: the vehicle's force (its first; a
/// jump to the vehicle base's), each tire's step and the aero (its last, before the body integrates).
const VEHICLE_FORCE: u32 = 0x004a_4ef0;
const VEHICLE_FORCE_TO: u32 = 0x0049_3f60;
const TIRE_STEP: u32 = 0x004b_5ca0;
const AERO: u32 = 0x004c_b200;
/// The rigid body's force and moment adds (a tire's step adds into the body's accumulators more than
/// once, and the recorder can read between two adds).
const ADD_FORCE: u32 = 0x0049_4000;
const ADD_MOMENT: u32 = 0x0049_7530;
/// The tire step's three parts (its contact, its forces, their hand-out).
const TIRE_PARTS: [u32; 3] = [0x004b_5cf0, 0x004b_62c0, 0x004b_6400];
/// The tire words its step clears before it writes them, two stores apart or more (the contact flag,
/// cleared and set again by the first part; the quad at +0x230, cleared whole by the third, then x and
/// z apart; research/oracle.txt `# MD107`): a row the recorder read in between holds a 0 no kept
/// moment does.
const CLEARED: [&str; 5] = [".raw+0xb0", ".raw+0x230", ".raw+0x234", ".raw+0x238", ".raw+0x23c"];

type Seen = std::rc::Rc<std::cell::RefCell<Vec<Vec<u32>>>>;

/// Hooks the Force body's calls to keep the compared words before and after each (into the returned
/// list), each run as it was: nothing the frame does changes.
fn watch_substeps(ee: &mut Ee, car: u32) -> Seen {
    let seen = Seen::default();
    for (at, to) in [(VEHICLE_FORCE, VEHICLE_FORCE_TO), (TIRE_STEP, TIRE_STEP), (AERO, AERO), (ADD_FORCE, ADD_FORCE), (ADD_MOMENT, ADD_MOMENT)].into_iter().chain(TIRE_PARTS.map(|p| (p, p))) {
        let keep = seen.clone();
        ee.hooks.insert(
            at,
            Box::new(move |ee: &mut Ee| {
                keep.borrow_mut().push(words(ee, car));
                let (sp, ra, stack) = (ee.r(29), ee.r(31), ee.stack);
                let ints: Vec<u64> = (4..12).map(|k| ee.r(k)).collect();
                let floats: Vec<f32> = (12..20).map(|k| ee.f(k)).collect();
                ee.stack = (sp as u32 - 0x100) & !0xf;
                ee.call(to, &ints, &floats, BUDGET);
                (ee.stack, ee.pc) = (stack, at);
                ee.set(29, sp);
                ee.set(31, ra);
                keep.borrow_mut().push(words(ee, car));
            }),
        );
    }
    seen
}

/// Replays a drive in the interpreter from its image, frame by frame; every row is compared with the
/// frame its vsync names. The buggy's and baja's images are their row 0; the other three's are the
/// frame before (their row 0 is the first frame stepped), so frame `j` shows row `j - 1` and its
/// input is tape tick `j - 1 - lag`: two frames back for every drive.
fn interpret(d: &Drive) -> Option<Verdict> {
    let mut ee = hooked(image(d.name)?);
    let (rows, ticks) = (rows(d.name)?, tape()?);
    assert_eq!(ee.u32_at(d.car + 4), d.vtable, "{}: no car at {:#x}", d.name, d.car);
    let s = sim(&ee);
    let behind = usize::from(!diff(&ee, d.car, &rows[0]).is_empty());
    let back = d.lag + behind;
    assert_eq!(back, 2, "{}: frame j's input is not tape tick j - 2", d.name);
    // Every drive's row 0 reads the vsync one tick on from the image's (the recorder's poll lands
    // on the next tick's count, or after the next frame).
    let v0 = rows[0]["vsync"].parse::<usize>().unwrap();
    assert_eq!(v0, ee.u32_at(VSYNC) as usize + 2, "{}: row 0 is not the tick after the image", d.name);
    let at: Vec<usize> = rows.iter().map(|row| (row["vsync"].parse::<usize>().unwrap() - v0) / 2 + behind).collect();
    let names: Vec<String> = blocks(&ee, d.car).iter().flat_map(|(n, _, len)| (0..len / 4).map(move |i| format!("{n}+{:#x}", 4 * i))).collect();
    let seen = watch_substeps(&mut ee, d.car);
    // Every frame's words at its end (the image's first, one past the last row's), and the words as
    // each of its substeps started (none kept for the image's).
    let (mut frames, mut subs, mut j) = (vec![words(&ee, d.car)], vec![Vec::new()], 0);
    let last = at.iter().copied().max().unwrap_or(0) + 1;
    let mut v = Verdict { rows: rows.len(), ..Verdict::default() };
    for (r, &f) in at.iter().enumerate() {
        while j < (f + 1).min(last) {
            step(&mut ee, s, d, &ticks, &mut j, back);
            frames.push(words(&ee, d.car));
            subs.push(std::mem::take(&mut *seen.borrow_mut()));
            // Only the frames about the row are looked at again.
            if j >= 3 {
                subs[j - 3] = Vec::new();
            }
        }
        let want = row_words(&ee, d.car, &rows[r]);
        let own = &frames[f];
        let before = f.checked_sub(1).map(|b| &frames[b]);
        let after = frames.get(f + 1);
        // Every moment from the frame before's end to the frame after's.
        let moments: Vec<&Vec<u32>> = before.into_iter().chain(&subs[f]).chain([own]).chain(subs.get(f + 1).into_iter().flatten()).chain(after).collect();
        if want == *own {
            v.agreed += 1;
        } else if before == Some(&want) {
            v.stale.push(r);
        } else if after == Some(&want) {
            v.late.push(r);
        } else if (0..want.len()).all(|i| moments.iter().any(|m| m[i] == want[i])) {
            v.torn.push(r);
            for i in (0..want.len()).filter(|&i| want[i] != own[i]) {
                *v.torn_words.entry(names[i].clone()).or_default() += 1;
            }
        } else {
            let unheld: Vec<usize> = (0..want.len()).filter(|&i| moments.iter().all(|m| m[i] != want[i])).collect();
            // Every word no kept moment holds is one the tire's step clears, one still the clear's 0
            // (another may be written already: row 686 of the sandrail's holds the new x, z still 0).
            let mid = unheld.iter().all(|&i| CLEARED.iter().any(|c| names[i].ends_with(c)));
            if mid && unheld.iter().any(|&i| want[i] == 0) {
                v.cleared.push(r);
                continue;
            }
            let bad: Vec<String> = unheld
                .iter()
                .take(40)
                .map(|&i| format!("{}: {:#010x} ({}) want {:#010x} ({})", names[i], own[i], f32::from_bits(own[i]), want[i], f32::from_bits(want[i])))
                .collect();
            v.parted = Some((r, format!("held at no moment about frame {f}:\n  {}", bad.join("\n  "))));
            break;
        }
    }
    Some(v)
}

fn interpreted(d: &Drive) {
    let Some(v) = interpret(d) else { return };
    let words: Vec<String> = v.torn_words.iter().map(|(w, n)| format!("{w} ({n})")).collect();
    println!(
        "{}: {} rows agree; stale rows {:?}, late rows {:?}, torn rows {:?}; words torn: {}",
        d.name,
        v.agreed,
        v.stale,
        v.late,
        v.torn,
        words.join(", ")
    );
    println!("{}: rows read between a tire's clear and its write {:?}", d.name, v.cleared);
    if let Some((r, dd)) = &v.parted {
        panic!("{} row {r} of {}: differs\n  {dd}", d.name, v.rows);
    }
    assert_eq!(v.agreed + v.stale.len() + v.late.len() + v.torn.len() + v.cleared.len(), v.rows, "{}: rows unaccounted for", d.name);
    assert!(v.stale.len() + v.late.len() <= v.rows / 20, "{}: {} stale and late rows of {}: re-record it", d.name, v.stale.len() + v.late.len(), v.rows);
}

/// **The buggy's drive replays bit-exact**: every row's car object, body, engine and tires, off the
/// image and the tape through the port's input step.
#[test]
fn the_buggys_drive_replays_bit_exact() {
    interpreted(&BUGGY);
}

/// **The baja's.**
#[test]
fn the_bajas_drive_replays_bit_exact() {
    interpreted(&BAJA);
}

/// **The sandrail's.**
#[test]
fn the_sandrails_drive_replays_bit_exact() {
    interpreted(&SANDRAIL);
}

/// **The golf cart's.**
#[test]
fn the_golf_carts_drive_replays_bit_exact() {
    interpreted(&GOLF);
}

/// **The monster truck's.**
#[test]
fn the_trucks_drive_replays_bit_exact() {
    interpreted(&TRUCK);
}


