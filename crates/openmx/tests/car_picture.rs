//! S245: the cars drawn in the app. The shell rides each car's own track headless and says under
//! `--trace-picture` what each drawn frame did. Parked, the car stands see-through in the bay
//! `machine.txt` puts it in, on the ground (`picture bay:`), and a track that is not its own parks
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

        // Parked: see-through in its bay, on the ground under the file's spot.
        let shot = scratch().join(format!("{track}-parked.png"));
        let said = ride(track, None, "gpu", 12, &shot, None);
        assert!(said.contains(&format!("{machine} parked here")), "{track}: the {machine} is not parked:\n{said}");
        let bay = said.lines().find(|l| l.starts_with("picture bay: ")).unwrap_or_else(|| panic!("{track}: no bay drawn:\n{said}"));
        assert_eq!(word(bay, "bay:"), key, "{bay}");
        assert!(said.lines().filter(|l| l.starts_with("picture bay: ")).all(|l| word(l, "bay:") == key), "{track}: another car parked:\n{said}");
        let at: [f32; 3] = after(bay, "at");
        assert!(at[1] > 100.0, "{track}: the bay is not on the ground: {bay}");
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

            // Taken at its bay: the first frame stands it there.
            let first: [f32; 3] = after(frames[0], "at");
            assert!((first[0] - at[0]).abs() < 0.01 && (first[2] - at[2]).abs() < 0.01, "{renderer}: not taken at its bay {at:?}: {}", frames[0]);

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
    assert!(!said.lines().any(|l| l.starts_with("picture bay: car:")), "free06 draws a car's bay:\n{said}");
}
