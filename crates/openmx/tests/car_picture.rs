//! S245: the cars drawn in the app. The shell rides each car's own track headless and says under
//! `--trace-picture` what each drawn frame did. Parked, the car stands solid in play where
//! `machine.txt` puts it, on the ground (`picture parked:`, F72), with no see-through bay drawn
//! inside it (F74; a bay elsewhere, as free07's, still is), and a track that is not its own parks
//! none. Taken (a hand-written tape's switch, then the gas held), it is drawn at its bay and on, by
//! both renderers, posed by its tires — their spin and their suspension's compression
//! (`picture car:`) — with its driver rooted in the seat (`pilot:`). Each run writes a shot, which is
//! looked at by eye. Skips without a disc.

use std::path::{Path, PathBuf};
use std::process::Command;

use game_render::car::{self, Car};
use game_render::world::Assets;

/// Each car: its track, the tape's machine name and its model.
const CARS: [(&str, &str, &str); 3] = [("free09", "monster", "monstertruck"), ("free07", "baja", "baja"), ("free11", "buggy", "buggy")];

/// A tape of `frames` ticks that switches into `machine` at tick 10 (none for `None`) and holds the
/// gas (R2) from tick 30.
fn tape(frames: usize, machine: Option<&str>) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the {}\n", machine.unwrap_or("bike"));
    for f in 0..frames {
        let gas = if f >= 30 && machine.is_some() { "ff" } else { "00" };
        text += &format!("frame 0x3d08850a 0x0079ffbf7f7f7f7f000000000000{gas}{} -\n", "00".repeat(17));
    }
    match machine {
        Some(m) => text + &format!("switch 10 {m}\n"),
        None => text,
    }
}

fn scratch() -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("s245-car-picture");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// The shell on `track`, headless on `renderer`, playing [`tape`] for `machine` for `frames` and
/// writing a shot of the last but one to `shot` (from `pose` where given): what it said.
fn ride(track: &str, machine: Option<&str>, renderer: &str, frames: usize, shot: &Path, pose: Option<String>) -> String {
    let play = scratch().join(format!("{track}-{}-{renderer}.tape", machine.unwrap_or("none")));
    std::fs::write(&play, tape(frames + 5, machine)).expect("the tape is written");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_openmx"));
    cmd.current_dir(&root)
        .args(["--headless", "--track", track, "--size", "640x448", "--overlay", "none", "--steady-clock"])
        .args(["--renderer", renderer, "--trace-picture", "--frames", &frames.to_string(), "--play"])
        .arg(&play)
        .arg("--shot")
        .arg(shot)
        .args(["--shot-frame", &(frames - 2).to_string()]);
    if let Some(pose) = pose {
        cmd.args(["--shot-pose", &pose]);
    }
    let out = cmd.output().expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    said
}

/// The numbers after `key` in `line`.
fn after<const N: usize>(line: &str, key: &str) -> [f32; N] {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == key).unwrap_or_else(|| panic!("no {key}: {line}"));
    std::array::from_fn(|j| w[i + 1 + j].parse().unwrap_or_else(|_| panic!("not a number after {key}: {line}")))
}

/// One word after `key` in `line`.
fn word<'a>(line: &'a str, key: &str) -> &'a str {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == key).unwrap_or_else(|| panic!("no {key}: {line}"));
    w[i + 1]
}

/// A `picture car:` line's four tires: spin, steer, compression and scale each.
fn tires(line: &str) -> Vec<[f32; 4]> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == "tires").unwrap_or_else(|| panic!("no tires: {line}"));
    w[i + 1..i + 5]
        .iter()
        .map(|t| {
            let n: Vec<f32> = t.split('/').map(|v| v.parse().unwrap_or_else(|_| panic!("a tire: {line}"))).collect();
            [n[0], n[1], n[2], n[3]]
        })
        .collect()
}

/// A png's width and height, read off its header.
fn png_size(path: &Path) -> (u32, u32) {
    let png = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&png[1..4], b"PNG", "{}: not a png", path.display());
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

#[test]
fn each_car_parks_on_its_own_track_and_is_drawn_driven() {
    let mut disc = match game_render::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return;
        }
    };
    let archives = [disc.res(car::ARCHIVE).expect("the machines' archive")];
    let mut assets = Assets::default();
    for (track, machine, model) in CARS {
        let drawn = Car::load(&mut assets, &archives, model).unwrap_or_else(|| panic!("{model} is not on the disc"));
        let seat = drawn.seat(&assets).expect("the seat");
        let key = format!("car:{model}");

        // Parked: solid where the Sim holds it in play (F72), on the ground under the file's spot,
        // and its see-through bay not drawn inside it (F74): free09's and free11's bays are where
        // the car rests, so they are not drawn; free07's is 140 ft from its baja and is.
        let shot = scratch().join(format!("{track}-parked.png"));
        let said = ride(track, None, "gpu", 12, &shot, None);
        assert!(said.contains(&format!("{machine} parked here")), "{track}: the {machine} is not parked:\n{said}");
        let parked = said.lines().find(|l| l.starts_with("picture parked: ")).unwrap_or_else(|| panic!("{track}: no parked car drawn:\n{said}"));
        assert_eq!(word(parked, "gpu"), key, "{parked}");
        assert!(said.lines().filter(|l| l.starts_with("picture parked: ")).all(|l| word(l, "gpu") == key), "{track}: another car parked:\n{said}");
        let rest: [f32; 3] = after(parked, "at");
        assert!(rest[1] > 100.0, "{track}: the parked car is not on the ground: {parked}");
        let bays: Vec<&str> = said.lines().filter(|l| l.starts_with("picture bay: car:")).collect();
        assert!(bays.iter().all(|l| word(l, "bay:") == key), "{track}: another car's bay:\n{said}");
        for line in &bays {
            let at: [f32; 3] = after(line, "at");
            assert!((at[0] - rest[0]).hypot(at[2] - rest[2]) > 10.0, "{track}: the bay is drawn inside the parked car {rest:?}: {line}");
        }
        assert_eq!(bays.is_empty(), track != "free07", "{track}: bays drawn {bays:?}");
        // The bay, where the car is taken: the parked car's spot where it is not drawn.
        let at: [f32; 3] = bays.first().map_or(rest, |l| after(l, "at"));
        assert!(at[1] > 100.0, "{track}: the bay is not on the ground: {at:?}");
        // And a shot of it from behind it, looking along +z.
        let shot = scratch().join(format!("{track}-parked-behind.png"));
        let pose = format!("{},{},{},0,-12,60", at[0], at[1] + 10.0, at[2] - 34.0);
        ride(track, None, "gpu", 12, &shot, Some(pose));
        assert_eq!(png_size(&shot), (640, 448), "{}", shot.display());

        for renderer in ["gpu", "software"] {
            let shot = scratch().join(format!("{track}-{machine}-{renderer}.png"));
            let said = ride(track, Some(machine), renderer, 70, &shot, None);
            assert!(said.lines().any(|l| l == format!("vehicle: {machine}")), "{renderer}: never in the {machine}:\n{said}");
            assert_eq!(png_size(&shot), (640, 448), "{}", shot.display());
            let frames: Vec<&str> = said.lines().filter(|l| l.starts_with(&format!("picture car: seat 0 {renderer} "))).collect();
            assert!(frames.len() >= 40, "{renderer}: the {machine} was hardly drawn:\n{said}");
            assert!(frames.iter().all(|l| word(l, renderer) == key), "{renderer}: another model: {}", frames[0]);

            // Taken at its bay: the first frame stands it there; where its bay is not drawn, within
            // the 10 ft the parked car hides it by (the truck rests 0.05 ft off it, the buggy 1.06).
            let first: [f32; 3] = after(frames[0], "at");
            let near = if bays.is_empty() { 10.0 } else { 0.01 };
            assert!((first[0] - at[0]).hypot(first[2] - at[2]) < near, "{renderer}: not taken at its bay {at:?}: {}", frames[0]);

            // Posed by its tires: the springs take its weight and the gas spins them.
            let (rest, last) = (tires(frames[0]), tires(frames[frames.len() - 1]));
            assert!(rest.iter().all(|t| t[0] == 0.0 && t[2] == 0.0), "{renderer}: not at rest when taken: {}", frames[0]);
            assert!(frames.iter().any(|l| tires(l).iter().all(|t| t[2] > 0.1)), "{renderer}: the springs never took the weight");
            assert!(last.iter().all(|t| t[0] != 0.0), "{renderer}: the tires never spun: {}", frames[frames.len() - 1]);
            let moved: [f32; 3] = after(frames[frames.len() - 1], "at");
            assert!((moved[0] - first[0]).hypot(moved[2] - first[2]) > 1.0, "{renderer}: the gas never moved it");

            // Its driver, every frame, rooted in the seat.
            let pilots: Vec<&str> = said.lines().filter(|l| l.starts_with(&format!("pilot: seat 0 {renderer} "))).collect();
            assert!(pilots.len() >= frames.len(), "{renderer}: frames without a driver:\n{said}");
            for line in pilots {
                let root: [f32; 3] = after(line, "root");
                let count: usize = word(line, "of").parse().unwrap();
                assert!(count > 0, "{renderer}: nothing of the driver was drawn: {line}");
                let off = [root[0] - seat.x, root[1] - seat.y, root[2] - seat.z];
                assert!(off.iter().all(|d| d.abs() < 1e-3), "{renderer}: the driver's root is not in the seat {seat:?}: {line}");
            }
        }
    }

    // A track that is no car's parks none.
    let shot = scratch().join("free06-parked.png");
    let said = ride("free06", None, "gpu", 12, &shot, None);
    assert!(said.contains("none parked here"), "free06 parks a car:\n{said}");
    assert!(!said.lines().any(|l| l.starts_with("picture bay: car:")), "free06 draws a car's bay:\n{said}");}

/// S265: [`tape`] with the left stick's x held at `x` from tick 30 beside the gas: `00` full left,
/// `ff` full right, `7f` centred.
fn steer_tape(frames: usize, machine: &str, x: &str) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the {machine} steered {x}\n");
    for f in 0..frames {
        let (gas, x) = if f >= 30 { ("ff", x) } else { ("00", "7f") };
        text += &format!("frame 0x3d08850a 0x0079ffbf7f7f{x}7f000000000000{gas}{} -\n", "00".repeat(17));
    }
    text + &format!("switch 10 {machine}\n")
}

/// S265: a driven car's driver leans with its steer. The original hands a car's driver one clip
/// (`buggy_steer` for the buggy, `baja_steer` for the truck and the baja), played at the body's
/// phase axis, which the car's driver request eases towards `0.5 * (1 + steer)`. Each car is driven
/// on the gas with the stick held left, then right: its driver plays its car's clip, the phase
/// follows the steer to the other side of a half, and the body drawn leans across (its head, the
/// topmost node, moves across the cab). A shot of each turn is written for the eye. Skips without a disc.
#[test]
fn a_driven_cars_driver_leans_with_its_steer() {
    if let Err(e) = game_render::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let frames = 70;
    for (track, machine, model) in CARS {
        let clip = car::steer_clip(model);
        let mut heads = Vec::new();
        for (side, x) in [("left", "00"), ("right", "ff")] {
            let play = scratch().join(format!("s265-{machine}-{side}.tape"));
            std::fs::write(&play, steer_tape(frames + 5, machine, x)).expect("the tape is written");
            let shot = scratch().join(format!("s265-{machine}-{side}.png"));
            let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
                .current_dir(&root)
                .args(["--headless", "--track", track, "--size", "640x448", "--overlay", "none", "--steady-clock"])
                .args(["--renderer", "gpu", "--trace-picture", "--frames", &frames.to_string(), "--play"])
                .arg(&play)
                .arg("--shot")
                .arg(&shot)
                .args(["--shot-frame", &(frames - 2).to_string()])
                .output()
                .expect("the shell runs");
            let said = String::from_utf8_lossy(&out.stdout).to_string();
            assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(png_size(&shot), (640, 448), "{}", shot.display());
            let drivers: Vec<&str> = said.lines().filter(|l| l.starts_with("picture driver: seat 0 ")).collect();
            let last = drivers.last().unwrap_or_else(|| panic!("{machine} {side}: no driver drawn:\n{said}"));
            assert_eq!(word(last, "clip"), clip, "{machine} {side}: not posed by its car's clip: {last}");
            let [phase] = after::<1>(last, "phase");
            let [steer] = after::<1>(last, "steer");
            assert!(steer.abs() > 0.5, "{machine} {side}: the stick never steered: {last}");
            // The lag has caught the target up by the end: the phase on the steer's side of a half.
            let want = 0.5 * (1.0 + steer);
            assert!((phase - want).abs() < 0.05, "{machine} {side}: phase {phase}, the steer's {want}: {last}");
            heads.push((steer, after::<3>(last, "top")));
        }
        let ((s_left, h_left), (s_right, h_right)) = (heads[0], heads[1]);
        assert!(s_left.signum() != s_right.signum(), "{machine}: left and right steered the same way: {s_left} {s_right}");
        // Held to the wheel and the seat, the head swings ~0.13 ft across the monster's cab from
        // full lock to full lock; under the plane pilot's default lean it never moved at all.
        let lean = (h_left[0] - h_right[0]).hypot(h_left[2] - h_right[2]);
        eprintln!("{machine}: steer {s_left:.3}/{s_right:.3}, the head at {h_left:?}/{h_right:?}, {lean:.3} ft apart");
        assert!(lean > 0.05, "{machine}: the driver does not lean across: head {h_left:?} vs {h_right:?}");
    }
}
