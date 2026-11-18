//! MD92 — MX vs. ATV Untamed's (SLUS-21701) MX ride with a wreck, replayed bit-exact by the original's
//! own frame.
//!
//! The captures (`context/capture/md92/wreck-1` and `wreck-3`, two clean runs of the tape
//! `context/recordings/md92/wreck.p2m2` off copy 9's save state 8, `context/untamed/ram/state08`;
//! wreck-2 is torn) hold, every 30 Hz tick, the bike object and its body, frame node, collision record,
//! both wheels and the engine. Here the interpreter runs SLUS-21701's per-frame sim update (the twin
//! of SLUS-21104's that tests/mxvatv_bike_ride.rs runs: the +0x24 list's vehicle frames, the physics
//! world's step, the tail, the +0x30 list) off state 8, and every row's body, node, record, wheels and
//! engine must come out word for word: 665 rows, the wreck on row 340.
//!
//! What runs outside this frame in the game, and how it is had here:
//! - the pad reader's outputs ([`READER`], [`PAD_RECORD`]): fed from the rows;
//! - the put-back spot ([`SPOT`]): fed from the rows, row r's into frame r + 1;
//! - the spatial database's visit mark ([`VISIT_MARK`]): fed so the body's stamp is the row's;
//! - the engine's dt: 0.03333 times the slow-motion factor, which the wreck ramps to 0.18 and the
//!   next button down ramps back. The game's own step and switch run in the interpreter; only their
//!   wall-clock milliseconds (33 or 34) are picked to fit the rows (see [`engine_tick`]).
//!
//! This is the per-row oracle a port of Untamed's bike frame (`Handling::Untamed`, the untamed_tricks
//! pack) is held to, as F31's interpreter replay was for SLUS-21104's. Skips without the image, the
//! captures or the tape.
//!
//! MD92a holds the ports already made to it: each of `handling::untamed`'s seven (the vehicle update, the
//! air state, the backflip, the 360, the wreck entry, the put-back and the preload) runs in the frame in
//! its original's place, the original's calls out made from the port's callouts, and both captures stay
//! bit-exact, the bike object's words too (but [`UNFED`]). The reader words fed from the rows are the
//! tape's ([`tape_words`]).

use game_sim::air::{self, AcosTable, Air, Chassis, Params, Spin, SpinTuning};
use game_sim::handling::FlipClip;
use game_sim::handling::untamed::{self, AirCall, AirState, Call, FlipIn, PutBack, SpinIn, Update, UpdateCall, WreckWords};
use ps2_float::{F, Vf, fpu};
use ps2_oracle::{Ee, Kit, skip};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

/// State 8's player bike (vtable 0x003f1d10, `Veh\mx_Race250P.phy`).
const BIKE: u32 = 0x01be_47f0;
const BIKE_VTABLE: u32 = 0x003f_1d10;
/// The sim object whose frame is [`FRAME`]; the physics world is at +0x48.
const SIM: u32 = 0x006d_2440;
const FRAME: u32 = 0x0020_4ab0;
/// The engine's clamped frame time, 0.03333.
const DT: u32 = 0x3d08_850a;
/// The application object: +0x13c the frame's dt and +0x140 its inverse (the sim reads them), +0x150
/// the frame's raw wall-clock time (whole ms × 0.001).
const APP: u32 = 0x0040_eb00;
/// The slow-motion object's pointer: +0x34 the time factor the engine multiplies dt by, +0x38/+0x3c
/// its bounds, +0x40 its rate a second, +0x44 its target. The wreck sets a target of 0.18 over 1 s.
const SLOW_MOTION: u32 = 0x0046_2ff8;
/// Its step, run each tick after the dt is taken and before the sim: factor += rate × raw time,
/// clamped to the bounds, while the factor is not the target.
const SLOW_MOTION_STEP: u32 = 0x0022_e350;
/// The slow motion's switch (a0 the wreck settings, *0x003ad698: +0x161c the factor, +0x1620 the
/// time in, +0x1624 the time out; a1 on). The bike's event handler 0x004cb960 turns it off when a
/// button goes down while the bike is wrecked (its state's kind 1, +0xc70 a reason); the event comes
/// from the input layer, outside the sim frame.
const SLOW_MOTION_SWITCH: u32 = 0x001c_3d70;
const WRECK_SETTINGS: u32 = 0x003a_d698;
/// The bike's ride clock, which adds the frame's dt every frame; it tells the raw times apart.
const RIDE_CLOCK: u32 = 0x448;
/// The ELF's `.reginfo` gp.
const GP: u64 = 0x003f_d070;
/// The bike object's bytes in a row (obj0..obj2).
const OBJ_LEN: u32 = 0x1c00;

/// The pad reader's stores, bike-relative: brake/gas in; the d-pad, sticks, holds and the limiter's
/// inputs; the put-back request; the pad record the backflip reads; +0x17ec.
const READER: [(u32, u32); 6] =
    [(0x340, 8), (0x1290, 0x20), (0x12b4, 0x18), (0x12d0, 8), (0x14dc, 4), (0x17ec, 4)];
/// The pad record (+0x14f4) the reader fills.
const PAD_RECORD: (u32, u32) = (0x14f4, 0x1c);
/// The spatial database's visit mark (a halfword): each search bumps it and stamps what it visits
/// with it, the bike's body (+0x260) and object (+0x1a70) among them. The game's other searches bump
/// it between frames (3 or 4 times a tick on this ride), so it is fed (see [`fed_mark_frame`]).
const VISIT_MARK: u32 = 0x003f_51b0;
/// The put-back spot, bike-relative.
const SPOT: [(u32, u32); 1] = [(0xd28, 0x48)];

/// `jal` EIntr.
const EINTR_CALL: u32 = 0x0c0c_dddc;
/// The scratch slot of the timer's stand-in word.
const COUNTER_K: u32 = 250;

type Row = HashMap<String, String>;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("untamed/ram").join(format!("state08.ee{}", ".bin"));
    if !path.exists() {
        skip("no context/untamed/ram/state08 RAM image (MD92's; ~/repos/openmx-slots/kept-context/md92)");
        return None;
    }
    let ee = Ee::from_ram(&path).unwrap();
    assert_eq!(ee.u32_at(BIKE + 4), BIKE_VTABLE, "{} is not state 8", path.display());
    Some(ee)
}

fn csv(path: std::path::PathBuf) -> Option<Vec<Row>> {
    let Ok(text) = std::fs::read_to_string(&path) else {
        skip(&format!("no {}", path.display()));
        return None;
    };
    let mut lines = text.lines();
    let head: Vec<String> = lines.next()?.split(',').map(String::from).collect();
    Some(lines.map(|l| head.iter().cloned().zip(l.split(',').map(String::from)).collect()).collect())
}

fn rows(name: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("capture/md92").join(format!("{name}{}", ".csv")))
}

fn ticks() -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("recordings/md92").join(format!("wreck.p2m2.ticks{}", ".csv")))
}

fn hex_words(s: &str) -> Vec<u32> {
    (0..s.len() / 8)
        .map(|i| {
            let b = |j: usize| u8::from_str_radix(&s[8 * i + 2 * j..8 * i + 2 * j + 2], 16).unwrap();
            u32::from_le_bytes([b(0), b(1), b(2), b(3)])
        })
        .collect()
}

/// The bike object's words in a row.
fn obj(row: &Row) -> Vec<u32> {
    let o: Vec<u32> = ["obj0", "obj1", "obj2"].iter().flat_map(|k| hex_words(&row[*k])).collect();
    assert_eq!(o.len() as u32, OBJ_LEN / 4);
    o
}

/// The physics blocks, as (name, address in the image, bytes).
fn physics(ee: &Ee) -> Vec<(&'static str, u32, u32)> {
    let p = |o: u32| ee.u32_at(BIKE + o);
    vec![
        ("body", p(0xe8), 640),
        ("node", p(0x114), 256),
        ("record", p(0xf8), 256),
        ("front", p(0x17f0), 0x920),
        ("rear", p(0x17f4), 0x920),
        ("engine", p(0x143c), 512),
    ]
}

fn diff(ee: &Ee, row: &Row) -> Vec<String> {
    let mut out = Vec::new();
    for (name, at, len) in physics(ee) {
        let want = hex_words(&row[name]);
        for i in 0..len / 4 {
            let got = ee.u32_at(at + 4 * i);
            let w = want[i as usize];
            if got != w {
                out.push(format!(
                    "{name}+{:#x}: {got:#010x} ({}) want {w:#010x} ({})",
                    4 * i,
                    f32::from_bits(got),
                    f32::from_bits(w)
                ));
            }
        }
    }
    out
}

/// The bike object's words the frame does not come out with (MD92a, the same on both captures): the
/// spot and the saved place (fed after the frame, or written outside it), the wreck's and the rider's
/// words from row 340 on, and words the game's other updates write (from rows 0 to 150). Each
/// was measured, not traced.
const UNFED: [(u32, u32); 25] = [
    (0x74, 0x4), (0x490, 0x4), (0x71c, 0x8), (0x890, 0xc), (0x8a0, 0xc), (0x8b0, 0xc), (0x940, 0x4),
    (0x950, 0xc), (0x960, 0x4), (0x970, 0xc), (0x980, 0x20), (0xc74, 0x4), (0xc7c, 0x4), (0xd28, 0x4),
    (0xd30, 0xc), (0xd40, 0x4), (0xd48, 0x4), (0xd50, 0x10), (0xd80, 0xc), (0x1a70, 0x4), (0x1a80, 0xc),
    (0x1a9c, 0x4), (0x1ad0, 0xc), (0x1bc0, 0xc), (0x1bd0, 0xc),
];

/// The bike object's words, but [`UNFED`]'s, that differ from the row's (MD92a: where the ports'
/// words are).
fn obj_diff(ee: &Ee, row: &Row) -> Vec<String> {
    let o = obj(row);
    (0..OBJ_LEN)
        .step_by(4)
        .filter(|&at| !UNFED.iter().any(|&(from, len)| (from..from + len).contains(&at)))
        .filter_map(|at| {
            let (got, w) = (ee.u32_at(BIKE + at), o[at as usize / 4]);
            (got != w).then(|| format!("bike+{at:#x}: {got:#010x} ({}) want {w:#010x} ({})", f32::from_bits(got), f32::from_bits(w)))
        })
        .collect()
}

fn poke(ee: &mut Ee, row: &Row, words: &[(u32, u32)]) {
    let o = obj(row);
    for &(at, len) in words {
        for i in 0..len / 4 {
            ee.set_u32(BIKE + at + 4 * i, o[(at / 4 + i) as usize]);
        }
    }
}

fn hooked(mut ee: Ee) -> Ee {
    // The kernel's DIntr and EIntr (mfc0 on Status): "were off".
    ee.hooks.insert(0x0033_7720, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0033_7770, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    // libgcc's 64-bit multiply (its `madd` and `pmfhl.lw` are not the interpreter's).
    ee.hooks.insert(0x0034_a3e0, Box::new(|ee: &mut Ee| {
        let v = ee.r(4).wrapping_mul(ee.r(5));
        ee.set(2, v);
    }));
    // The profiling reads of RCNT0's count: `lui rA, 0x1000`, then `jal EIntr` with `lw rT, 0(rA)` in
    // its delay slot. Each reads a stand-in word (0) instead.
    let at = ee.scratch(COUNTER_K);
    ee.set_u32(at, 0);
    let mut timers = 0;
    for a in (0x0010_0000..0x005e_0000u32).step_by(4) {
        let x = ee.u32_at(a);
        if x >> 21 != 0x1e0 || x & 0xffff != 0x1000 {
            continue;
        }
        let ra = (x >> 16) & 31;
        for i in 1..4 {
            let y = ee.u32_at(a + 4 * i);
            if y >> 26 == 0x23 && (y >> 21) & 31 == ra && y & 0xffff == 0 && ee.u32_at(a + 4 * i - 4) == EINTR_CALL {
                ee.set_u32(a, x & 0xffff_0000 | (at.wrapping_add(0x8000) >> 16));
                ee.set_u32(a + 4 * i, y | (at & 0xffff));
                timers += 1;
                break;
            }
        }
    }
    assert_eq!(timers, 111, "the profiling timer reads");
    // Every `addiu $3, $zero, n; syscall; jr $ra`: the answer is a0, the thread id (0x2f) 1.
    for a in (0x0010_0000..0x005e_0000u32).step_by(4) {
        if ee.u32_at(a) >> 16 == 0x2403 && ee.u32_at(a + 4) == 0x0000_000c && ee.u32_at(a + 8) == 0x03e0_0008 {
            let n = ee.u32_at(a) & 0xffff;
            ee.hooks.insert(a, Box::new(move |ee: &mut Ee| {
                let v = if n == 0x2f { 1 } else { ee.r(4) };
                ee.set(2, v);
            }));
        }
    }
    ee
}

/// Runs one frame.
fn frame(ee: &mut Ee) {
    ee.set(28, GP);
    let dt = f32::from_bits(ee.u32_at(APP + 0x13c));
    ee.call(FRAME, &[u64::from(SIM)], &[dt], 400_000_000);
}

/// The slow-motion factor.
fn time_factor(ee: &Ee) -> f32 {
    f32::from_bits(ee.u32_at(ee.u32_at(SLOW_MOTION) + 0x34))
}

/// A step of the slow motion: the raw time and the object it leaves.
type Step = (i32, Vec<u32>);

/// The engine's part of a tick before the sim: dt = 0.03333 × the factor (floor 0.001) into the
/// application, the slow motion off when a button went down on a wreck, then the factor's step.
/// The step's raw time is the wall clock's (33 or 34 ms on PCSX2), which only shows in the next
/// frame's dt, so it is picked so that frame's dt adds up to the next row's ride clock; while the factor sits on its target the step does nothing and the
/// image's raw time stays. Answers every step that fits (the first one taken); two can fit when
/// their dts differ below the clock's last bit, and then only the next frame tells them apart.
fn engine_tick(ee: &mut Ee, pressed: bool, clock: u32, next_clock: Option<u32>, at: &str) -> Vec<Step> {
    let dt = fpu::mul(f32::from_bits(DT), time_factor(ee)).max(0.001);
    ee.set_u32(APP + 0x13c, dt.to_bits());
    ee.set_u32(APP + 0x140, fpu::div(1.0, dt).to_bits());
    if pressed && ee.u32_at(ee.u32_at(BIKE + 0xbb4) + 0x28) == 1 && ee.u32_at(BIKE + 0xc70) != 0 {
        ee.set(28, GP);
        ee.call(SLOW_MOTION_SWITCH, &[u64::from(ee.u32_at(WRECK_SETTINGS)), 0, 0], &[], 1_000_000);
    }
    let slow = ee.u32_at(SLOW_MOTION);
    if ee.u32_at(slow + 0x34) == ee.u32_at(slow + 0x44) {
        return Vec::new();
    }
    let saved = slow_motion(ee);
    let mut fits: Vec<Step> = Vec::new();
    for ms in [33, 34] {
        set_slow_motion(ee, &(ms, saved.clone()));
        ee.set(28, GP);
        ee.call(SLOW_MOTION_STEP, &[u64::from(slow)], &[], 100_000);
        let next_dt = fpu::mul(f32::from_bits(DT), time_factor(ee)).max(0.001);
        let after = slow_motion(ee);
        if next_clock.is_none_or(|n| fpu::add(f32::from_bits(clock), next_dt).to_bits() == n)
            && fits.iter().all(|(_, o)| o[0x34 / 4] != after[0x34 / 4])
        {
            fits.push((ms, after));
        }
    }
    assert!(!fits.is_empty(), "{at}: no raw time steps the slow motion to the next ride clock");
    set_slow_motion(ee, &fits[0]);
    fits
}

/// The slow-motion object's words.
fn slow_motion(ee: &Ee) -> Vec<u32> {
    let slow = ee.u32_at(SLOW_MOTION);
    (0..0x48).step_by(4).map(|k| ee.u32_at(slow + k)).collect()
}

/// Puts a step's raw time and object back.
fn set_slow_motion(ee: &mut Ee, (ms, words): &Step) {
    let slow = ee.u32_at(SLOW_MOTION);
    for (k, &w) in (0..0x48).step_by(4).zip(words) {
        ee.set_u32(slow + k, w);
    }
    ee.set_u32(APP + 0x150, fpu::mul(fpu::from_i32(*ms), 0.001).to_bits());
}

/// The bike body's visit stamp.
fn body_stamp(ee: &Ee) -> u16 {
    ee.u16_at(ee.u32_at(BIKE + 0xe8) + 0x260)
}

/// Runs one frame with the visit mark fed so the body comes out with `stamp`: first as if the frame
/// bumps the mark once before it reaches the body (or not at all, when the body keeps its stamp),
/// and when it bumps it more, again from the same RAM with the mark that many lower. The mark never goes back.
fn fed_mark_frame(ee: &mut Ee, stamp: u16, at: &str) {
    let start = ee.u16_at(VISIT_MARK);
    let (ram, spr) = (ee.ram.clone(), ee.spr.clone());
    // A body the frame does not stamp (it keeps the last one) leaves the mark where it is.
    let mut guess = if body_stamp(ee) == stamp { start } else { stamp.wrapping_sub(1) };
    for _ in 0..2 {
        assert!(guess.wrapping_sub(start) < 0x8000, "{at}: the visit mark would go back ({start:#x} -> {guess:#x})");
        ee.write(VISIT_MARK, 2, u128::from(guess));
        frame(ee);
        let got = body_stamp(ee);
        if got == stamp {
            return;
        }
        ee.ram.copy_from_slice(&ram);
        ee.spr.copy_from_slice(&spr);
        guess = guess.wrapping_add(stamp.wrapping_sub(got));
    }
    panic!("{at}: the body's visit stamp is not {stamp:#x} with the mark at {guess:#x}");
}

/// Replays a capture: each row's reader words in, the engine's tick, one frame, the physics and the
/// bike object (but [`UNFED`]) compared, the spot in. A row that fails while the last tick's slow-motion step had another fit is
/// run again on that one. Answers the rows' wreck counts as the interpreter left them.
fn replay(name: &str) -> Option<Vec<u32>> {
    replay_with(hooked(image()?), name)
}

/// [`replay`] on `ee`, the image with its hooks.
fn replay_with(mut ee: Ee, name: &str) -> Option<Vec<u32>> {
    let rows = rows(name)?;
    let clock = |r: usize| obj(&rows[r])[RIDE_CLOCK as usize / 4];
    // Tape tick t is read on row t; a button down is one held now and not on the last tick.
    let tape = ticks()?;
    let pressed = |r: usize| r > 0 && !tape[r]["buttons"].is_empty() && tape[r - 1]["buttons"].is_empty();
    let mut out = Vec::new();
    let mut others: Vec<Step> = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let at = format!("{name} row {r} (vsync {})", row["vsync"]);
        poke(&mut ee, row, &READER);
        poke(&mut ee, row, &[PAD_RECORD]);
        let stamp = hex_words(&row["body"])[0x260 / 4] as u16;
        let (ram, spr) = (ee.ram.clone(), ee.spr.clone());
        let mut tries = std::iter::once(None).chain(std::mem::take(&mut others).into_iter().map(Some));
        let d = loop {
            let Some(step) = tries.next() else { unreachable!() };
            if let Some(step) = &step {
                ee.ram.copy_from_slice(&ram);
                ee.spr.copy_from_slice(&spr);
                set_slow_motion(&mut ee, step);
            }
            // The ride clock after this frame is this row's, and the step's dt shows in the next.
            let fits = engine_tick(&mut ee, pressed(r), clock(r), rows.get(r + 1).map(|_| clock(r + 1)), &at);
            if r == 0 {
                assert_eq!(ee.u16_at(VISIT_MARK), stamp.wrapping_sub(1), "{at}: state 8's visit mark");
                frame(&mut ee);
            } else {
                fed_mark_frame(&mut ee, stamp, &at);
            }
            let mut d = diff(&ee, row);
            d.extend(obj_diff(&ee, row));
            others = fits.into_iter().skip(1).collect();
            if d.is_empty() || tries.size_hint().1 == Some(0) {
                break d;
            }
        };
        assert!(
            d.is_empty(),
            "{at}: {} words differ\n  {}",
            d.len(),
            d.iter().filter(|l| !l.starts_with("body") && !l.starts_with("node")).chain(d.iter()).take(60).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let count = ee.u32_at(BIKE + 0x708);
        assert_eq!(count, obj(row)[0x708 / 4], "{at}: the wreck count");
        out.push(count);
        poke(&mut ee, row, &SPOT);
    }
    Some(out)
}

#[test]
fn wreck_ride_replays_bit_exact() {
    let Some(w) = replay("wreck-1") else { return };
    assert_eq!(w.len(), 665);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [340], "the one wreck, on row 340");
}

/// The second clean run of the same tape off state 8 (wreck-2 is torn).
#[test]
fn second_run_replays_bit_exact() {
    let Some(w) = replay("wreck-3") else { return };
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [340], "the one wreck, on row 340");
}

// --- MD92a: the ported functions in the frame ---------------------------------------------------------

/// The originals `handling::untamed` ports, as the frame calls them (a0 the bike, f12 the dt).
const VEHICLE_UPDATE: u32 = 0x004f_9bd0;
const AIR_STATE: u32 = 0x004f_1270;
const BACKFLIP: u32 = 0x004f_d0d0;
const SPIN: u32 = 0x004f_d630;
const WRECK_ENTRY: u32 = 0x004f_a390;
const PUT_BACK: u32 = 0x004c_a900;
const PRELOAD: u32 = 0x004f_e360;
/// What the ports' callouts are, in the original.
const POST_EVENT: u32 = 0x0025_e740;
const POST_QUEUE: u32 = 0x0047_5bd0;
const RAND: u32 = 0x0034_4360;
const BASE_WRECK: u32 = 0x004c_bc60;
const GROUND_QUERY: u32 = 0x004b_0910;
const SETTLE: u32 = 0x004c_8880;
const ATTACHMENT_PUT_BACK: u32 = 0x0051_2e90;
const STEEP: u32 = 0x0050_0880;
const BINDING_PRESSED: u32 = 0x0025_2640;
const BINDING_LEFT: u32 = 0x0025_2690;
const BINDING_RIGHT: u32 = 0x0025_26e0;
const NODE_AXES: u32 = 0x0019_0580;
const BASE_UPDATE: u32 = 0x004c_b340;
const LISTENERS: u32 = 0x0012_6c50;
const FLAGS_PTR: u32 = 0x003f_f6b8;
const BALANCE_CLEARED: u32 = 0x004f_e950;
const NO_HANDER: u32 = 0x004f_3c70;
const IN_WORLD: u32 = 0x004b_a650;
const VEHICLE_POST: u32 = 0x004c_8b20;
const NODE_POSITION: u32 = 0x0019_0830;

/// A port's words in RAM, as (address, word): flags as 0 or 1. Only the words the port writes.
type Words = Vec<(u32, u32)>;

/// Writes the words of `now` that the port changed since `last` (a flag the port left as it was keeps
/// RAM's word, 1 or not), and keeps `now` as the last.
fn sync(ee: &mut Ee, last: &mut Words, now: Words) {
    for (&(at, w), &(_, was)) in now.iter().zip(last.iter()) {
        if w != was {
            ee.set_u32(at, w);
        }
    }
    *last = now;
}

fn vf(ee: &Ee, at: u32) -> Vf {
    Vf(std::array::from_fn(|k| f32::from_bits(ee.u32_at(at + 4 * k as u32))))
}
fn ff(ee: &Ee, at: u32) -> F {
    F::from_bits(ee.u32_at(at))
}
fn flag(ee: &Ee, at: u32) -> bool {
    ee.u32_at(at) != 0
}
fn vw(at: u32, v: Vf) -> [(u32, u32); 4] {
    std::array::from_fn(|k| (at + 4 * k as u32, v.0[k].to_bits()))
}

/// The bike's virtual at `slot`.
fn virt(ee: &Ee, slot: u32) -> u32 {
    ee.u32_at(ee.u32_at(BIKE + 4) + slot)
}

/// A port running in its original's place, inside the original's hook: what the hook must put back
/// when it is done, and the calls the port makes out meanwhile (each on a stack under the caller's).
struct Nest {
    addr: u32,
    ra: u64,
    sp: u64,
    stack: u32,
}

impl Nest {
    fn enter(ee: &mut Ee, addr: u32) -> Nest {
        let n = Nest { addr, ra: ee.r(31), sp: ee.r(29), stack: ee.stack };
        ee.stack = (n.sp as u32 - 0x40) & !0xf;
        n
    }
    /// Calls `at` with `ints` and `dt` in f12: answers v0.
    fn call(&self, ee: &mut Ee, at: u32, ints: &[u64], dt: F) -> u64 {
        ee.set(28, GP);
        ee.call(at, ints, &[dt.0], 200_000_000);
        ee.r(2)
    }
    fn leave(self, ee: &mut Ee) {
        (ee.stack, ee.pc) = (self.stack, self.addr);
        ee.set(29, self.sp);
        ee.set(31, self.ra);
    }
}

/// What a port asks where its original calls out but the port's callback is not handed the state.
#[derive(Clone, Copy, Debug)]
enum Ask {
    Call(Call),
    Rand,
    Ground,
    StartStunt(i32),
    Steep,
}

/// What the original's call answered, kept for the port's runs after.
#[derive(Clone, Copy, Debug)]
enum Answer {
    Done,
    Int(i32),
    Bool(bool),
    Ground(Vf, Vf),
}

/// The port's callouts so far: answered from `answers`, and the first past them stops the run.
struct Stage {
    answers: Vec<Answer>,
    at: usize,
    asked: Option<Ask>,
}

/// The unwind that stops a staged run (no panic message: [`std::panic::resume_unwind`]).
struct Stopped;

impl Stage {
    fn ask(&mut self, what: Ask) -> Answer {
        if let Some(&a) = self.answers.get(self.at) {
            self.at += 1;
            return a;
        }
        self.asked = Some(what);
        std::panic::resume_unwind(Box::new(Stopped))
    }
}

/// Runs a port whose callbacks are not handed its state so that each original call sees the state the
/// port had there: the port runs from `s0` up to its next callout, its words so far go into RAM, the
/// original's call is made (`answer`), and the port runs again from `s0` with that answer, until it
/// returns. A port is a function of its state and its callbacks' answers, so each run retraces the last.
fn staged<S: Clone, R>(
    ee: &mut Ee,
    s0: &S,
    words: impl Fn(&S) -> Words,
    run: impl Fn(&mut S, &RefCell<Stage>) -> R,
    mut answer: impl FnMut(&mut Ee, &S, Ask) -> Answer,
) -> (S, R) {
    let mut last = words(s0);
    let mut answers = Vec::new();
    loop {
        let mut s = s0.clone();
        let stage = RefCell::new(Stage { answers: answers.clone(), at: 0, asked: None });
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut s, &stage)));
        sync(ee, &mut last, words(&s));
        match r {
            Ok(r) => return (s, r),
            Err(e) => {
                let Some(what) = stage.borrow_mut().asked.take() else { std::panic::resume_unwind(e) };
                answers.push(answer(ee, &s, what));
            }
        }
    }
}

/// Every tuning zero; the hooks read the ones each port reads.
fn zero_params() -> Params {
    Params {
        airborne_control_moment: Vf::ZERO,
        airborne_damping: F::ZERO,
        airborne_control_force: F::ZERO,
        brake_gyro_torque: F::ZERO,
        unroll_gain: F::ZERO,
        unroll_torque: F::ZERO,
        unyaw_gain: F::ZERO,
        unyaw_torque: F::ZERO,
        control_authority_time: F::ZERO,
        preload_time: F::ZERO,
        max_preload: F::ZERO,
        min_preload_time: F::ZERO,
        max_preload_time: F::ZERO,
        min_dpad_preload_time: F::ZERO,
        max_dpad_preload_time: F::ZERO,
        min_backflip_time: F::ZERO,
        max_backflip_time: F::ZERO,
        min_backflip_torque: F::ZERO,
        max_backflip_torque: F::ZERO,
        backflip_torque_rate_increase: F::ZERO,
        backflip_torque_rate_decrease: F::ZERO,
        front_wreck_speed_limit: F::ZERO,
        back_wreck_speed_limit: F::ZERO,
        side_wreck_speed_limit: F::ZERO,
        racing_down_wreck_speed_limit: F::ZERO,
        freestyle_down_wreck_speed_limit: F::ZERO,
        ground_pitch_down_wreck_angle: F::ZERO,
        ground_pitch_up_wreck_angle: F::ZERO,
        airborne_pitch_down_wreck_angle: F::ZERO,
        airborne_pitch_up_wreck_angle: F::ZERO,
        airborne_roll_wreck_angle: F::ZERO,
        no_hander_wreck_speed_limit: F::ZERO,
        no_hander_max_roll_angle: F::ZERO,
        joust_wreck_height: F::ZERO,
        crossup_wreck: F::ZERO,
        crossup_wreck_time: F::ZERO,
        rotation_speed: None,
    }
}

fn body(ee: &Ee) -> u32 {
    ee.u32_at(BIKE + 0xe8)
}

/// The body's own add (its vt `+0x80` a force, `+0x78` a moment) of the quad at `from` on the bike,
/// which also tells the body's listeners; the ports add straight into the body.
fn body_add(ee: &mut Ee, n: &Nest, slot: u32, from: u32) {
    let b = body(ee);
    let at = ee.u32_at(ee.u32_at(b + 0x110) + slot);
    n.call(ee, at, &[u64::from(b), u64::from(BIKE + from)], F::ZERO);
}

/// The step words the backflip, the 360 and the preload read.
fn peek_step(ee: &Ee, dt: F) -> air::Step {
    air::Step {
        dt,
        brake: ff(ee, BIKE + 0x340),
        stick: (ff(ee, BIKE + 0x1298), ff(ee, BIKE + 0x129c)),
        stick_hold: ff(ee, BIKE + 0x12a0),
        dpad_y: flag(ee, BIKE + 0x1294),
        spinning: flag(ee, BIKE + 0x11c0),
        spin_active: flag(ee, BIKE + 0x11c8),
        touching: flag(ee, ee.u32_at(BIKE + 0xf8) + 0x34),
        ..air::Step::default()
    }
}

/// The trick buttons as the reader keeps them (pad record bytes 18, 20, 22 and 24).
fn trick_bytes(ee: &Ee) -> [bool; 4] {
    std::array::from_fn(|k| ee.u8_at(BIKE + 0x14f4 + 18 + 2 * k as u32) != 0)
}

/// Whether the controls bind the device's stunt control (the binding table's byte for the pad
/// record's device).
fn bound(ee: &Ee) -> bool {
    ee.u8_at(ee.u32_at(BIKE + 0x1510) + 36 + ee.u32_at(BIKE + 0x14f4)) != 0
}

#[derive(Clone)]
struct PreloadState {
    p: Params,
    air: Air,
    ch: Chassis,
    st: air::Step,
    axis: Option<F>,
    cue: bool,
}

fn preload_words(s: &PreloadState) -> Words {
    let a = &s.air;
    let mut w = vec![
        (BIKE + 0xc40, u32::from(a.preload_ready)),
        (BIKE + 0xc44, u32::from(a.preload_firing)),
        (BIKE + 0xc48, u32::from(a.preload_armed)),
        (BIKE + 0xc50, a.preload_timer.to_bits()),
        (BIKE + 0x724, u32::from(s.cue)),
    ];
    w.extend(vw(BIKE + 0xc60, a.preload_force));
    w
}

/// The preload's hook: the port, then its force through the body's add while it fires, then the cue's
/// post.
fn port_preload(ee: &mut Ee) {
    let n = Nest::enter(ee, PRELOAD);
    let dt = F::from_bits(ee.fpr[12]);
    let mut p = zero_params();
    p.preload_time = ff(ee, BIKE + 0xc4c);
    p.min_preload_time = ff(ee, BIKE + 0xc54);
    p.max_preload_time = ff(ee, BIKE + 0xc58);
    p.max_preload = ff(ee, BIKE + 0xc5c);
    p.min_dpad_preload_time = ff(ee, BIKE + 0x1150);
    p.max_dpad_preload_time = ff(ee, BIKE + 0x1154);
    let air = Air {
        preload_ready: flag(ee, BIKE + 0xc40),
        preload_firing: flag(ee, BIKE + 0xc44),
        preload_armed: flag(ee, BIKE + 0xc48),
        preload_timer: ff(ee, BIKE + 0xc50),
        preload_force: vf(ee, BIKE + 0xc60),
        on_ground: flag(ee, BIKE + 0x354),
        contact_vel: vf(ee, BIKE + 0xc20),
        ..Air::default()
    };
    let b = body(ee);
    let ch = Chassis { vel: vf(ee, b + 0x80), force: vf(ee, b + 0xa0), ..Chassis::default() };
    let axis = flag(ee, BIKE + 0x12c8).then(|| ff(ee, BIKE + 0x12b4));
    let s0 = PreloadState { p, air, ch, st: peek_step(ee, dt), axis, cue: flag(ee, BIKE + 0x724) };
    let mut s = s0.clone();
    let posted = untamed::preload(&s.p, &mut s.air, &mut s.ch, &s.st, s.axis, &mut s.cue);
    let mut last = preload_words(&s0);
    sync(ee, &mut last, preload_words(&s));
    if s.ch.force.to_bits() != s0.ch.force.to_bits() {
        body_add(ee, &n, 0x80, 0xc60);
        assert_eq!(vf(ee, b + 0xa0).to_bits(), s.ch.force.to_bits(), "the preload's force, the body's add against the port's");
    }
    if posted {
        let queue = ee.u32_at(POST_QUEUE);
        let tag = ee.u32_at(BIKE + 0xd10);
        n.call(ee, POST_EVENT, &[u64::from(queue), u64::from(BIKE + 0x698), 8, u64::from(untamed::PRELOAD_EVENT), u64::from(tag), 0], dt);
    }
    n.leave(ee);
}

#[derive(Clone)]
struct EntryState {
    air: Air,
    w: WreckWords,
    angular: Vf,
}

/// `angular` the body's angular velocity's address.
fn entry_words(s: &EntryState, angular: u32) -> Words {
    let mut w = vec![
        (BIKE + 0xc40, u32::from(s.air.preload_ready)),
        (BIKE + 0xc44, u32::from(s.air.preload_firing)),
        (BIKE + 0xdd8, s.w.hold),
        (BIKE + 0x1714, u32::from(s.w.entered)),
        (BIKE + 0x17c4, u32::from(s.w.reasoned)),
        (BIKE + 0x708, s.w.count),
    ];
    w.extend(vw(angular, s.angular));
    w
}

/// The wreck entry's hook (vt `+0x368`, a1 the reason).
fn port_wreck_entry(ee: &mut Ee) {
    let n = Nest::enter(ee, WRECK_ENTRY);
    let reason = ee.r(5) as u32;
    let air = Air {
        preload_ready: flag(ee, BIKE + 0xc40),
        preload_firing: flag(ee, BIKE + 0xc44),
        preload_armed: flag(ee, BIKE + 0xc48),
        ..Air::default()
    };
    let w = WreckWords {
        hold: ee.u32_at(BIKE + 0xdd8),
        entered: flag(ee, BIKE + 0x1714),
        reasoned: flag(ee, BIKE + 0x17c4),
        count: ee.u32_at(BIKE + 0x708),
    };
    let angular = body(ee) + 0x1d0;
    let s0 = EntryState { air, w, angular: vf(ee, angular) };
    staged(
        ee,
        &s0,
        |s| entry_words(s, angular),
        |s, stage| {
            untamed::wreck_entry(&mut s.air, &mut s.w, &mut s.angular, reason, || {
                let Answer::Int(v) = stage.borrow_mut().ask(Ask::Rand) else { unreachable!() };
                v
            }, |c| {
                stage.borrow_mut().ask(Ask::Call(c));
            })
        },
        |ee, _, what| {
            let bike = u64::from(BIKE);
            match what {
                Ask::Rand => return Answer::Int(n.call(ee, RAND, &[], F::ZERO) as i32),
                Ask::Call(Call::EndStunts) => n.call(ee, virt(ee, 0x4a0), &[bike], F::ZERO),
                Ask::Call(Call::StopClip) => n.call(ee, virt(ee, 0x4a8), &[bike, u64::MAX], F::ZERO),
                Ask::Call(Call::PhysicsOff) => n.call(ee, virt(ee, 0x398), &[bike, 0], F::ZERO),
                Ask::Call(Call::WreckRays) => n.call(ee, virt(ee, 0x484), &[bike], F::ZERO),
                Ask::Call(Call::BaseWreck(r)) => n.call(ee, BASE_WRECK, &[bike, u64::from(r)], F::ZERO),
                other => panic!("the wreck entry does not ask {other:?}"),
            };
            Answer::Done
        },
    );
    n.leave(ee);
}

/// Where the put-back's ground query leaves its normal, and the 360's node its axes: the interpreter's
/// scratch slots over the top-level stack.
const NORMAL_K: u32 = 1;
const AXES_K: u32 = 2;
const ORIGIN_K: u32 = 4;

/// `driver` the bike's driver.
fn put_back_words(pb: &PutBack, driver: u32) -> Words {
    let mut w: Words = [vw(BIKE + 0x80, pb.position), vw(BIKE + 0x90, pb.orientation), vw(BIKE + 0xa0, pb.up)].concat();
    w.push((BIKE + 0xdd8, pb.hold));
    if let Some(d) = pb.driver_respawn {
        w.extend(d.iter().enumerate().map(|(i, &x)| (driver + 0x94 + 4 * i as u32, x)));
    }
    w
}

/// The put-back's hook (a1 snap, a2 keep).
fn port_put_back(ee: &mut Ee, top: u32) {
    let n = Nest::enter(ee, PUT_BACK);
    let (snap, keep) = (ee.r(5) != 0, ee.r(6) != 0);
    let driver = ee.u32_at(BIKE + 0x6a4);
    let words3 = |at: u32| [ee.u32_at(at), ee.u32_at(at + 4), ee.u32_at(at + 8)];
    let pb = PutBack {
        position: vf(ee, BIKE + 0x80),
        orientation: vf(ee, BIKE + 0x90),
        up: vf(ee, BIKE + 0xa0),
        saved_position: vf(ee, BIKE + 0xd30),
        saved_orientation: vf(ee, BIKE + 0xd40),
        saved_up: vf(ee, BIKE + 0xd50),
        respawn: words3(BIKE + 0xd60),
        driver_respawn: (driver != 0).then(|| words3(driver + 0x94)),
        hold: ee.u32_at(BIKE + 0xdd8),
        saved_hold: ee.u32_at(BIKE + 0xddc),
        has_ground: flag(ee, BIKE + 0x3e4),
        has_attachment: flag(ee, BIKE + 0x10cc),
    };
    let normal = top + 0x40 + NORMAL_K * 0x40;
    staged(
        ee,
        &pb,
        |pb| put_back_words(pb, driver),
        |pb, stage| {
            untamed::put_back(pb, snap, keep, |at| {
                let Answer::Ground(p, normal) = stage.borrow_mut().ask(Ask::Ground) else { unreachable!() };
                *at = p;
                normal
            }, |c| {
                stage.borrow_mut().ask(Ask::Call(c));
            })
        },
        |ee, _, what| {
            let bike = u64::from(BIKE);
            match what {
                Ask::Call(Call::Ground) => {}
                Ask::Ground => {
                    let ground = ee.u32_at(BIKE + 0x3e4);
                    n.call(ee, GROUND_QUERY, &[u64::from(ground), u64::from(BIKE + 0x80), u64::from(normal), 0, 0], F::ZERO);
                    return Answer::Ground(vf(ee, BIKE + 0x80), vf(ee, normal));
                }
                Ask::Call(Call::Settle) => _ = n.call(ee, SETTLE, &[bike], F::ZERO),
                Ask::Call(Call::RideRays) => _ = n.call(ee, virt(ee, 0x39c), &[bike], F::ZERO),
                Ask::Call(Call::KeptRays) => _ = n.call(ee, virt(ee, 0x3a0), &[bike], F::ZERO),
                Ask::Call(Call::Attachment) => {
                    let a = ee.u32_at(BIKE + 0x10cc);
                    n.call(ee, ATTACHMENT_PUT_BACK, &[u64::from(a)], F::ZERO);
                }
                Ask::Call(Call::Post(id)) => {
                    let (queue, tag) = (ee.u32_at(POST_QUEUE), ee.u32_at(BIKE + 0xd10));
                    n.call(ee, POST_EVENT, &[u64::from(queue), u64::from(BIKE + 0x698), 8, u64::from(id), u64::from(tag), 0], F::ZERO);
                }
                other => panic!("the put-back does not ask {other:?}"),
            }
            Answer::Done
        },
    );
    n.leave(ee);
}

#[derive(Clone)]
struct FlipState {
    air: Air,
    ch: Chassis,
    clip: FlipClip,
}

fn flip_words(s: &FlipState) -> Words {
    let a = &s.air;
    let mut w = vec![
        (BIKE + 0x1178, a.authority.to_bits()),
        (BIKE + 0x1184, u32::from(a.flip_torque_applied)),
        (BIKE + 0x1188, u32::from(a.flipping)),
        (BIKE + 0x118c, u32::from(a.flip_active)),
        (BIKE + 0x1190, u32::from(a.flip_armed)),
        (BIKE + 0x119c, a.flip_grace.to_bits()),
    ];
    w.extend(vw(BIKE + 0x11b0, a.flip_torque));
    w
}

/// The backflip's hook. The port's deferred outputs are the original's calls: the clip's stores
/// (vt `+0x530`), the stunt's start (vt `+0x4b0`) and end (vt `+0x4a4`), the wreck (vt `+0x368`); its
/// moment the body's add (vt `+0x78`); a cleared stunt id 255.
fn port_backflip(ee: &mut Ee) {
    let n = Nest::enter(ee, BACKFLIP);
    let dt = F::from_bits(ee.fpr[12]);
    let mut p = zero_params();
    p.min_backflip_time = ff(ee, BIKE + 0x1194);
    p.max_backflip_time = ff(ee, BIKE + 0x1198);
    p.min_backflip_torque = ff(ee, BIKE + 0x11a0);
    p.max_backflip_torque = ff(ee, BIKE + 0x11a4);
    p.backflip_torque_rate_decrease = ff(ee, BIKE + 0x11ac);
    p.control_authority_time = ff(ee, BIKE + 0x117c);
    p.airborne_pitch_down_wreck_angle = ff(ee, BIKE + 0x1414);
    p.airborne_pitch_up_wreck_angle = ff(ee, BIKE + 0x1418);
    let air = Air {
        on_ground: flag(ee, BIKE + 0x354),
        contact_vel: vf(ee, BIKE + 0xc20),
        authority: ff(ee, BIKE + 0x1178),
        flip_torque_applied: flag(ee, BIKE + 0x1184),
        flipping: flag(ee, BIKE + 0x1188),
        flip_active: flag(ee, BIKE + 0x118c),
        flip_armed: flag(ee, BIKE + 0x1190),
        flip_grace: ff(ee, BIKE + 0x119c),
        flip_torque: vf(ee, BIKE + 0x11b0),
        ..Air::default()
    };
    let b = body(ee);
    let ch = Chassis {
        vel: vf(ee, b + 0x80),
        moment: vf(ee, b + 0x1b0),
        pitch: ff(ee, BIKE + 0x390),
        roll: ff(ee, BIKE + 0x398),
        ..Chassis::default()
    };
    let step = peek_step(ee, dt);
    let node = ee.u32_at(BIKE + 0x114);
    let buttons = trick_bytes(ee);
    let button = bound(ee).then(|| {
        let table = ee.u32_at(BIKE + 0x1510);
        n.call(ee, BINDING_PRESSED, &[u64::from(table), u64::from(BIKE + 0x14f4)], F::ZERO) != 0
    });
    let inp = FlipIn {
        arm_button: buttons[3],
        trick_held: buttons.contains(&true),
        lip_y: ff(ee, BIKE + 0x12c4),
        hold: ff(ee, BIKE + 0x12c0),
        button,
        right_y: ff(ee, node + 0xb4),
        up_y: ff(ee, node + 0xc4),
        stunt_id: ee.i32_at(BIKE + 0x1548),
    };
    let s0 = FlipState { air, ch, clip: FlipClip(ee.i32_at(BIKE + 0x1794)) };
    let stunt = ee.u32_at(BIKE + 0x13c8);
    let (added, cleared) = (Cell::new(false), Cell::new(false));
    let add = |ee: &mut Ee, s: &FlipState| {
        if s.air.flip_torque_applied && !added.replace(true) {
            body_add(ee, &n, 0x78, 0x11b0);
            assert_eq!(vf(ee, b + 0x1b0).to_bits(), s.ch.moment.to_bits(), "the backflip's moment, the body's add against the port's");
        }
    };
    let clip = |ee: &mut Ee, c: u64| _ = n.call(ee, virt(ee, 0x530), &[u64::from(BIKE), c], F::ZERO);
    let (s, out) = staged(
        ee,
        &s0,
        flip_words,
        |s, stage| {
            untamed::backflip(&p, &mut s.air, &mut s.ch, &step, &inp, &mut s.clip, || {
                let Answer::Bool(v) = stage.borrow_mut().ask(Ask::StartStunt(stunt as i32)) else { unreachable!() };
                v
            }, || {
                let Answer::Bool(v) = stage.borrow_mut().ask(Ask::Steep) else { unreachable!() };
                v
            })
        },
        |ee, s, what| match what {
            Ask::StartStunt(word) => {
                add(ee, s);
                let v = n.call(ee, virt(ee, 0x4b0), &[u64::from(BIKE), word as i64 as u64, 1], F::ZERO) != 0;
                if v {
                    clip(ee, 1);
                }
                Answer::Bool(v)
            }
            Ask::Steep => {
                if inp.stunt_id >= untamed::FIRST_FLIP_STUNT {
                    ee.set_u32(BIKE + 0x1548, 255);
                    cleared.set(true);
                }
                clip(ee, 0);
                Answer::Bool(n.call(ee, STEEP, &[u64::from(BIKE)], F::ZERO) != 0)
            }
            other => panic!("the backflip does not ask {other:?}"),
        },
    );
    add(ee, &s);
    if out.stunt_cleared && !cleared.get() {
        ee.set_u32(BIKE + 0x1548, 255);
    }
    if (s0.clip.0, s.clip.0) == (1, 2) {
        clip(ee, 2);
    }
    if out.stunt_ended {
        n.call(ee, virt(ee, 0x4a4), &[u64::from(BIKE), u64::from(stunt)], F::ZERO);
        clip(ee, 0);
    }
    if let Some(r) = out.wreck {
        n.call(ee, virt(ee, 0x368), &[u64::from(BIKE), u64::from(r)], F::ZERO);
    }
    n.leave(ee);
}

#[derive(Clone)]
struct SpinState {
    air: Air,
    spin: Spin,
    ch: Chassis,
}

fn spin_words(s: &SpinState) -> Words {
    let sp = &s.spin;
    let mut w = vec![
        (BIKE + 0x1178, s.air.authority.to_bits()),
        (BIKE + 0x11c0, u32::from(sp.applied)),
        (BIKE + 0x11c4, u32::from(sp.ramp)),
        (BIKE + 0x11c8, u32::from(sp.active)),
        (BIKE + 0x11cc, u32::from(sp.armed)),
        (BIKE + 0x11d8, sp.grace.to_bits()),
        (BIKE + 0x1210, sp.start.to_bits()),
        (BIKE + 0x1214, sp.dir as u32),
    ];
    w.extend(vw(BIKE + 0x1200, sp.torque));
    w
}

/// The 360's hook, as the backflip's: its clip's stores are vt `+0x534`, its stunt the bike's left
/// (`+0x13cc`, a direction under 1) or right one (`+0x13d0`), its moment the body's add of `+0x1200`;
/// the node's axes (`NODE_AXES`) asked on every call with the 360 on, the binding (left, then right)
/// only when the device has one.
fn port_spin(ee: &mut Ee, top: u32, table: &AcosTable) {
    let n = Nest::enter(ee, SPIN);
    let dt = F::from_bits(ee.fpr[12]);
    let f = |o: u32| ff(ee, BIKE + o);
    let t = SpinTuning {
        min_time: f(0x11d0),
        max_time: f(0x11d4),
        min_torque_x: f(0x11dc),
        max_torque_x: f(0x11e0),
        min_torque_y: f(0x11e4),
        max_torque_y: f(0x11e8),
        rate_increase: f(0x11ec),
        rate_decrease: f(0x11f0),
    };
    let mut p = zero_params();
    p.control_authority_time = f(0x117c);
    let air = Air {
        on_ground: flag(ee, BIKE + 0x354),
        contact_vel: vf(ee, BIKE + 0xc20),
        authority: f(0x1178),
        flip_torque_applied: flag(ee, BIKE + 0x1184),
        flip_active: flag(ee, BIKE + 0x118c),
        ..Air::default()
    };
    let spin = Spin {
        applied: flag(ee, BIKE + 0x11c0),
        ramp: flag(ee, BIKE + 0x11c4),
        active: flag(ee, BIKE + 0x11c8),
        armed: flag(ee, BIKE + 0x11cc),
        grace: f(0x11d8),
        torque: vf(ee, BIKE + 0x1200),
        start: f(0x1210),
        dir: ee.i32_at(BIKE + 0x1214),
        clip: ee.u32_at(BIKE + 0x1798) as u8,
        ..Spin::default()
    };
    let b = body(ee);
    let ch = Chassis { vel: vf(ee, b + 0x80), moment: vf(ee, b + 0x1b0), ..Chassis::default() };
    let step = peek_step(ee, dt);
    let axes = top + 0x40 + AXES_K * 0x40;
    if spin.active {
        let node = ee.u32_at(BIKE + 0x114);
        n.call(ee, NODE_AXES, &[u64::from(node), 0, u64::from(axes), u64::from(axes + 0x40)], F::ZERO);
    }
    let buttons = trick_bytes(ee);
    let button = bound(ee).then(|| {
        let at = [u64::from(ee.u32_at(BIKE + 0x1510)), u64::from(BIKE + 0x14f4)];
        if n.call(ee, BINDING_LEFT, &at, F::ZERO) != 0 {
            -1
        } else {
            i32::from(n.call(ee, BINDING_RIGHT, &at, F::ZERO) != 0)
        }
    });
    let inp = SpinIn {
        arm_button: buttons[3],
        trick_held: buttons.contains(&true),
        lip_x: ff(ee, BIKE + 0x12bc),
        x: ff(ee, BIKE + 0x1298),
        hold: ff(ee, BIKE + 0x12b8),
        button,
        forward: vf(ee, axes),
        stunt_id: ee.i32_at(BIKE + 0x1548),
    };
    let s0 = SpinState { air, spin, ch };
    let stunt = |ee: &Ee, dir: i32| ee.u32_at(BIKE + if dir < 1 { 0x13cc } else { 0x13d0 });
    let (added, cleared) = (Cell::new(false), Cell::new(false));
    let add = |ee: &mut Ee, s: &SpinState| {
        if s.spin.applied && !added.replace(true) {
            body_add(ee, &n, 0x78, 0x1200);
            assert_eq!(vf(ee, b + 0x1b0).to_bits(), s.ch.moment.to_bits(), "the 360's moment, the body's add against the port's");
        }
    };
    let clip = |ee: &mut Ee, c: u64| _ = n.call(ee, virt(ee, 0x534), &[u64::from(BIKE), c], F::ZERO);
    let (s, out) = staged(
        ee,
        &s0,
        spin_words,
        |s, stage| {
            untamed::spin(&t, &p, &mut s.air, &mut s.spin, &mut s.ch, &step, &inp, table, |dir| {
                let Answer::Bool(v) = stage.borrow_mut().ask(Ask::StartStunt(dir)) else { unreachable!() };
                v
            }, || {
                let Answer::Bool(v) = stage.borrow_mut().ask(Ask::Steep) else { unreachable!() };
                v
            })
        },
        |ee, s, what| match what {
            Ask::StartStunt(dir) => {
                add(ee, s);
                let word = stunt(ee, dir);
                let v = n.call(ee, virt(ee, 0x4b0), &[u64::from(BIKE), word as i32 as i64 as u64, 1], F::ZERO) != 0;
                if v {
                    clip(ee, 1);
                }
                Answer::Bool(v)
            }
            Ask::Steep => {
                if inp.stunt_id >= untamed::FIRST_FLIP_STUNT {
                    ee.set_u32(BIKE + 0x1548, 255);
                    cleared.set(true);
                }
                clip(ee, 0);
                Answer::Bool(n.call(ee, STEEP, &[u64::from(BIKE)], F::ZERO) != 0)
            }
            other => panic!("the 360 does not ask {other:?}"),
        },
    );
    add(ee, &s);
    if out.stunt_cleared && !cleared.get() {
        ee.set_u32(BIKE + 0x1548, 255);
    }
    if (s0.spin.clip, s.spin.clip) == (1, 2) {
        clip(ee, 2);
    }
    if out.stunt_ended {
        let word = stunt(ee, s.spin.dir);
        n.call(ee, virt(ee, 0x4a4), &[u64::from(BIKE), u64::from(word)], F::ZERO);
        clip(ee, 0);
    }
    if let Some(r) = out.wreck {
        n.call(ee, virt(ee, 0x368), &[u64::from(BIKE), u64::from(r)], F::ZERO);
    }
    n.leave(ee);
}

fn peek_update(ee: &Ee) -> Update {
    let state = ee.u32_at(BIKE + 0xbb4);
    let timed = ee.u32_at(BIKE + 0x17b8);
    Update {
        vehicle_state: (state != 0).then(|| ee.u32_at(state + 0x28)),
        skip: ee.u8_at(ee.u32_at(FLAGS_PTR) + 0x11d) & 8 != 0,
        stick: (ff(ee, BIKE + 0x1298), ff(ee, BIKE + 0x129c)),
        stick_angle: ff(ee, BIKE + 0x12a8),
        stick_length: ff(ee, BIKE + 0x12ac),
        brake: ff(ee, BIKE + 0x340),
        input: ee.u32_at(BIKE + 0x12d0),
        wreck_reason: ee.u32_at(BIKE + 0xc70),
        request_countdown: ee.i32_at(BIKE + 0x17b0),
        request: ee.u32_at(BIKE + 0x34c),
        timer: ff(ee, BIKE + 0x17bc),
        timed: timed != 0 && ee.u32_at(timed + 0x30) != 0,
        cleared: ee.u32_at(BIKE + 0x1690),
    }
}

fn update_words(u: &Update) -> Words {
    vec![
        (BIKE + 0x1298, u.stick.0.to_bits()),
        (BIKE + 0x129c, u.stick.1.to_bits()),
        (BIKE + 0x12a8, u.stick_angle.to_bits()),
        (BIKE + 0x12ac, u.stick_length.to_bits()),
        (BIKE + 0x340, u.brake.to_bits()),
        (BIKE + 0x12d0, u.input),
        (BIKE + 0x17b0, u.request_countdown as u32),
        (BIKE + 0x34c, u.request),
        (BIKE + 0x17bc, u.timer.to_bits()),
        (BIKE + 0x1690, u.cleared),
    ]
}

/// The vehicle update's hook: each call the port makes is the original's, with the port's words in
/// RAM before it and the state read again after (as the port's `call` allows).
fn port_vehicle_update(ee: &mut Ee, table: &AcosTable) {
    let n = Nest::enter(ee, VEHICLE_UPDATE);
    let dt = F::from_bits(ee.fpr[12]);
    let mut u = peek_update(ee);
    let mut last = update_words(&u);
    let bike = u64::from(BIKE);
    untamed::vehicle_update(&mut u, dt, table, |c, u| {
        sync(ee, &mut last, update_words(u));
        match c {
            UpdateCall::Ghost(dt) => n.call(ee, virt(ee, 0x3a4), &[bike], dt),
            UpdateCall::WreckTest(dt) => n.call(ee, virt(ee, 0x494), &[bike, 0], dt),
            UpdateCall::StuntIdle(dt) => n.call(ee, virt(ee, 0x4e4), &[bike], dt),
            UpdateCall::Listeners(dt) => n.call(ee, LISTENERS, &[bike + 0x30], dt),
            UpdateCall::NodeFromBody => n.call(ee, virt(ee, 0x4ec), &[bike], dt),
            UpdateCall::StuntClocks(dt) => n.call(ee, virt(ee, 0x4e0), &[bike], dt),
            UpdateCall::RemoteWreck { dt, reason } => n.call(ee, virt(ee, 0x498), &[bike, u64::from(reason)], dt),
            UpdateCall::TimerOut => {
                let obj = ee.u32_at(ee.u32_at(BIKE + 0x17b8) + 0x30);
                n.call(ee, ee.u32_at(ee.u32_at(obj + 4) + 0x90), &[u64::from(obj)], dt)
            }
            UpdateCall::VehicleUpdate(dt) => n.call(ee, BASE_UPDATE, &[bike], dt),
        };
        *u = peek_update(ee);
        last = update_words(u);
    });
    sync(ee, &mut last, update_words(&u));
    n.leave(ee);
}

/// The air state's body words, as the port's [`game_sim::quad::Body`]'s fields.
const AIR_BODY: [u32; 8] = [0xa0, 0x1b0, 0x190, 0x1d0, 0x80, 0xc0, 0x70, 0xd0];

fn peek_air(ee: &Ee) -> AirState {
    let state = ee.u32_at(BIKE + 0xbb4);
    let b = body(ee);
    let bv = |o: usize| vf(ee, b + AIR_BODY[o]);
    let wheels = [ee.u32_at(BIKE + 0x17f0), ee.u32_at(BIKE + 0x17f4)];
    let f = |o: u32| ff(ee, BIKE + o);
    let w = |o: u32| ee.u32_at(BIKE + o);
    AirState {
        vehicle_state: (state != 0).then(|| ee.u32_at(state + 0x28)),
        wreck_reason: w(0xc70),
        stunt_id: w(0x1544),
        balance_id: w(0x154c),
        body: game_sim::quad::Body {
            force: bv(0),
            moment: bv(1),
            orientation: bv(2),
            ang_vel: bv(3),
            velocity: bv(4),
            reset_each_frame: bv(5),
            position: bv(6),
            correction: bv(7),
        },
        cofg: vf(ee, b + 0x1a0),
        wheel_touching: wheels.map(|at| ee.u32_at(at + 0xdc)),
        wheel_contact_vel: wheels.map(|at| vf(ee, at + 0x190)),
        speed: f(0x35c),
        local_vel: vf(ee, BIKE + 0x380),
        heading: vf(ee, BIKE + 0xbd0),
        up: vf(ee, BIKE + 0xbe0),
        euler: vf(ee, BIKE + 0x390),
        world_ang_vel: vf(ee, BIKE + 0x370),
        on_ground: w(0x354),
        both_down: w(0x1260),
        airborne: w(0xdd0),
        was_airborne: w(0xdd4),
        hang_time: f(0x1180),
        contact_vel: vf(ee, BIKE + 0xc20),
        flipping: w(0x1188),
        spin_ramp: w(0x11c4),
        authority: f(0x1178),
        authority_time: f(0x117c),
        crossup: f(0x360),
        tipped: ee.u32_at(ee.u32_at(BIKE + 0xf8) + 0x24),
        event_tag: w(0xd10),
        node_pos: vf(ee, BIKE + 0xbc0),
        node_pos_kept: vf(ee, BIKE + 0xc30),
    }
}

/// `record` the collision record.
fn air_words(a: &AirState, record: u32) -> Words {
    let mut w: Words = [
        vw(BIKE + 0x380, a.local_vel),
        vw(BIKE + 0xbd0, a.heading),
        vw(BIKE + 0xbe0, a.up),
        vw(BIKE + 0x390, a.euler),
        vw(BIKE + 0x370, a.world_ang_vel),
        vw(BIKE + 0xc20, a.contact_vel),
        vw(BIKE + 0xc30, a.node_pos_kept),
    ]
    .concat();
    w.extend([
        (BIKE + 0x35c, a.speed.to_bits()),
        (BIKE + 0x354, a.on_ground),
        (BIKE + 0x1260, a.both_down),
        (BIKE + 0xdd0, a.airborne),
        (BIKE + 0xdd4, a.was_airborne),
        (BIKE + 0x1180, a.hang_time.to_bits()),
        (BIKE + 0x360, a.crossup.to_bits()),
        (record + 0x24, a.tipped),
    ]);
    w
}

/// The air state's hook, as the vehicle update's; the in-world test is handed the port's origin in a
/// scratch quad, and its answer is the port's.
fn port_air_state(ee: &mut Ee, top: u32, table: &AcosTable) {
    let n = Nest::enter(ee, AIR_STATE);
    let dt = F::from_bits(ee.fpr[12]);
    let record = ee.u32_at(BIKE + 0xf8);
    let mut a = peek_air(ee);
    let mut last = air_words(&a, record);
    let bike = u64::from(BIKE);
    let origin = top + 0x40 + ORIGIN_K * 0x40;
    untamed::air_state(&mut a, dt, table, |c, a| {
        sync(ee, &mut last, air_words(a, record));
        let v = match c {
            AirCall::BalanceCleared => n.call(ee, BALANCE_CLEARED, &[bike], dt),
            AirCall::NoHander(dt) => n.call(ee, NO_HANDER, &[bike], dt),
            AirCall::InWorld(at) => {
                for (k, x) in at.0.iter().enumerate() {
                    ee.set_f32(origin + 4 * k as u32, *x);
                }
                n.call(ee, IN_WORLD, &[bike, u64::from(origin)], dt)
            }
            AirCall::Post { tag } => {
                let queue = ee.u32_at(POST_QUEUE);
                n.call(ee, POST_EVENT, &[u64::from(queue), bike + 0x698, 8, u64::from(untamed::OUT_OF_WORLD_EVENT), u64::from(tag), 0], dt)
            }
            AirCall::Unwreck => n.call(ee, virt(ee, 0x36c), &[bike, 1], dt),
            AirCall::VehiclePost(dt) => n.call(ee, VEHICLE_POST, &[bike], dt),
            AirCall::NodePosition => {
                let node = ee.u32_at(BIKE + 0x114);
                n.call(ee, NODE_POSITION, &[u64::from(node), 0, bike + 0xbc0], dt)
            }
        };
        *a = peek_air(ee);
        last = air_words(a, record);
        v != 0
    });
    sync(ee, &mut last, air_words(&a, record));
    n.leave(ee);
}

/// How many times each port ran in its original's place.
type Runs = Rc<RefCell<BTreeMap<&'static str, u32>>>;

/// The image with every port hooked over its original.
fn ported(mut ee: Ee) -> (Ee, Runs) {
    let runs: Runs = Rc::default();
    let table = Rc::new(AcosTable::build());
    let at = ee.u32_at(0x003f_f538);
    for (i, &e) in table.entries().iter().enumerate() {
        assert_eq!(ee.u32_at(at + 4 * i as u32), e.to_bits(), "the game's arc cosine table, entry {i}");
    }
    let top = ee.stack;
    let ports: [(&'static str, u32, Rc<dyn Fn(&mut Ee)>); 7] = [
        ("vehicle update", VEHICLE_UPDATE, { let t = table.clone(); Rc::new(move |ee: &mut Ee| port_vehicle_update(ee, &t)) }),
        ("air state", AIR_STATE, { let t = table.clone(); Rc::new(move |ee: &mut Ee| port_air_state(ee, top, &t)) }),
        ("backflip", BACKFLIP, Rc::new(port_backflip)),
        ("360", SPIN, { let t = table.clone(); Rc::new(move |ee: &mut Ee| port_spin(ee, top, &t)) }),
        ("wreck entry", WRECK_ENTRY, Rc::new(port_wreck_entry)),
        ("put-back", PUT_BACK, Rc::new(move |ee: &mut Ee| port_put_back(ee, top))),
        ("preload", PRELOAD, Rc::new(port_preload)),
    ];
    for (name, addr, port) in ports {
        let runs = runs.clone();
        ee.hooks.insert(addr, Box::new(move |ee: &mut Ee| {
            assert_eq!(ee.r(4) as u32, BIKE, "the {name} is called on the bike");
            *runs.borrow_mut().entry(name).or_default() += 1;
            port(ee);
        }));
    }
    (ee, runs)
}

/// [`replay`] with every port in its original's place: answers the wreck counts and the runs.
fn replay_ported(name: &str) -> Option<(Vec<u32>, BTreeMap<&'static str, u32>)> {
    let (ee, runs) = ported(hooked(image()?));
    let w = replay_with(ee, name)?;
    let runs = runs.borrow().clone();
    Some((w, runs))
}

/// The ports all ran: the per-frame ones every row (the backflip, the 360 and the preload each
/// substep), the wreck entry and the put-back once.
fn assert_all_ran(runs: &BTreeMap<&'static str, u32>, rows: usize) {
    for name in ["vehicle update", "air state", "backflip", "360", "preload"] {
        assert!(runs.get(name).copied().unwrap_or(0) as usize >= rows, "the {name} ran {runs:?}");
    }
    assert_eq!((runs.get("wreck entry"), runs.get("put-back")), (Some(&1), Some(&1)), "{runs:?}");
}

/// **MD92a**: wreck-1, bit-exact on every row with Untamed's seven ported functions run in the frame
/// in their originals' places.
#[test]
fn wreck_ride_replays_bit_exact_with_the_ports() {
    let Some((w, runs)) = replay_ported("wreck-1") else { return };
    assert_eq!(w.len(), 665);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [340], "the one wreck, on row 340");
    assert_all_ran(&runs, w.len());
}

/// **MD92a**: wreck-3 the same.
#[test]
fn second_run_replays_bit_exact_with_the_ports() {
    let Some((w, runs)) = replay_ported("wreck-3") else { return };
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [340], "the one wreck, on row 340");
    assert_all_ran(&runs, w.len());
}


// --- MD92a: the reader's words off the tape -----------------------------------------------------------

/// **MD92a — the reader's words off a tape tick**: what the pad reader stores for `tick` (MD92's tape
/// holds cross, square and a left stick at -1, 0 or 1 sideways, nothing else), the bike `up` (no wreck
/// reason as the frame before left it), on [`READER`]'s and [`PAD_RECORD`]'s words. Gas cross, the
/// stick, its angle (off the arc cosine table; a centred stick's is the table's middle) and length,
/// the lip's copy of the stick's side-to-side (+0x12bc), the riding word (+0x14dc) and the stick's
/// way (+0x14f8: left 3, right 4) all 0 while wrecked; brake square; the pad record's device word
/// (+0x14f4) `device`, the image's; every other word 0 (no trick button, no d-pad, no flick, no
/// preload axis on this tape).
fn tape_words(tick: &Row, up: bool, device: u32, table: &AcosTable) -> Vec<(u32, u32)> {
    let held = |k: &str| tick["buttons"].split(' ').any(|x| x == k);
    let stick = |k: &str| {
        let v: f32 = tick[k].parse().unwrap();
        assert!(v == -1.0 || v == 0.0 || v == 1.0, "MD92's tape holds the stick at -1, 0 or 1: {k} {v}");
        v
    };
    for b in tick["buttons"].split(' ').filter(|b| !b.is_empty()) {
        assert!(["cross", "square"].contains(&b), "MD92's tape presses cross and square only: {b}");
    }
    assert_eq!((stick("ly"), stick("rx"), stick("ry")), (0.0, 0.0, 0.0), "MD92's tape moves the left stick sideways only");
    let lx = if up { stick("lx") } else { 0.0 };
    let lever = |on: bool| if on { 1.0f32.to_bits() } else { 0 };
    let angle = if up { table.at(F(lx)).to_bits() } else { 0 };
    let way = if lx < 0.0 {
        3
    } else if lx > 0.0 {
        4
    } else {
        0
    };
    let mut w = vec![(0x340, lever(held("square"))), (0x344, lever(held("cross") && up))];
    for at in [0x1290..0x12b0, 0x12b4..0x12cc, 0x12d0..0x12d8, 0x17ec..0x17f0].into_iter().flat_map(|r| r.step_by(4)) {
        let v = match at {
            0x1298 | 0x12bc => lx.to_bits(),
            0x12a8 => angle,
            0x12ac => lx.abs().to_bits(),
            _ => 0,
        };
        w.push((at, v));
    }
    w.push((0x14dc, u32::from(up)));
    w.extend([(0x14f4, device), (0x14f8, way)]);
    w.extend((0x14fc..0x1510).step_by(4).map(|at| (at, 0)));
    w
}

/// **MD92a**: [`tape_words`] are the reader's words of every row, word for word, on both captures
/// (tick t on row t, the wreck state the row before's; row 0's bike, state 8's, is not wrecked), and
/// cover every word [`READER`] and [`PAD_RECORD`] poke.
#[test]
fn the_tape_gives_every_reader_word() {
    let table = AcosTable::build();
    let idle: Row = [("buttons", ""), ("lx", "0"), ("ly", "0"), ("rx", "0"), ("ry", "0")].iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let mut covered: Vec<u32> = tape_words(&idle, true, 0, &table).iter().map(|w| w.0).collect();
    covered.sort();
    let mut want: Vec<u32> = READER.iter().chain([&PAD_RECORD]).flat_map(|&(at, len)| (0..len / 4).map(move |i| at + 4 * i)).collect();
    want.sort();
    assert_eq!(covered, want, "the tape's words are READER's and the pad record's");
    let Some(ticks) = ticks() else { return };
    for name in ["wreck-1", "wreck-3"] {
        let Some(rows) = rows(name) else { return };
        let device = obj(&rows[0])[0x14f4 / 4];
        for (r, row) in rows.iter().enumerate() {
            let o = obj(row);
            let up = r == 0 || obj(&rows[r - 1])[0xc70 / 4] == 0;
            let got = tape_words(&ticks[r], up, device, &table);
            let bad: Vec<String> = got.iter().filter(|&&(at, w)| o[at as usize / 4] != w).map(|&(at, w)| format!("+{at:#x}: {w:#x}, the row {:#x}", o[at as usize / 4])).collect();
            assert!(bad.is_empty(), "{name} row {r}: {}", bad.join(", "));
        }
    }
}
