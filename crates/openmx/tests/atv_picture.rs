//! MD138: the quad drawn. With `--atv` and openmx-mods' `mxvatv_atv` pack in this checkout's mods
//! folder, the shell rides free09 headless on a hand-written tape (switch to the quad, the gas, then
//! the stick hard left) and says under `--trace-picture` what each drawn frame did: the build's
//! chassis at the quad's matrix with its parts, its wheels posed from the step — spin, steer and
//! compression (`picture atv:`) — and the rider on it on the pack's ATV motion set (`pilot:`), by
//! both renderers. Each run writes a shot, looked at by eye. Without `--atv` nothing of it is drawn.
//! Skips without a disc or the pack.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md138-atv-picture");
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir.join(name)
}

/// A tape of `frames` ticks: the switch to the quad at tick 30, the gas (R2) from tick 40 and the
/// left stick hard left from tick 110.
fn tape(frames: usize) -> PathBuf {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape the quad\n");
    for f in 0..frames {
        let gas = if f >= 40 { "ff" } else { "00" };
        let lx = if f >= 110 { "00" } else { "7f" };
        text += &format!("frame 0x3d08850a 0x0079ffbf7f7f{lx}7f000000000000{gas}{} -\n", "00".repeat(17));
    }
    text += "switch 30 atv\n";
    let path = scratch("quad.tape");
    std::fs::write(&path, text).expect("the tape is written");
    path
}

/// The shell on free09, headless on `renderer`, for `frames`, writing a shot of the last but one:
/// what it said. `None` with no disc.
fn ride(renderer: &str, frames: usize, more: &[&str], shot: &Path) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .args(["--headless", "--track", "free09", "--mods", "mods", "--size", "640x448", "--overlay", "none", "--steady-clock"])
        .args(["--renderer", renderer, "--trace-picture", "--frames", &frames.to_string(), "--play"])
        .arg(tape(frames + 5))
        .arg("--shot")
        .arg(shot)
        .args(["--shot-frame", &(frames - 2).to_string()])
        .args(more)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
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

/// A `picture atv:` line's four wheels: spin, steer and compression each.
fn wheels(line: &str) -> Vec<[f32; 3]> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|x| *x == "wheels").unwrap_or_else(|| panic!("no wheels: {line}"));
    w[i + 1..i + 5]
        .iter()
        .map(|t| {
            let n: Vec<f32> = t.split('/').map(|v| v.parse().unwrap_or_else(|_| panic!("a wheel: {line}"))).collect();
            [n[0], n[1], n[2]]
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
fn the_quad_is_drawn_with_its_rider() {
    if !root().join("mods/machines/mxvatv_atv/mod.json").is_file() {
        eprintln!("SKIPPED: no mods/machines/mxvatv_atv (openmx-mods' import-atv makes it)");
        return;
    }
    for renderer in ["gpu", "software"] {
        let shot = scratch(&format!("quad-{renderer}.png"));
        let Some(said) = ride(renderer, 150, &["--atv", "atv_Race250P"], &shot) else { return };
        assert!(said.lines().any(|l| l.starts_with("atv: drawn as Veh/Anim/ATV_R_")), "{renderer}: no build drawn:\n{said}");
        assert!(said.lines().any(|l| l == "vehicle: atv"), "{renderer}: never on the quad:\n{said}");
        assert_eq!(png_size(&shot), (640, 448), "{}", shot.display());

        let frames: Vec<&str> = said.lines().filter(|l| l.starts_with(&format!("picture atv: seat 0 {renderer} "))).collect();
        assert!(frames.len() >= 100, "{renderer}: the quad was hardly drawn ({} frames):\n{said}", frames.len());
        assert!(frames.iter().all(|l| word(l, renderer) == "quad:250r_atv_chasis"), "{renderer}: another chassis: {}", frames[0]);
        assert!(frames.iter().all(|l| word(l, "parts").parse::<usize>().unwrap() > 0), "{renderer}: no parts on it: {}", frames[0]);

        // Posed from the step: the springs take its weight, the gas spins every wheel, the stick
        // steers the fronts and not the rears.
        let posed: Vec<Vec<[f32; 3]>> = frames.iter().map(|l| wheels(l)).collect();
        assert!(posed.iter().any(|w| w.iter().all(|t| t[2] > 0.05)), "{renderer}: the springs never took the weight");
        for k in 0..4 {
            assert!(posed.iter().any(|w| (w[k][0] - posed[0][k][0]).abs() > 0.5), "{renderer}: wheel {k} never spun");
        }
        let last = &posed[posed.len() - 1];
        assert!(last[0][1] < -0.05 && last[1][1] < -0.05, "{renderer}: the fronts are not steered left: {}", frames[frames.len() - 1]);
        assert!(last[2][1] == 0.0 && last[3][1] == 0.0, "{renderer}: a rear is steered: {}", frames[frames.len() - 1]);
        let (first, moved): ([f32; 3], [f32; 3]) = (after(frames[0], "at"), after(frames[frames.len() - 1], "at"));
        assert!((moved[0] - first[0]).hypot(moved[2] - first[2]) > 10.0, "{renderer}: the gas never moved it");
        assert!(frames.iter().any(|l| word(l, "row") == "ATV_turn_riding_Mid"), "{renderer}: never on the riding row");

        // Its rider, every frame, rooted at the quad's origin, as the motion set's clips are.
        let riders: Vec<&str> = said.lines().filter(|l| l.starts_with(&format!("pilot: seat 0 {renderer} "))).collect();
        assert!(riders.len() >= frames.len(), "{renderer}: frames without a rider:\n{said}");
        for line in riders {
            let root: [f32; 3] = after(line, "root");
            assert!(root.iter().all(|d| d.abs() < 1e-3), "{renderer}: the rider's root is off the quad's origin: {line}");
            assert!(word(line, "of").parse::<usize>().unwrap() > 0, "{renderer}: nothing of the rider was drawn: {line}");
            let middle: [f32; 3] = after(line, "middle");
            assert!(middle[1] > 1.5 && middle[1] < 5.0, "{renderer}: the rider is not sat on it: {line}");
        }
    }

    // Without --atv nothing of the quad is read or drawn.
    let shot = scratch("off.png");
    let Some(said) = ride("gpu", 60, &[], &shot) else { return };
    assert!(!said.lines().any(|l| l.starts_with("picture atv:") || l.starts_with("atv: drawn")), "the quad drawn unpicked:\n{said}");
}
