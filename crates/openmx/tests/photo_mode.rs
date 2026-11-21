//! P385: photo mode. The shell itself, headless, opens a ride under photo mode (`--menu --screen
//! photo`, the pause with photo mode over it) and takes a picture on a frame (`--photo-at`, as its
//! Cross would). Photo mode holds the ride still — the ridden seat's simulation tick count is the
//! same from the frame it opened to the frame the picture was taken, while the engine ticked on —
//! and the picture is a PNG at the window's size.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A PNG's width and height, from its header.
fn png_size(path: &Path) -> (usize, usize) {
    let png = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{}: not a PNG", path.display());
    assert_eq!(&png[12..16], b"IHDR");
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
    (be(16), be(20))
}

/// The number after `word` in `line`.
fn number_after(line: &str, word: &str) -> u64 {
    let at = line.find(word).unwrap_or_else(|| panic!("no {word:?} in {line:?}")) + word.len();
    line[at..].trim_start().split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
}

/// One headless ride under photo mode at `size`, its picture taken on frame 30 of 40; what it said.
fn ride(name: &str, size: &str) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let iso = game_render::disc::Disc::configured_path().expect("an opened disc has a path");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p385-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kit.toml"), "[game]\niso = \"disc.iso\"\n").unwrap();
    std::os::unix::fs::symlink(&iso, dir.join("disc.iso")).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&dir)
        .args(["--headless", "--steady-clock", "--sim-thread", "off", "--no-sound", "--trace-hash"])
        .args(["--size", size, "--menu", "--screen", "photo", "--frames", "40", "--photo-at", "30"])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{name}: the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

#[test]
fn photo_mode_holds_the_ride_and_shoots_at_the_window_size() {
    for (name, size, want) in [("small", "640x448", (640, 448)), ("wide", "960x540", (960, 540))] {
        let Some(said) = ride(name, size) else { return };
        let lines: Vec<&str> = said.lines().collect();
        let opened = lines.iter().position(|l| l.starts_with("photo mode: the ride frozen")).unwrap_or_else(|| panic!("{name}: photo mode never opened\n{said}"));
        let shot = lines.iter().position(|l| l.starts_with("photo: Photo saved: ")).unwrap_or_else(|| panic!("{name}: no picture\n{said}"));
        assert!(shot > opened, "{name}");
        // The tick count where it opened is the one the picture was taken at.
        let (from, to) = (number_after(lines[opened], "sim tick"), number_after(lines[shot], "sim tick"));
        assert_eq!(from, to, "{name}: the ride moved under photo mode");
        // While the engine ran on, a tick a frame: the ride was held, not the run.
        let engine: Vec<u64> = lines[opened..shot].iter().filter(|l| l.starts_with("tick ")).map(|l| number_after(l, "tick")).collect();
        assert!(engine.len() >= 20 && engine.windows(2).all(|w| w[1] > w[0]), "{name}: the engine's ticks under photo mode: {engine:?}");
        // The picture: a PNG at the window's size.
        let path: PathBuf = lines[shot]["photo: Photo saved: ".len()..].rsplit_once(" (").unwrap().0.into();
        assert_eq!(path.file_name().and_then(|n| n.to_str()), Some("photo-0001.png"), "{name}");
        assert_eq!(png_size(&path), want, "{name}: {}", path.display());
    }
}
