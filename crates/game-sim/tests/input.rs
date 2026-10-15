//! The input layer, proved three ways.
//!
//! 1. **The mapping**, against the button word measured one button at a time off the real pad (C5, and
//!    `docs/PCSX2.md`'s table): which bit is which control, which controls are read from a pressure byte
//!    instead, and the arithmetic that turns a pressure into a strength.
//! 2. **The timer arithmetic**, against the game's own instructions through the oracle. The span is not
//!    a function — it is thirty instructions inside the poll — so the test loads them into a bare
//!    interpreter, appends its own `jr $ra` where the poll's branch went, and compares
//!    [`game_sim::input::add_ms`] against them over values the game itself never reaches: the far side
//!    of 2³¹, where the unsigned conversion needs its bias.
//! 3. **The whole poll**, against a 220-frame recording of the real game: every control's six words and
//!    every event's six words, row for row, from pad reports taken out of the same recording. That is
//!    the test that covers the parts no amount of reading proves — which frame latches a press, whether
//!    the timers accumulate, and the order two presses in one frame come out in.
//!
//! The capture is game data, so it lives in the private directory and this test says so and passes when
//! it is not there. Recording another: `./kit record watch/input.toml --frames 220 --out
//! context/capture/i1-input.csv` with `watch/pads/two-at-once.pad` driving the pad.

use std::path::{Path, PathBuf};

use game_sim::hash;
use game_sim::input::{
    CONTROLS, Control, ControlState, EventQueue, Manager, PADS, PRESSURE_THRESHOLD, REPORT_BYTES,
    Report, add_ms,
};
use game_sim::replay::Capture;
use ps2_float::F;
use ps2_oracle::{Ee, addr, game_or_skip, skip};

/// The engine's own 30 fps cap, and the `dt` most frames of the recording ran at.
const DT: F = F(0.03333);

/// Every pad connected, which is what a port driven by a host pad has.
fn manager() -> Manager {
    Manager::connected()
}

/// One frame with these controls down on pad 0 and nothing on pad 1.
fn press(m: &mut Manager, dt: F, down: &[Control]) {
    m.tick_buttons(dt, &[game_sim::input::mask_of(down), 0]);
}

// ---------------------------------------------------------------------------------------------------
// The mapping
// ---------------------------------------------------------------------------------------------------

/// The button word as measured against the real pad, one button at a time. This table is the evidence;
/// [`CONTROL_MAP`] is the port's copy of it, and the two are compared here rather than being the same
/// line of source.
const MEASURED: [(Control, u16); CONTROLS] = [
    (Control::L2, 0x0001),
    (Control::R2, 0x0002),
    (Control::L1, 0x0004),
    (Control::R1, 0x0008),
    (Control::Triangle, 0x0010),
    (Control::Circle, 0x0020),
    (Control::Cross, 0x0040),
    (Control::Square, 0x0080),
    (Control::Select, 0x0100),
    (Control::L3, 0x0200),
    (Control::R3, 0x0400),
    (Control::Start, 0x0800),
    (Control::Up, 0x1000),
    (Control::Right, 0x2000),
    (Control::Down, 0x4000),
    (Control::Left, 0x8000),
];

#[test]
fn masks_are_the_word_measured_off_the_pad() {
    for (control, mask) in MEASURED {
        assert_eq!(control.mask(), mask, "{}", control.name());
    }
}

#[test]
fn every_control_has_its_own_bit_and_its_own_index() {
    let mut seen = 0u16;
    for (i, control) in Control::ALL.into_iter().enumerate() {
        assert_eq!(control.index(), i);
        assert_eq!(Control::from_index(i), Some(control));
        assert_eq!(control.mask().count_ones(), 1, "{}", control.name());
        assert_eq!(seen & control.mask(), 0, "{} shares a bit", control.name());
        seen |= control.mask();
    }
    assert_eq!(seen, 0xffff, "the sixteen controls cover the whole word");
    assert_eq!(Control::from_index(CONTROLS), None);
}

/// The four without a pressure byte are exactly the four a DualShock 2 reports no pressure for, and the
/// twelve that have one use twelve different bytes inside the report.
#[test]
fn only_the_four_pressureless_buttons_are_digital() {
    let digital: Vec<&str> = Control::ALL
        .into_iter()
        .filter(|c| !c.binding().is_analogue())
        .map(Control::name)
        .collect();
    assert_eq!(digital, ["select", "start", "r3", "l3"]);

    let mut indices: Vec<u8> = Control::ALL
        .into_iter()
        .filter(|c| c.binding().is_analogue())
        .map(|c| c.binding().pressure)
        .collect();
    indices.sort();
    assert_eq!(indices, (8..20).collect::<Vec<u8>>());
}

// ---------------------------------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------------------------------

/// The word is big-endian and active low, so a report with two buttons down has two zero bits.
#[test]
fn the_button_word_is_big_endian_and_active_low() {
    let r = Report::from_buttons(Control::L1.mask() | Control::Up.mask());
    assert_eq!(r.0[2], 0xef, "the high byte keeps up's bit low");
    assert_eq!(r.0[3], 0xfb, "the low byte keeps L1's bit low");
    assert_eq!(r.buttons(), 0x1004);
    assert_eq!(r.status(), 0);
    assert_eq!(r.mode(), 0x79);
}

#[test]
fn a_bitmask_round_trips_through_a_report() {
    for mask in [0x0000, 0xffff, 0x1234, 0x0040, 0x8001] {
        assert_eq!(Report::from_buttons(mask).buttons(), mask, "{mask:#06x}");
    }
}

#[test]
fn an_idle_report_has_nothing_down_and_a_missing_one_says_so() {
    assert_eq!(Report::idle().buttons(), 0);
    assert_eq!(Report::idle().status(), 0);
    assert_ne!(Report::missing().status(), 0);
    for axis in 0..4 {
        assert_eq!(Report::idle().axis(axis), 0x7f);
    }
}

// ---------------------------------------------------------------------------------------------------
// The poll: how a control is read
// ---------------------------------------------------------------------------------------------------

#[test]
fn a_digital_control_reads_its_bit_at_full_strength() {
    let mut m = manager();
    press(&mut m, DT, &[Control::Start]);
    assert!(m.pad(0).held(Control::Start));
    assert_eq!(m.pad(0).strength(Control::Start), F(1.0));
    assert!(!m.pad(0).held(Control::Select));
    assert_eq!(m.pad(0).strength(Control::Select), F::ZERO);
}

/// The twelve analogue controls are read from their pressure byte **only**. A report whose word says
/// cross is down but whose pressure byte is zero leaves cross released — which is exactly what a pad
/// still in digital mode sends, and why the platform layer has to get the pad into pressure mode.
#[test]
fn an_analogue_control_ignores_its_bit() {
    let mut m = manager();
    let mut r = Report::from_buttons(Control::Cross.mask());
    r.0[Control::Cross.binding().pressure as usize] = 0;
    m.tick(DT, &[r, Report::missing()]);
    assert_eq!(r.buttons(), Control::Cross.mask(), "the bit really is set");
    assert!(!m.pad(0).held(Control::Cross));
    assert!(m.events.is_empty());
}

/// Held strictly above the threshold, and the strength is `(pressure + 1) / 256`.
#[test]
fn pressure_becomes_strength_one_byte_at_a_time() {
    let cross = Control::Cross.binding().pressure as usize;
    for pressure in 0u16..=255 {
        let mut m = manager();
        let mut r = Report::idle();
        r.0[cross] = pressure as u8;
        m.tick(DT, &[r, Report::missing()]);

        let want_held = pressure as u8 > PRESSURE_THRESHOLD;
        assert_eq!(m.pad(0).held(Control::Cross), want_held, "{pressure:#04x}");
        let want = if want_held {
            F::from(pressure as i32 + 1) * 0.003_906_25
        } else {
            F::ZERO
        };
        assert_eq!(m.pad(0).strength(Control::Cross), want, "{pressure:#04x}");
    }
    // The two ends of the range, spelled out: 0x19 is not a press, and a full byte is exactly 1.0.
    assert_eq!(PRESSURE_THRESHOLD, 0x19);
    assert_eq!(F::from(0x100), F(1.0) * 256.0);
}

// ---------------------------------------------------------------------------------------------------
// The timers, and which frame latches what
// ---------------------------------------------------------------------------------------------------

#[test]
fn the_press_frame_latches_the_held_timer_at_zero() {
    let mut m = manager();
    press(&mut m, DT, &[Control::R1]);
    let c = *m.pad(0).control(Control::R1);
    assert_eq!((c.held, c.held_ms, c.prev_held_ms), (1, 0, 0));
    assert!(m.pad(0).pressed(Control::R1));
    assert!(!m.pad(0).released(Control::R1));
}

/// The timer is stored as an integer and read back each frame, so the 0.33 ms the cap leaves over is
/// thrown away every frame instead of accumulating. Thirty-six frames of the recording's long hold, to
/// the millisecond.
#[test]
fn the_held_timer_does_not_accumulate_its_fraction() {
    let mut m = manager();
    let mut seen = Vec::new();
    for _ in 0..36 {
        press(&mut m, DT, &[Control::R1]);
        seen.push(m.pad(0).control(Control::R1).held_ms);
    }
    let want: Vec<u32> = (0..36).map(|k| k * 33).collect();
    assert_eq!(seen, want, "+33 a frame, flat");
    assert_eq!(seen[35], 1155, "the recording's own last value");
    assert_ne!(seen[35], 1166, "which is what a float accumulator would read");
}

#[test]
fn the_release_frame_latches_the_release_timer_at_zero() {
    let mut m = manager();
    for _ in 0..3 {
        press(&mut m, DT, &[Control::R1]);
    }
    press(&mut m, DT, &[]);
    let c = *m.pad(0).control(Control::R1);
    assert_eq!((c.held, c.since_release_ms, c.prev_since_release_ms), (0, 0, 0));
    assert_eq!(c.held_ms, 66, "the held timer keeps its last value");
    assert!(m.pad(0).released(Control::R1));
    assert!(!m.pad(0).pressed(Control::R1));

    press(&mut m, DT, &[]);
    let c = *m.pad(0).control(Control::R1);
    assert_eq!((c.since_release_ms, c.prev_since_release_ms), (33, 0));
    assert_eq!(c.held_ms, 66, "and keeps it for as long as the control is up");
    assert!(!m.pad(0).released(Control::R1));
}

#[test]
fn the_previous_values_are_one_frame_behind() {
    let mut m = manager();
    let mut pairs = Vec::new();
    for _ in 0..4 {
        press(&mut m, DT, &[Control::R1]);
        let c = *m.pad(0).control(Control::R1);
        pairs.push((c.prev_held_ms, c.held_ms));
    }
    assert_eq!(pairs, [(0, 0), (0, 33), (33, 66), (66, 99)]);
}

/// With the clock stopped — a paused frame, a single step — the timer cannot leave 0, so "held with the
/// timer at 0" stays true while the event queue holds the press for one frame only. The original has
/// the same two answers; the module's docs say which is which, and this pins it.
#[test]
fn a_stopped_clock_makes_pressed_and_the_queue_disagree() {
    let mut m = manager();
    press(&mut m, F::ZERO, &[Control::R1]);
    assert_eq!(m.events.len(), 1);
    assert!(m.pad(0).pressed(Control::R1));

    press(&mut m, F::ZERO, &[Control::R1]);
    assert!(m.events.is_empty(), "the edge is gone from the queue");
    assert!(m.pad(0).pressed(Control::R1), "but the timer still reads 0");
}

// ---------------------------------------------------------------------------------------------------
// The event queue
// ---------------------------------------------------------------------------------------------------

#[test]
fn a_press_raises_one_event_for_exactly_one_frame() {
    let mut m = manager();
    press(&mut m, DT, &[Control::Cross]);
    assert_eq!(m.events.len(), 1);
    let e = m.events.events()[0];
    assert_eq!(e.control(), Some(Control::Cross));
    assert_eq!((e.kind, e.device, e.index, e.pressed), (2, -1, 0, 1));
    assert_eq!(e.stamp, 0, "this game's manager has no time source");
    assert!(m.events.press(Control::Cross).is_some());
    assert!(m.events.release(Control::Cross).is_none());

    press(&mut m, DT, &[Control::Cross]);
    assert!(m.events.is_empty(), "held is not an edge");
}

#[test]
fn a_release_raises_an_event_that_says_so() {
    let mut m = manager();
    press(&mut m, DT, &[Control::Cross]);
    press(&mut m, DT, &[]);
    assert_eq!(m.events.len(), 1);
    let e = m.events.events()[0];
    assert_eq!(e.control(), Some(Control::Cross));
    assert_eq!(e.pressed, 0);
    assert!(!e.is_press());
    assert!(m.events.release(Control::Cross).is_some());
}

/// Two controls in one frame come out in **control order**, not in the order the pad pressed them: the
/// poll reads all sixteen controls and only then walks them again looking for changes. The recording
/// shows the same thing — a frame where the pad script had pressed up, then L1, then R2 queues L1 (4)
/// before R2 (7) before up (12).
#[test]
fn events_come_out_in_control_order_not_press_order() {
    let mut m = manager();
    press(&mut m, DT, &[Control::Up, Control::L1, Control::R2]);
    let got: Vec<usize> = m.events.events().iter().map(|e| e.control as usize).collect();
    assert_eq!(got, [Control::L1.index(), Control::R2.index(), Control::Up.index()]);
    assert!(m.events.events().iter().all(|e| e.is_press()));
}

/// A press of one control and a release of another in the same frame: one queue, still in control
/// order, each event carrying its own direction.
#[test]
fn a_press_and_a_release_share_one_frame() {
    let mut m = manager();
    press(&mut m, DT, &[Control::Square]);
    press(&mut m, DT, &[Control::Triangle]);
    let got: Vec<(usize, u32)> = m
        .events
        .events()
        .iter()
        .map(|e| (e.control as usize, e.pressed))
        .collect();
    assert_eq!(got, [(Control::Triangle.index(), 1), (Control::Square.index(), 0)]);
}

#[test]
fn both_pads_queue_onto_the_same_frame_pad_zero_first() {
    let mut m = manager();
    m.tick_buttons(DT, &[Control::Left.mask(), Control::Start.mask()]);
    let got: Vec<(u32, usize)> = m
        .events
        .events()
        .iter()
        .map(|e| (e.index, e.control as usize))
        .collect();
    assert_eq!(got, [(0, Control::Left.index()), (1, Control::Start.index())]);
}

/// The queue holds a frame in which *everything* changes on both pads, which is the most the original
/// can raise: every control of every pad can change at most once a frame.
///
/// The same frame with the word all down and no pressures at all — a pad still in digital mode — raises
/// four events a pad instead of sixteen, because the twelve analogue controls are read from their
/// pressure bytes and nothing else.
#[test]
fn the_queue_holds_every_control_of_both_pads() {
    let everything = Report(core::array::from_fn(|i| match i {
        0 => 0,           // a good read
        1 => 0x79,        // pressures mode
        2 | 3 => 0,       // the whole word, active low
        _ => 0xff,        // every pressure at full
    }));
    let mut m = manager();
    m.tick(DT, &[everything, everything]);
    assert_eq!(m.events.len(), PADS * CONTROLS, "every control of both pads");
    assert_eq!(m.pad(0).held_mask(), 0xffff);
    assert_eq!(m.pad(0).pressed_mask(), 0xffff);
    assert_eq!(m.pad(1).held_mask(), 0xffff);

    let mut digital = Report::idle();
    digital.0[2] = 0;
    digital.0[3] = 0;
    let mut m = manager();
    m.tick(DT, &[digital, digital]);
    assert_eq!(m.events.len(), PADS * 4, "only the four controls that have no pressure byte");
    assert_eq!(
        m.pad(0).held_mask(),
        Control::Select.mask() | Control::Start.mask() | Control::R3.mask() | Control::L3.mask()
    );
}

#[test]
fn the_masks_are_the_three_questions_a_caller_asks() {
    let mut m = manager();
    press(&mut m, DT, &[Control::Up, Control::Start]);
    assert_eq!(m.pad(0).held_mask(), Control::Up.mask() | Control::Start.mask());
    assert_eq!(m.pad(0).pressed_mask(), Control::Up.mask() | Control::Start.mask());

    press(&mut m, DT, &[Control::Up]);
    assert_eq!(m.pad(0).held_mask(), Control::Up.mask());
    assert_eq!(m.pad(0).pressed_mask(), 0);
    assert_eq!(m.pad(0).released_mask(), Control::Start.mask());
}

// ---------------------------------------------------------------------------------------------------
// A pad that goes away
// ---------------------------------------------------------------------------------------------------

/// Unplugged mid-press: everything reads released, and **no event is raised** — the front end never
/// hears the release. The timers are not touched either, so the release timer goes on counting from
/// whatever it held before the press.
#[test]
fn a_pad_that_vanishes_releases_everything_without_an_event() {
    let mut m = manager();
    press(&mut m, DT, &[]); // one quiet frame, so the release timer has a value
    press(&mut m, DT, &[Control::Cross]);
    let before = *m.pad(0).control(Control::Cross);

    m.pad_mut(0).connected = false;
    m.tick(DT, &[Report::idle(), Report::missing()]);
    let after = *m.pad(0).control(Control::Cross);
    assert!(m.events.is_empty(), "the release event is lost with the pad");
    assert_eq!(after.held, 0);
    assert_eq!(after.strength, F::ZERO);
    assert_eq!(
        (after.since_release_ms, after.held_ms),
        (before.since_release_ms, before.held_ms),
        "the timers are left exactly as they were"
    );
}

/// A failed read — a non-zero status byte — takes the same path for the controls, and also raises
/// nothing.
#[test]
fn a_bad_status_byte_releases_everything_and_raises_nothing() {
    let mut m = manager();
    press(&mut m, DT, &[Control::Cross]);
    let before = *m.pad(0).control(Control::Cross);

    let mut bad = Report::from_buttons(Control::Cross.mask());
    bad.0[0] = 1;
    m.tick(DT, &[bad, Report::missing()]);
    assert!(m.events.is_empty());
    assert!(!m.pad(0).held(Control::Cross));
    assert_eq!(m.pad(0).control(Control::Cross).held_ms, before.held_ms);
}

// ---------------------------------------------------------------------------------------------------
// The state hash
// ---------------------------------------------------------------------------------------------------

/// Only the events a frame actually raised are hashed. The original clears the queue by zeroing its
/// count and leaves the stale events where they lie, so hashing the whole array would make two frames
/// that are identical differ on what happened before them.
#[test]
fn the_hash_ignores_the_slots_past_the_count() {
    let mut used = EventQueue::new();
    let mut m = manager();
    press(&mut m, DT, &[Control::Cross]);
    used.push(m.events.events()[0]);
    used.clear();
    assert_eq!(hash::of(&used), hash::of(&EventQueue::new()));
}

#[test]
fn the_hash_moves_when_a_control_does() {
    let mut m = manager();
    press(&mut m, DT, &[]);
    let quiet = m.hash();
    press(&mut m, DT, &[Control::Cross]);
    assert_ne!(m.hash(), quiet);

    let mut again = manager();
    press(&mut again, DT, &[]);
    press(&mut again, DT, &[Control::Cross]);
    assert_eq!(again.hash(), m.hash(), "the same frames, the same state");
}

// ---------------------------------------------------------------------------------------------------
// The timer arithmetic, against the game's own instructions
// ---------------------------------------------------------------------------------------------------

/// Where the span's thirty instructions are copied to, where its control record goes, and the budget.
/// Any addresses in a bare interpreter; the game's own are names, resolved through `research/oracle.txt`.
const SPAN_INSTRUCTIONS: usize = 30;
const CODE_AT: u32 = 0x1000;
const RECORD_AT: u32 = 0x4000;
/// The release timer's offset inside the control record the span addresses off `$s2`.
const TIMER_OFFSET: u32 = 0x330;

#[test]
fn the_timer_update_is_the_games_own_arithmetic() {
    let Some(game) = game_or_skip() else { return };
    let at = match addr("pad_timer_add_ms") {
        Ok(a) => a,
        Err(why) => return skip(&why),
    };

    // Lift the span out of the loaded program and give it a return of its own, where the poll's branch
    // out of the loop body used to be.
    let off = (at & 0x0fff_ffff) as usize;
    let span = &game.ram[off..off + SPAN_INSTRUCTIONS * 4];
    let mut ee = Ee::bare(0x10_000);
    ee.load(CODE_AT, span);
    ee.load(CODE_AT + (SPAN_INSTRUCTIONS * 4) as u32, &[
        0x08, 0x00, 0xe0, 0x03, // jr $ra
        0x00, 0x00, 0x00, 0x00, // nop
    ]);

    // Timers the game reaches in a session, and timers it would need days to reach: either side of
    // 2²⁴ (where a single can no longer count by ones) and either side of 2³¹ (where the conversion
    // back needs its bias).
    let timers: [u32; 14] = [
        0, 1, 32, 33, 1155, 73_800, 0x00ff_ffff, 0x0100_0000, 0x0100_0001, 0x7fff_ff80,
        0x8000_0000, 0x8000_0021, 0xc000_0000, 0xffff_ff00,
    ];
    // Milliseconds a frame: the 30 fps cap, the two the recording saw, a stopped clock, a long stall,
    // and a step big enough to walk a timer over the bias.
    let steps: [F; 7] = [
        DT * 1000.0,
        F(0.032_866_653) * 1000.0,
        F(0.032_833_323) * 1000.0,
        F::ZERO,
        F(1.0),
        F(1_000_000.0),
        F(2_000_000_000.0),
    ];

    for timer in timers {
        for dt_ms in steps {
            ee.set(18, u64::from(RECORD_AT)); // $s2, the control record
            ee.fpr[20] = dt_ms.to_bits(); // $f20, the frame's milliseconds
            ee.set_u32(RECORD_AT + TIMER_OFFSET, timer);
            ee.call(CODE_AT, &[], &[], 1_000);

            let want = ee.u32_at(RECORD_AT + TIMER_OFFSET);
            assert_eq!(
                add_ms(timer, dt_ms),
                want,
                "{timer} + {dt_ms} ms: the original stores {want}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------
// The whole poll, against a recording of the real game
// ---------------------------------------------------------------------------------------------------

/// How many of the queue's events the watch spec records. Only the first two: a frame with more is
/// compared on its count and on the two it recorded.
const WATCHED_EVENTS: usize = 2;

/// Every capture in the private directory that has the input layer's columns replays row for row:
/// sixteen control records and the events, against the pad bytes from the same row.
///
/// The port is seeded from the first row because the original's four timers are never initialised —
/// they are heap garbage until the control's first transition, and the first row shows them mid-count
/// at values no port can reproduce or should. Every row after that is the port's own arithmetic.
#[test]
fn a_recording_of_the_real_game_replays_row_for_row() {
    let dir = private_dir("capture");
    let mut proved = 0;
    for path in files_in(&dir, "csv") {
        let text = std::fs::read_to_string(&path).expect("read the capture");
        let capture = match Capture::parse(&text) {
            Ok(c) => c,
            Err(e) => panic!("{}: {e}", path.display()),
        };
        if !capture.columns.iter().any(|c| c == "ctl0.held") {
            continue; // some other task's capture, watching something else
        }
        replay(&path, &capture);
        proved += 1;
    }
    if proved == 0 {
        skip(&format!(
            "no input capture in {} — record one with `./kit record watch/input.toml --frames 220 \
             --out context/capture/i1-input.csv` while `watch/pads/two-at-once.pad` drives the pad",
            dir.display()
        ));
    }
}

fn replay(path: &Path, capture: &Capture) {
    let rows = Rows::new(capture);
    let mut m = Manager::new();
    m.pad_mut(0).connected = true; // pad 1 was never plugged in: its control map is all zeros

    let seed = |m: &mut Manager, row: usize| {
        for i in 0..CONTROLS {
            m.pads[0].controls[i] = rows.control(row, i);
        }
    };
    seed(&mut m, 0);

    // What the capture is worth: a replay that never saw a press, or never saw a hold long enough for
    // the timers to matter, would pass without proving anything.
    let (mut presses, mut releases, mut multi, mut compared) = (0, 0, 0, 0);
    let mut longest = 0u32;

    for row in 1..capture.rows.len() {
        if capture.frames[row] != capture.frames[row - 1] + 1 {
            seed(&mut m, row); // a gap in the recording: start again from what the game had
            continue;
        }
        let report = rows.report(row);
        m.tick(rows.dt(row), &[report, Report::missing()]);
        compared += 1;

        let frame = capture.frames[row];
        let at = |what: &str| format!("{}: row {row}, frame {frame}, {what}", path.display());

        for i in 0..CONTROLS {
            let (got, want) = (m.pads[0].controls[i], rows.control(row, i));
            let name = Control::ALL[i].name();
            assert_eq!(i64::from(got.held), i64::from(want.held), "{}", at(&format!("{name}.held")));
            assert_eq!(
                got.strength.to_bits(),
                want.strength.to_bits(),
                "{}",
                at(&format!("{name}.strength {} vs {}", got.strength, want.strength))
            );
            for (field, g, w) in [
                ("since_release_ms", got.since_release_ms, want.since_release_ms),
                ("held_ms", got.held_ms, want.held_ms),
                ("prev_since_release_ms", got.prev_since_release_ms, want.prev_since_release_ms),
                ("prev_held_ms", got.prev_held_ms, want.prev_held_ms),
            ] {
                assert_eq!(g, w, "{}", at(&format!("{name}.{field} {g} vs {w}")));
            }
            if got.is_held() {
                longest = longest.max(got.held_ms); // a *held* control's timer: the garbage ones count up too
            }
        }

        let want_events = rows.uint(row, "events") as usize;
        assert_eq!(m.events.len(), want_events, "{}", at("event count"));
        if want_events > 1 {
            multi += 1;
        }
        for k in 0..want_events.min(WATCHED_EVENTS) {
            let e = m.events.events()[k];
            let p = |f: &str| format!("ev{k}.{f}");
            assert_eq!(u64::from(e.control), rows.uint(row, &p("control")), "{}", at(&p("control")));
            assert_eq!(u64::from(e.kind), rows.uint(row, &p("kind")), "{}", at(&p("kind")));
            assert_eq!(i64::from(e.device), rows.int(row, &p("device")), "{}", at(&p("device")));
            assert_eq!(u64::from(e.index), rows.uint(row, &p("index")), "{}", at(&p("index")));
            assert_eq!(u64::from(e.pressed), rows.uint(row, &p("pressed")), "{}", at(&p("pressed")));
            assert_eq!(u64::from(e.stamp), rows.uint(row, &p("stamp")), "{}", at(&p("stamp")));
            if e.is_press() { presses += 1 } else { releases += 1 }
        }
    }

    assert!(compared >= 100, "{}: only {compared} rows replayed", path.display());
    assert!(presses >= 4, "{}: only {presses} presses in the recording", path.display());
    assert!(releases >= 4, "{}: only {releases} releases", path.display());
    assert!(multi >= 2, "{}: no frame with two events at once", path.display());
    assert!(longest >= 1000, "{}: nothing held for a second", path.display());
    println!(
        "{}: {compared} frames, {presses} presses, {releases} releases, {multi} multi-event frames, \
         {longest} ms the longest hold",
        path.display()
    );
}

/// The capture's columns, by name.
struct Rows<'a> {
    capture: &'a Capture,
}

impl<'a> Rows<'a> {
    fn new(capture: &'a Capture) -> Rows<'a> {
        Rows { capture }
    }

    fn cell(&self, row: usize, column: &str) -> &str {
        let at = self
            .capture
            .columns
            .iter()
            .position(|c| c == column)
            .unwrap_or_else(|| panic!("the capture has no column '{column}'"));
        &self.capture.rows[row][at]
    }

    fn uint(&self, row: usize, column: &str) -> u64 {
        let c = self.cell(row, column);
        c.strip_prefix("0x")
            .map(|h| u64::from_str_radix(h, 16))
            .unwrap_or_else(|| c.parse())
            .unwrap_or_else(|_| panic!("'{c}' in {column} is not a number"))
    }

    fn int(&self, row: usize, column: &str) -> i64 {
        let c = self.cell(row, column);
        c.parse().unwrap_or_else(|_| panic!("'{c}' in {column} is not a number"))
    }

    fn float(&self, row: usize, column: &str) -> F {
        let c = self.cell(row, column);
        F(c.parse().unwrap_or_else(|_| panic!("'{c}' in {column} is not a float")))
    }

    fn dt(&self, row: usize) -> F {
        self.float(row, "dt")
    }

    /// The half of libpad's double buffer the engine actually read: the one whose stamp is *behind*.
    /// The IOP refreshes the other half between the tick and the sample, so a capture compared against
    /// the newer half disagrees on exactly the frames where a button changed.
    fn report(&self, row: usize) -> Report {
        let a = self.uint(row, "stamp_a");
        let b = self.uint(row, "stamp_b");
        let hex = self.cell(row, if a <= b { "pad_a" } else { "pad_b" });
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let mut bytes = [0u8; REPORT_BYTES];
        for (i, slot) in bytes.iter_mut().enumerate() {
            let Some(pair) = hex.get(i * 2..i * 2 + 2) else { break };
            *slot = u8::from_str_radix(pair, 16).expect("the pad column is hex");
        }
        Report(bytes)
    }

    fn control(&self, row: usize, i: usize) -> ControlState {
        let p = |f: &str| format!("ctl{i}.{f}");
        ControlState {
            held: self.uint(row, &p("held")) as i32,
            strength: self.float(row, &p("value")),
            since_release_ms: self.uint(row, &p("rel_ms")) as u32,
            held_ms: self.uint(row, &p("held_ms")) as u32,
            prev_since_release_ms: self.uint(row, &p("prev_rel_ms")) as u32,
            prev_held_ms: self.uint(row, &p("prev_held_ms")) as u32,
        }
    }
}

/// Where private data lives: `$GAME_SIM_FIXTURES`, or `context/` beside the repo. Game data never goes
/// in the repository, so a fresh checkout runs this test green and says it skipped.
fn private_dir(sub: &str) -> PathBuf {
    match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => PathBuf::from(dir).join(sub),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("context")
            .join(sub),
    }
}

fn files_in(dir: &PathBuf, extension: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == extension))
        .collect();
    out.sort();
    out
}
