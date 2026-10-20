//! I7: the ride's stick dead zone, proved on the original.
//!
//! Both gameplay save states register exactly four stick consumers — the player record's left x, left
//! y, right x and right y — and all four carry ±0.3 ([`AxisRange::PLAYER`], I6), where the main
//! menu's carry I2's ±0.2 ([`AxisRange::STICK`]). The plane was proved on its own capture; this is the
//! bike's: the human's recording2 played back on the original with the player record's two left-stick
//! floats (+0x324/+0x328) recorded beside the pad, and the port's [`Controls::from_pad`] checked
//! against them tick by tick. The two ranges only disagree on report bytes 90..=102 and 153..=165,
//! where ±0.2 hands out up to about ±0.12 and ±0.3 hands out exactly zero; the test counts those rows and
//! checks that the other range would have missed them.
//!
//! The capture is I7's (research/journal/2026-10-20-i7-ride-stick-dead-zone):
//! `context/capture/i7/recording2-sticks.csv`, frame counters 2374..4267.

use game_sim::input::{Axis, AxisRange, Control, Manager, Pad, REPORT_BYTES, Report};
use game_sim::replay::Capture;
use game_sim::ride::Controls;
use ps2_float::F;

/// The capture, or `None` (and a line saying how to make it) when this checkout does not have it.
fn capture() -> Option<Capture> {
    let path = match std::env::var_os("GAME_SIM_FIXTURES") {
        Some(dir) => std::path::PathBuf::from(dir).join("capture"),
        None => std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../context/capture"),
    }
    .join("i7/recording2-sticks.csv");
    let Ok(text) = std::fs::read_to_string(&path) else {
        println!(
            "skipped: no {} — play recording2.p2m2 back with I7's playback/play_capture.sh (journal \
             2026-10-20-i7-ride-stick-dead-zone), frame counters 2374..4267",
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

/// Where the two ranges disagree: a byte ±0.2 lets through and ±0.3 does not.
fn in_the_band(byte: F) -> bool {
    let b = byte.0 as u8;
    (90..=102).contains(&b) || (153..=165).contains(&b)
}

fn sticks_through(pad: &Pad, range: &AxisRange) -> [F; 2] {
    [pad.axis_through(Axis::LeftX, range), pad.axis_through(Axis::LeftY, range)]
}

/// One cell of a column, by name.
fn cell<'a>(cap: &'a Capture, row: usize, col: &str) -> &'a str {
    let at = cap.columns.iter().position(|c| c == col).unwrap_or_else(|| panic!("no column {col}"));
    &cap.rows[row][at]
}

/// The report a tick read, from a row's hex cell.
fn report(hex: &str) -> Report {
    let mut bytes = [0u8; REPORT_BYTES];
    for (i, b) in bytes.iter_mut().enumerate().take(hex.len() / 2) {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("a hex report");
    }
    Report::from_bytes(bytes)
}

/// The report row `row`'s tick polled, and its stamp: the *newer* buffer of the row before, the one
/// stamped two vsyncs before this row's frame.
///
/// This is not [`game_sim::replay::PadTape::from_capture`]'s rule (this row's older buffer, one vsync back), and the
/// difference is the recorder's phase, not the game's: on recording2's playback — this capture and
/// C6's `recording2-freeride.csv` alike — the game's own control state (`ctl*.held`) is the f−2
/// buffer's on every row where the buffers disagree, where on C7's biplane capture it is f−1's. The
/// test checks the control state against the port on every tick, so a wrong alignment fails there
/// before it can say anything about the sticks.
fn polled(cap: &Capture, row: usize) -> (Report, u64) {
    let [a, b] = ["a", "b"].map(|h| {
        let stamp: u64 = cell(cap, row - 1, &format!("stamp_{h}")).parse().expect("a stamp");
        (stamp, cell(cap, row - 1, &format!("pad_{h}")))
    });
    let (stamp, hex) = if a.0 >= b.0 { a } else { b };
    (report(hex), stamp)
}

/// I7: the recorded pad, through the port's input layer and the ride's hands, gives the two sticks
/// the original's player record held — tick for tick, to the bit — and those are ±0.3's, not ±0.2's.
///
/// Every tick checks the sixteen held flags of the game's own control state first (the alignment),
/// then the player record's `+0x324`/`+0x328` against [`Controls::from_pad`]'s stick. Unlike C7's
/// biplane capture, no row here holds the previous tick's sticks, so there is no exception: every
/// tick is the port's to the bit.
#[test]
fn the_bike_reads_the_players_dead_zone() {
    let Some(cap) = capture() else { return };
    assert_eq!(cap.rows.len(), 1894, "the capture is I7's");

    let mut input = Manager::connected();
    let (mut band, mut band_menu_right) = (0, 0);
    for row in 2..cap.rows.len() {
        let frame: u64 = cell(&cap, row, "frame").parse().expect("a frame");
        let (pad_report, stamp) = polled(&cap, row);
        assert_eq!(stamp + 2, frame, "row {row}: the row before carries the buffer two vsyncs back");
        input.tick(F::from_bits(bits(&cap, row, "dt")), &[pad_report, Report::missing()]);
        let pad = input.pad(0);
        for c in Control::ALL {
            let held = cell(&cap, row, &format!("ctl{}.held", c.index())) != "0";
            assert_eq!(held, pad.held(c), "row {row}: {c:?}'s held flag is not the original's");
        }
        let hands = Controls::from_pad(pad).stick;
        let ride = [hands.0, hands.1];
        assert_eq!(ride, sticks_through(pad, &AxisRange::PLAYER), "row {row}: the ride's hands are the player's range");
        let got = [bits(&cap, row, "stick_x"), bits(&cap, row, "stick_y")];
        assert_eq!(got, ride.map(F::to_bits), "row {row}: the player's sticks are not the port's {ride:?}");
        let menu = sticks_through(pad, &AxisRange::STICK);
        for (i, axis) in [Axis::LeftX, Axis::LeftY].into_iter().enumerate() {
            if in_the_band(pad.raw_axis(axis)) {
                band += 1;
                band_menu_right += (menu[i].to_bits() == got[i]) as usize;
            }
        }
    }
    let ticks = cap.rows.len() - 2;
    assert!(band > 100, "the human crosses the band on {band} axis values, too few to decide on");
    assert_eq!(band_menu_right, 0, "the menu's ±0.2 is right on {band_menu_right} of {band} band values");
    println!(
        "the bike reads the pad through ±0.3: {ticks} ticks bit-exact, 16 held flags and 2 sticks each; \
         {band} band values, where ±0.2 is wrong on every one"
    );
}
