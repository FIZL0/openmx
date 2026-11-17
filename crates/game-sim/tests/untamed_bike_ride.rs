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

use ps2_float::fpu;
use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;

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

/// Replays a capture: each row's reader words in, the engine's tick, one frame, the physics
/// compared, the spot in. A row that fails while the last tick's slow-motion step had another fit is
/// run again on that one. Answers the rows' wreck counts as the interpreter left them.
fn replay(name: &str) -> Option<Vec<u32>> {
    let mut ee = hooked(image()?);
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
            let d = diff(&ee, row);
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
