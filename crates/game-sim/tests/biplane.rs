//! The biplane, against the human's biplane recording played back on the original (C7).
//!
//! The capture is `context/capture/acceptance/biplane.csv` (`watch/biplane.toml`, journal
//! 2026-10-20-c7-c7-biplane-capture): 1,718 ticks of Free06 from frame counter 10971, with both
//! halves of the pad library's buffer, the player record's sticks and control table, and what the
//! plane's input step wrote from them. It is game data, so every test here skips without it.

use game_sim::input::{Axis, AxisRange, Control, Manager, PlaneCommand, PlaneInput, Report};
use game_sim::replay::{Capture, PadTape};
use ps2_float::F;
use ps2_oracle::{Ee, Kit, addr, skip};

/// The capture, or `None` (and a line saying how to make it) when this checkout does not have it.
fn capture() -> Option<Capture> {
    let path = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("acceptance/biplane.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!(
            "skipped: no {} — play biplane.p2m2 back with watch/biplane.toml (journal \
             2026-10-20-c7-c7-biplane-capture) and record frame counters 10971..12688",
            path.display()
        );
        return None;
    };
    Some(Capture::parse(&text).expect("the recorder's format"))
}

/// One f32 cell, as its bits.
fn bits(cap: &Capture, row: usize, col: &str) -> u32 {
    let at = cap.columns.iter().position(|c| c == col).unwrap_or_else(|| panic!("no column {col}"));
    let cell = &cap.rows[row][at];
    cell.parse::<f32>().unwrap_or_else(|_| panic!("row {row} {col}: '{cell}'")).to_bits()
}

/// The rows whose stick columns the recorder read before that tick's input step had written them
/// (C7's journal: row 150 is its torn row, and 158–179 are the rows where two playbacks disagree on
/// the sticks). On each, every stick-derived column holds the *previous* tick's value — which this
/// test checks rather than skipping the row.
const STICKS_READ_EARLY: [usize; 6] = [150, 158, 162, 163, 177, 179];

/// I6: the recorded pad, through the port's input layer and the plane's mapping, gives the eight
/// controls the original read — tick for tick, to the bit.
///
/// Every tick of the capture is checked three ways, each against the original's own RAM:
///
/// 1. **The control table**: the sixteen values at the player record's `+0x334` are the port's
///    control strengths ([`game_sim::input::Pad::strength`]) — so a throttle command's value is its
///    pressure byte's `(p + 1) / 256`, and a digital control is 1.0. One exception, measured and
///    not explained: R1's entry reads 0 after every tick R1 is held (154 of 154), though the plane
///    read it as held (its rudder is +1 on each of those ticks); something after the input step
///    clears it.
/// 2. **The sticks**: the player record's `+0x324`/`+0x328` are the left stick through
///    [`AxisRange::PLAYER`]'s ±0.3 dead zone.
/// 3. **The five commands** the plane's input step wrote (`+0x338..+0x348`) are
///    [`PlaneInput::from_pad`]'s.
///
/// Except on [`STICKS_READ_EARLY`]'s six rows, where the sticks and the two stick-derived commands
/// are the previous tick's, every value is the port's to the bit.
#[test]
fn the_biplane_reads_the_pad() {
    let Some(cap) = capture() else { return };
    let tape = PadTape::from_capture(&cap).expect("a dense capture with both pad halves");
    assert_eq!((cap.rows.len(), tape.frames.len()), (1718, 1717), "the capture is C7's");

    let mut input = Manager::connected();
    let mut bad: Vec<String> = Vec::new();
    let mut early = Vec::new();
    let mut r1_cleared = 0;
    let mut last: Option<[(&str, F); 4]> = None;
    for (k, frame) in tape.frames.iter().enumerate() {
        let row = k + 1;
        input.tick(F::from_bits(frame.dt), &[frame.reports[0], Report::missing()]);
        let pad = input.pad(0);
        for c in Control::ALL {
            let col = format!("pctl{}.value", c.index());
            let want = bits(&cap, row, &col);
            if c == Control::R1 && pad.held(c) {
                assert_eq!(want, 0, "row {row}: R1's entry, held");
                r1_cleared += 1;
            } else if want != pad.strength(c).to_bits() {
                bad.push(format!("row {row} {col}: {} vs {}", f32::from_bits(want), pad.strength(c).0));
            }
        }
        let p = PlaneInput::from_pad(pad);
        let stick = (pad.axis_through(Axis::LeftX, &AxisRange::PLAYER), pad.axis_through(Axis::LeftY, &AxisRange::PLAYER));
        let sticky = [("stick_x", stick.0), ("stick_y", stick.1), ("elev_cmd", p.elevator), ("ail_cmd", p.aileron)];
        let this_tick = sticky.iter().all(|(col, v)| bits(&cap, row, col) == v.to_bits());
        if !this_tick {
            let prev = last.expect("not the first tick");
            let as_before = prev.iter().all(|(col, v)| bits(&cap, row, col) == v.to_bits());
            assert!(as_before, "row {row}: the sticks are neither this tick's {sticky:?} nor the last's {prev:?}");
            early.push(row);
        }
        last = Some(sticky);
        for (col, v) in [("thr_up", p.throttle_up), ("thr_down", p.throttle_down), ("rud_cmd", p.rudder)] {
            if bits(&cap, row, col) != v.to_bits() {
                bad.push(format!("row {row} {col}: {} vs {}", f32::from_bits(bits(&cap, row, col)), v.0));
            }
        }
    }
    assert!(bad.is_empty(), "{} values are not the original's:\n{}", bad.len(), bad.join("\n"));
    assert_eq!(early, STICKS_READ_EARLY, "the rows whose sticks are the previous tick's");
    assert_eq!(r1_cleared, 154, "R1's held ticks");
    println!(
        "the biplane reads the pad: {} ticks, 16 table values, 2 sticks and 5 commands each, bit-exact \
         ({} rows with last tick's sticks, {r1_cleared} with R1's entry cleared)",
        tape.frames.len(),
        early.len()
    );
}

/// The biplane state's EE RAM, from this checkout's context or the one `kit.toml` points at.
fn plane_state() -> Option<Ee> {
    const STATE: &str = "biplane.p2m2_SaveState.eeMemory.bin";
    let mut tried = vec![std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/states").join(STATE)];
    if let Ok(kit) = Kit::load() {
        tried.push(kit.ctx("states").join(STATE));
    }
    for at in &tried {
        if let Ok(ee) = Ee::from_ram(at) {
            return Some(ee);
        }
    }
    skip(&format!(
        "no {STATE} — `./kit state ram context/recordings/biplane.p2m2_SaveState.p2s` writes it"
    ));
    None
}

/// I6: the plane's input step against the original's own, over the biplane state's RAM — the
/// shipped binding table in place — on 12,000 generated sticks and control tables, for both pads.
///
/// The capture proves the mapping on what the human pressed; this proves it on what they did not:
/// the d-pad's override of the stick (never pressed in the recording), both of a pair held at once,
/// a stick past ±1, an exact and a negative zero, and every binding the table has for pad 1.
#[test]
fn the_biplane_input_step_is_the_originals() {
    let Some(mut ee) = plane_state() else { return };
    let at = match addr("biplane_input") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };
    // Scratch for the plane's five fields and the player's table, below the oracle's stack.
    const PLANE: u32 = 0x01ff_0000;
    const TABLE: u32 = 0x01ff_1000;

    let mut seed: u64 = 0x16_b1_91_a9e;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as u32
    };
    let mut checked = 0;
    for n in 0..12_000u32 {
        let pad = n % 2;
        // A stick value: one a report byte gives through the ride's range, or something no pad can
        // give, or the zeros.
        let mut stick = || -> F {
            match next() % 8 {
                0 => F::ZERO,
                1 => F(-0.0),
                2 => F::from_bits(next() & 0x807f_ffff | 0x3f00_0000), // 0.5..1, either sign
                3 => F::from_bits(next() & 0x80ff_ffff | 0x3f00_0000), // 0.5..2, either sign
                _ => AxisRange::PLAYER.apply(F::from((next() & 0xff) as i32), F(255.0)),
            }
        };
        let (x, y) = (stick(), stick());
        let mut table = [F::ZERO; 16];
        for v in &mut table {
            *v = match next() % 4 {
                0 | 1 => F::ZERO,
                2 => F::ONE,
                _ => F::from((next() % 256) as i32 + 1) * F(0.003_906_25),
            };
        }
        for i in 0..16u32 {
            ee.set_u32(TABLE + i * 0x18 + 4, table[i as usize].to_bits());
        }
        for off in [0x338, 0x33c, 0x340, 0x344, 0x348] {
            ee.set_u32(PLANE + off, 0xdead_beef);
        }
        ee.call(at, &[u64::from(PLANE), u64::from(TABLE), u64::from(pad)], &[x.0, y.0], 10_000);

        let port = PlaneInput::from_sticks((x, y), |c: PlaneCommand| table[c.control().index()]);
        let theirs = [0x338, 0x33c, 0x340, 0x344, 0x348].map(|off| ee.u32_at(PLANE + off));
        let ours = [port.throttle_up, port.throttle_down, port.elevator, port.aileron, port.rudder].map(F::to_bits);
        assert_eq!(ours, theirs, "pad {pad}, stick ({x:?}, {y:?}), table {table:?}");
        checked += 1;
    }
    println!("the biplane input step: {checked} calls, both pads, bit-exact");
}
