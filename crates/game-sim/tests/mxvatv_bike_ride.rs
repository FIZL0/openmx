//! F31 — C15's SLUS-21104 ride and flip, replayed bit-exact by the original's own frame.
//!
//! The captures (`context/capture/c15/ride-2.csv`, `flip-1.csv`, re-recorded by F31 from
//! `context/mxvatv-ps2/state04.ee.bin` with `research/c15/tape.py` + `play.sh`) hold, every 30 Hz
//! tick, the bike object and its body, frame node, collision record, both wheels and the engine. Here
//! the interpreter runs SLUS-21104's per-frame sim update (the sim object's vt +0x40: the list's
//! vehicle frames, the physics world's step, the tail) off state04 at the game's tick (dt bits
//! 0x3d08850a, as MX Unleashed's), and every row's body, node, record, wheels and engine must come
//! out word for word.
//!
//! What is fed from the rows, and nothing else:
//! - the pad reader's outputs ([`READER`]): the reader runs outside this frame, from the game's
//!   input pass. Its words are the frame's inputs, and `ticks_land_one_row_later` ties them to the
//!   tape;
//! - the put-back spot ([`SPOT`]), which the game keeps outside this frame too. Row r's spot goes
//!   into frame r + 1. Without it the flip's put-back lands on state04's spot.
//!
//! Everything else in the bike object is the interpreter's. The words it gets wrong are written
//! outside this frame and read by nothing in it: the run stays bit-exact without them.
//!
//! Hooks: the kernel's DIntr/EIntr; every syscall stub (the heap's semaphores, the thread id); the
//! bike's network snapshot send (vt +0x544), which reads the EE timer and writes nothing of the bike's.
//!
//! The ride: gas, brake, both leans, never wrecks. The flip: the backflip on R1 with the stick, the
//! wreck at row 125 (the collision record swaps to the wreck part), and the put-back at row 193 (back
//! to the riding part, at the saved spot). Skips without the image or the captures.

use ps2_oracle::{Ee, Kit, skip};
use std::collections::HashMap;

const BIKE: u32 = 0x00e7_14e0;
/// The sim object (vtable 0x003b9320) whose vt +0x40 is the frame; the physics world is at +0x6c.
const SIM: u32 = 0x0066_59e0;
const FRAME: u32 = 0x0020_4040;
const DT: u32 = 0x3d08_850a;
const GP: u64 = 0x003d_baf0;

/// The pad reader's stores, bike-relative: brake/gas in, d-pad, stick and holds, clutch and reverse
/// in, +0x14c8, the put-back request, the trick byte, +0x17ac.
const READER: [(u32, u32); 7] =
    [(0x310, 8), (0x12a0, 0x18), (0x12c0, 8), (0x14c8, 4), (0x14d4, 4), (0x14ec, 4), (0x17ac, 4)];
/// The put-back spot and its orientation, bike-relative.
const SPOT: [(u32, u32); 1] = [(0xd38, 0x38)];

type Row = HashMap<String, String>;

fn image() -> Option<Ee> {
    let kit = Kit::load().map_err(|w| skip(&w)).ok()?;
    let path = kit.ctx("mxvatv-ps2").join(format!("state04.ee{}", ".bin"));
    if !path.exists() {
        skip("no context/mxvatv-ps2/state04 RAM image (F31's recipe: research/journal)");
        return None;
    }
    Some(Ee::from_ram(&path).unwrap())
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
    csv(Kit::load().ok()?.ctx("capture/c15").join(format!("{name}{}", ".csv")))
}

fn ticks(tape: &str) -> Option<Vec<Row>> {
    csv(Kit::load().ok()?.ctx("recordings/c15").join(format!("{tape}.p2m2.ticks{}", ".csv")))
}

fn hex_words(s: &str) -> Vec<u32> {
    (0..s.len() / 8)
        .map(|i| {
            let b = |j: usize| u8::from_str_radix(&s[8 * i + 2 * j..8 * i + 2 * j + 2], 16).unwrap();
            u32::from_le_bytes([b(0), b(1), b(2), b(3)])
        })
        .collect()
}

/// The bike object's words in a row (obj0 then obj1).
fn obj(row: &Row) -> Vec<u32> {
    hex_words(&row["obj0"]).into_iter().chain(hex_words(&row["obj1"])).collect()
}

/// The physics blocks, as (name, address in the image, bytes).
fn physics(ee: &Ee) -> Vec<(&'static str, u32, u32)> {
    let p = |o: u32| ee.u32_at(BIKE + o);
    vec![
        ("body", p(0xb8), 640),
        ("node", p(0xe4), 256),
        ("record", p(0xc8), 256),
        ("front", p(0x17b0), 0x920),
        ("rear", p(0x17b4), 0x920),
        ("engine", p(0x142c), 512),
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
    ee.hooks.insert(0x0034_8fa0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x0034_8ff0, Box::new(|ee: &mut Ee| ee.set(2, 0)));
    ee.hooks.insert(0x004e_7e70, Box::new(|_: &mut Ee| {}));
    // Every `addiu $3, $zero, n; syscall; jr $ra`: the answer is a0, the thread id (0x2f) 1.
    for a in (0x0030_0000..0x0036_0000u32).step_by(4) {
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

/// Whether the collision record (+0xcc) is the wreck part (+0x1434) rather than the riding one
/// (+0x1438), off a bike-relative word reader.
fn wrecked(o: impl Fn(u32) -> u32) -> bool {
    let at = o(0xcc);
    assert!(at == o(0x1434) || at == o(0x1438), "record {at:#x} is neither part");
    at == o(0x1434)
}

fn row_wrecked(row: &Row) -> bool {
    let o = obj(row);
    wrecked(|at| o[at as usize / 4])
}

/// Replays a capture; answers, row by row, whether the interpreter left the bike wrecked.
fn replay(name: &str) -> Option<Vec<bool>> {
    let mut ee = hooked(image()?);
    let rows = rows(name)?;
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        poke(&mut ee, row, &READER);
        ee.set(28, GP);
        ee.call(FRAME, &[u64::from(SIM)], &[f32::from_bits(DT)], 400_000_000);
        let d = diff(&ee, row);
        assert!(
            d.is_empty(),
            "{name} row {r} (vsync {}): {} words differ\n  {}",
            row["vsync"],
            d.len(),
            d.iter().take(40).cloned().collect::<Vec<_>>().join("\n  ")
        );
        let w = wrecked(|o| ee.u32_at(BIKE + o));
        assert_eq!(w, row_wrecked(row), "{name} row {r}: the wreck state");
        out.push(w);
        poke(&mut ee, row, &SPOT);
    }
    Some(out)
}

#[test]
fn ride_replays_bit_exact() {
    let Some(w) = replay("ride-2") else { return };
    assert_eq!(w.len(), 459);
    assert!(w.iter().all(|w| !w), "the ride never wrecks");
}

#[test]
fn flip_wrecks_and_is_put_back_bit_exact() {
    let Some(w) = replay("flip-1") else { return };
    assert_eq!(w.len(), 465);
    let changes: Vec<usize> = (1..w.len()).filter(|&r| w[r] != w[r - 1]).collect();
    assert_eq!(changes, [125, 193], "wrecked on row 125, put back on row 193");
}

/// Whether a row's reader words are a tape tick's pad: cross the gas (held at 0 while wrecked),
/// square the brake, the left stick as it is, R1 the trick bit 4 (not while wrecked). The reader runs
/// before the frame, so "wrecked" is the row before's.
fn reads(before: &Row, row: &Row, tick: &Row) -> bool {
    let o = obj(row);
    let f = |at: u32| f32::from_bits(o[at as usize / 4]);
    let held = |k: &str| tick["buttons"].split(' ').any(|x| x == k);
    let up = !row_wrecked(before);
    let lever = |on: bool| if on { 1.0 } else { 0.0 };
    f(0x314) == lever(held("cross") && up)
        && f(0x310) == lever(held("square"))
        && f(0x12a8) == tick["lx"].parse::<f32>().unwrap()
        && f(0x12ac) == tick["ly"].parse::<f32>().unwrap()
        && (o[0x14ec / 4] & 4 != 0) == (held("r1") && up)
}

/// Tape tick t is read on row t + 1, and not on row t.
#[test]
fn ticks_land_one_row_later() {
    for (name, tape) in [("ride-2", "ride"), ("flip-1", "flip")] {
        let (Some(rows), Some(ticks)) = (rows(name), ticks(tape)) else { return };
        assert_eq!(ticks.len(), rows.len() + 1, "{name}: one tick past the last row");
        let n = rows.len() - 1;
        let bad: Vec<usize> = (0..n).filter(|&t| !reads(&rows[t], &rows[t + 1], &ticks[t])).collect();
        assert!(bad.is_empty(), "{name}: ticks {bad:?} are not on the next row");
        let own = (1..n).filter(|&t| reads(&rows[t - 1], &rows[t], &ticks[t])).count();
        assert!(own < n - 1, "{name}: the tape also fits its own row");
    }
}
