//! P99 — session markers kept between sessions, per track and player profile (`saved_markers.rs`),
//! ridden the way `tests/markers.rs` rides them: the pad's poll with the d-pad out of the ride, the
//! step, then `markers::frame` (`Seat::pass`). The stand-in bike on a flat floor, at the 30 Hz clamp.
//! The files go in a folder of the test's own under the system's temp dir, never beside the disc.

#[path = "../src/saved_markers.rs"]
#[allow(dead_code)]
mod saved_markers;

use std::path::PathBuf;

use game_sim::input::{Control, PADS, Report};
use game_sim::markers::{self, Did, Markers, Pick, SLOTS, Spot};
use game_sim::replay::{PadTape, Recorder};
use game_sim::ride::Controls;
use game_sim::world::Flat;
use game_sim::{Sim, hash};
use ps2_float::F;

const DT: F = F(game_sim::tick::DeltaTime::CAP_30);
const GAS: u16 = 0x0040; // Cross, the shipped preset's gas
const BIKE: &str = "race125p";
const TRACK: &str = "free06";

fn pads(buttons: u16) -> [Report; PADS] {
    let mut r = [Report::missing(); PADS];
    r[0] = Report::from_buttons(buttons);
    r
}

/// One tick of the one rider on `buttons`, then their markers — and the tick on `tape` if one is
/// being written, as the shell's `--record` writes the pass it simulates.
fn tick(sim: &mut Sim, buttons: u16, tape: Option<&mut Recorder>) -> Option<Did> {
    let reports = pads(buttons);
    if let Some(tape) = tape {
        tape.frame(DT, &reports);
    }
    let spots: Vec<Spot> = vec![Spot::of(&sim.rider)];
    let hands = sim.poll(DT, &reports).stick_alone();
    sim.step(DT, &hands, &mut Flat::default());
    markers::frame(sim, 0, &spots, &hands)
}

/// A fresh ride with some speed on it: the same every time, as a ride from the start line is.
fn moving() -> Sim {
    let mut sim = Sim::new(0x99);
    for _ in 0..45 {
        tick(&mut sim, GAS, None);
    }
    assert!(F(sim.bike_vel().z()).abs() > F(1.0), "the bike is not moving: {:?}", sim.bike_vel());
    sim
}

/// A folder of this test's own, empty.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("openmx-p99-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// How many ticks Down or Up is held at 30 Hz before it fires (P106): the press's tick and seven
/// more, the timer crossing `markers::HOLD_MS` on the last.
const HOLD_TICKS: usize = 8;

/// `buttons` held for [`HOLD_TICKS`]: what the last tick fired, nothing having fired before it.
fn held(sim: &mut Sim, buttons: u16, mut tape: Option<&mut Recorder>) -> Option<Did> {
    for k in 1..HOLD_TICKS {
        assert_eq!(tick(sim, buttons, tape.as_deref_mut()), None, "fired before the hold, on tick {k}");
    }
    tick(sim, buttons, tape)
}

/// `d` pressed on the gas — Left and Right for a tick, Down and Up held until they fire — and let go
/// for one: what it fired.
fn press(sim: &mut Sim, d: Control, mut tape: Option<&mut Recorder>) -> Option<Did> {
    let did = match d {
        Control::Up | Control::Down => held(sim, GAS | d.mask(), tape.as_deref_mut()),
        _ => tick(sim, GAS | d.mask(), tape.as_deref_mut()),
    };
    tick(sim, GAS, tape);
    did
}

/// The ride as the shell keeps it: a marker placed writes the seat's file (`Kept::frame` after the
/// pass), and the next ride on the same track starts with it — and Up rides back to the very bike
/// the marker kept, bit for bit, ridden on from there exactly as the ride it was placed in.
#[test]
fn a_marker_kept_rides_back_bit_for_bit_in_the_next_ride() {
    let dir = folder("next");
    let profile = saved_markers::profile("", 0);
    assert_eq!(profile, "1P");

    // The first ride: two markers, slots 1 and 3, each written as it is placed.
    let mut first = moving();
    let mut kept = saved_markers::Kept::new(TRACK, &profile, BIKE, &first.markers);
    assert_eq!(kept.frame(&dir, &first.markers), None, "nothing placed, nothing written");
    let placed = held(&mut first, GAS | Control::Down.mask(), None);
    assert_eq!(placed, Some(Did::Placed(0)));
    let at_placement = first.rider.ride.clone();
    let after: Vec<u64> = (0..30)
        .map(|_| {
            tick(&mut first, GAS, None);
            hash::of(&first.rider.ride)
        })
        .collect();
    assert!(kept.frame(&dir, &first.markers).is_some(), "the placement was not written");
    let file = saved_markers::path(&dir, TRACK, &profile);
    assert!(file.exists(), "no file at {}", file.display());
    assert_eq!(press(&mut first, Control::Right, None), Some(Did::Stepped(Pick::Slot(1))));
    assert_eq!(press(&mut first, Control::Right, None), Some(Did::Stepped(Pick::Slot(2))));
    assert_eq!(kept.frame(&dir, &first.markers), None, "a step is not a change of the slots");
    assert_eq!(press(&mut first, Control::Down, None), Some(Did::Placed(2)));
    assert!(kept.frame(&dir, &first.markers).is_some());
    let slot3 = hash::of(&**first.markers.slots[2].as_ref().unwrap());
    drop(first); // the ride ends

    // The next ride on the same track, the same bike and player: the file's slots, on the first.
    let mut next = Sim::new(0x99);
    next.markers = saved_markers::load(&dir, TRACK, &profile, BIKE);
    assert_eq!(next.markers.at, 0);
    assert!(next.markers.holds(0) && !next.markers.holds(1) && next.markers.holds(2) && !next.markers.holds(3));
    assert_eq!(hash::of(&**next.markers.slots[0].as_ref().unwrap()), hash::of(&at_placement));
    assert_eq!(hash::of(&**next.markers.slots[2].as_ref().unwrap()), slot3);
    for _ in 0..20 {
        tick(&mut next, GAS, None);
    }
    assert_eq!(held(&mut next, GAS | Control::Up.mask(), None), Some(Did::Teleported(Pick::Slot(0))));
    assert_eq!(hash::of(&next.rider.ride), hash::of(&at_placement), "the teleport is not the bike the marker kept");
    let p = (next.rider.ride.position(), at_placement.position());
    assert_eq!((p.0.x().to_bits(), p.0.y().to_bits(), p.0.z().to_bits()), (p.1.x().to_bits(), p.1.y().to_bits(), p.1.z().to_bits()));
    for (k, want) in after.iter().enumerate() {
        tick(&mut next, GAS, None);
        assert_eq!(hash::of(&next.rider.ride), *want, "tick {} after the teleport parts from the ride it was placed in", k + 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The files are per track, per player and per bike: another track, another rider name and another
/// bike all start empty — and the right one is still there after them.
#[test]
fn another_track_starts_empty() {
    let dir = folder("tracks");
    let mut sim = moving();
    let profile = saved_markers::profile("Ryan", 0);
    assert_eq!(profile, "Ryan");
    let mut kept = saved_markers::Kept::new(TRACK, &profile, BIKE, &sim.markers);
    assert_eq!(held(&mut sim, GAS | Control::Down.mask(), None), Some(Did::Placed(0)));
    kept.frame(&dir, &sim.markers).expect("written");

    assert!(saved_markers::load(&dir, "free09", &profile, BIKE).fresh(), "another track has the marker");
    assert!(saved_markers::load(&dir, TRACK, "2P", BIKE).fresh(), "another player has the marker");
    assert!(saved_markers::load(&dir, TRACK, &profile, "race250p").fresh(), "another bike has the marker");
    assert!(saved_markers::load(&dir, TRACK, &profile, BIKE).holds(0));
    // A name is a file name and stays one, whatever it holds.
    let odd = saved_markers::path(&dir, "../free06", "a/b c");
    assert!(odd.starts_with(dir.join(saved_markers::DIR)), "{}", odd.display());
    assert_eq!(odd.file_name().unwrap(), "a_b_c.markers");

    // Every slot cleared removes the file.
    sim.markers.clear();
    kept.frame(&dir, &sim.markers).expect("cleared");
    assert!(!saved_markers::path(&dir, TRACK, &profile).exists());
    assert!(saved_markers::load(&dir, TRACK, &profile, BIKE).fresh());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file of another format — another version, another build's ride codec, cut short, or longer — is
/// dropped, never read as this one's and never a crash.
#[test]
fn another_format_is_dropped() {
    let mut sim = moving();
    held(&mut sim, GAS | Control::Down.mask(), None);
    let good = sim.markers.to_file(BIKE);
    assert!(Markers::from_file(&good, BIKE).is_ok());
    let mut version = good.clone();
    version[8] = version[8].wrapping_add(1);
    assert!(Markers::from_file(&version, BIKE).unwrap_err().contains("format"));
    let mut codec = good.clone();
    let codec_at = 8 + 4 + 4 + BIKE.len();
    codec[codec_at] ^= 0xff;
    assert!(Markers::from_file(&codec, BIKE).is_err());
    for cut in [0, 7, 15, codec_at + 2, good.len() - 1] {
        assert!(Markers::from_file(&good[..cut], BIKE).is_err(), "cut at {cut} read");
    }
    let mut longer = good.clone();
    longer.push(0);
    assert!(Markers::from_file(&longer, BIKE).is_err());
    // And through the shell's loader, from a file on disk.
    let dir = folder("format");
    let file = saved_markers::path(&dir, TRACK, "1P");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, &version).unwrap();
    assert!(saved_markers::load(&dir, TRACK, "1P", BIKE).fresh());
    let _ = std::fs::remove_dir_all(&dir);
    eprintln!("a markers file of one slot is {} bytes, of five {} bytes", good.len(), {
        let mut five = sim.markers.clone();
        five.slots = std::array::from_fn(|_| five.slots[0].clone());
        five.to_file(BIKE).len()
    });
}

/// A tape carries the markers its ride started with (and a clear from the pause), and replays tick for
/// tick with the files gone: the replay starts from the tape's markers, not the machine's.
#[test]
fn a_tape_replays_with_the_files_gone() {
    let dir = folder("tape");
    // A ride that keeps two markers, as the shell would.
    let mut before = moving();
    let mut kept = saved_markers::Kept::new(TRACK, "1P", BIKE, &before.markers);
    held(&mut before, GAS | Control::Down.mask(), None);
    press(&mut before, Control::Right, None);
    for _ in 0..15 {
        tick(&mut before, GAS, None);
    }
    press(&mut before, Control::Down, None);
    kept.frame(&dir, &before.markers).expect("written");

    // The ride the tape is of: it loads them, rides back to both and places a third, the pause clears
    // them all, and Up then finds nothing.
    let mut live = Sim::new(0x99);
    live.markers = saved_markers::load(&dir, TRACK, "1P", BIKE);
    assert!(!live.markers.fresh());
    let mut rec = Recorder::new("p99");
    rec.tape.markers = live.markers.to_file(BIKE);
    let mut hashes = Vec::new();
    let mut did = Vec::new();
    let (up, down) = ([GAS | Control::Up.mask(); HOLD_TICKS], [GAS | Control::Down.mask(); HOLD_TICKS]);
    let script: Vec<u16> = [GAS; 20]
        .into_iter()
        .chain(up)
        .chain([GAS, GAS, GAS | Control::Right.mask(), GAS])
        .chain(up)
        .chain([GAS; 10])
        .chain([GAS | Control::Right.mask(), GAS])
        .chain(down)
        .chain([GAS, GAS])
        .collect();
    for &b in &script {
        did.push(tick(&mut live, b, Some(&mut rec)));
        hashes.push(live.hash());
    }
    live.markers.clear();
    rec.markers_cleared();
    for &b in [GAS | Control::Left.mask(), GAS].iter().chain(&up).chain(&[GAS, GAS]) {
        did.push(tick(&mut live, b, Some(&mut rec)));
        hashes.push(live.hash());
    }
    let fired: Vec<Did> = did.iter().flatten().copied().collect();
    assert_eq!(
        fired,
        [
            Did::Teleported(Pick::Slot(0)),
            Did::Stepped(Pick::Slot(1)),
            Did::Teleported(Pick::Slot(1)),
            Did::Stepped(Pick::Slot(2)),
            Did::Placed(2),
            Did::Stepped(Pick::Slot(1)),
            Did::Refused(Pick::Slot(1)),
        ]
    );
    let text = rec.to_text();
    std::fs::remove_dir_all(&dir).unwrap(); // the files gone

    // The replay: the tape's file, its markers and its clear, and nothing from the folder.
    let tape = PadTape::parse(&text).unwrap();
    assert_eq!(tape.markers, rec.tape.markers);
    assert_eq!(tape.cleared, vec![script.len()]);
    assert_eq!(PadTape::parse(&tape.to_text()).unwrap(), tape, "the text round trip");
    let mut again = Sim::new(0x99);
    assert!(saved_markers::load(&dir, TRACK, "1P", BIKE).fresh());
    again.markers = tape.start_markers().unwrap().expect("the tape's markers");
    for (k, frame) in tape.frames.iter().enumerate() {
        if tape.cleared.contains(&k) {
            again.markers.clear();
        }
        let spots: Vec<Spot> = vec![Spot::of(&again.rider)];
        let hands = again.poll(frame.dt(), &frame.reports).stick_alone();
        again.step(frame.dt(), &hands, &mut Flat::default());
        markers::frame(&mut again, 0, &spots, &hands);
        assert_eq!(again.hash(), hashes[k], "the replay parts on frame {k}");
    }

    // A tape before P99 says nothing of markers, and its text is what it was.
    let mut old = Recorder::new("old");
    old.frame(DT, &pads(GAS));
    assert!(!old.to_text().contains("markers"));
    assert!(matches!(PadTape::parse(&old.to_text()).unwrap().start_markers(), Ok(None)));
    // A tape whose markers this build cannot read says so rather than replaying without them.
    let mut bad = tape.clone();
    bad.markers[8] ^= 1;
    assert!(bad.start_markers().is_err());
}

/// What a seat's slots are worth on the wire, for the pick that carries them online.
#[test]
fn markers_in_a_pick_round_trip() {
    let mut sim = moving();
    assert_eq!(press(&mut sim, Control::Down, None), Some(Did::Placed(0)));
    let mut pick = openmx_net::Pick::new("thq1", BIKE);
    pick.markers = saved_markers::to_pick(&sim.markers, BIKE, TRACK);
    let m = saved_markers::from_pick(&pick.markers, BIKE, TRACK);
    assert_eq!(saved_markers::signature(&m), saved_markers::signature(&sim.markers));
    assert!(m.holds(0) && (1..SLOTS).all(|s| !m.holds(s)));
    assert!(saved_markers::from_pick(&pick.markers, "race250p", TRACK).fresh(), "another bike's markers");
    assert!(saved_markers::from_pick(&pick.markers, BIKE, "free09").fresh(), "another track's markers");
    // A pick that says none — a build before P99's — is none, and so is junk.
    assert!(saved_markers::from_pick(&openmx_net::Pick::new("thq1", BIKE).markers, BIKE, TRACK).fresh());
    assert!(saved_markers::from_pick("free06:not markers", BIKE, TRACK).fresh());
    assert!(saved_markers::to_pick(&Markers::default(), BIKE, TRACK).is_empty());

    // The lobby's pick reads the player's file on the room's track, and again after a ride.
    let dir = folder("pick");
    let mut lobby = saved_markers::PickMarkers::default();
    assert_eq!(lobby.of(&dir, TRACK, "1P", BIKE), "");
    saved_markers::save(&dir, TRACK, "1P", BIKE, &sim.markers).unwrap();
    assert_eq!(lobby.of(&dir, TRACK, "1P", BIKE), "", "read again with nothing changed");
    lobby.stale();
    assert_eq!(lobby.of(&dir, TRACK, "1P", BIKE), pick.markers);
    assert_eq!(lobby.of(&dir, "free09", "1P", BIKE), "");
    let _ = std::fs::remove_dir_all(&dir);
}
