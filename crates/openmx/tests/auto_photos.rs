//! P390: Auto Photos. The shell itself, headless, rides free06 on a tape that holds the gas and
//! taps the No Hander (Triangle, the stick up) every 40 ticks. With `--auto-photos tricks` the
//! first landed trick takes a picture on its landing tick, a PNG at the window's size in the photos
//! folder, and the ones landed within the gap after it take none; without the flag nothing is
//! written.

use std::path::{Path, PathBuf};
use std::process::Command;

/// A PNG's width and height, from its header.
fn png_size(path: &Path) -> (usize, usize) {
    let png = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "{}: not a PNG", path.display());
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
    (be(16), be(20))
}

/// The number after `word` in `line`.
fn number_after(line: &str, word: &str) -> u64 {
    let at = line.find(word).unwrap_or_else(|| panic!("no {word:?} in {line:?}")) + word.len();
    line[at..].trim_start().split(|c: char| !c.is_ascii_digit()).next().unwrap().parse().unwrap()
}

/// The tape: 30 Hz ticks, Cross (the gas) held throughout, and from tick 61 Triangle with the left
/// stick up for 8 ticks of every 40 (the original's No Hander).
fn tape(n: usize) -> String {
    let mut out = format!("# game-sim pad tape: {n} frames\nformat 1\ntape thq1 on free06 / 125sx\n");
    for t in 0..n {
        let mut b = [0u8; 32];
        b[..8].copy_from_slice(&[0x00, 0x79, 0xff, 0xbf, 0x7f, 0x7f, 0x7f, 0x7f]);
        b[14] = 0xff;
        if t % 40 < 8 && t > 60 {
            b[3] &= 0xef;
            b[12] = 0xff;
            b[7] = 0x00;
        }
        let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
        out += &format!("frame 0x3d08850a 0x{hex} -\n");
    }
    out
}

/// One headless ride of the tape in its own folder, with `extra` flags; the folder and what it said.
fn ride(name: &str, extra: &[&str]) -> Option<(PathBuf, String)> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let iso = game_render::disc::Disc::configured_path().expect("an opened disc has a path");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p390-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kit.toml"), "[game]\niso = \"disc.iso\"\n").unwrap();
    std::os::unix::fs::symlink(&iso, dir.join("disc.iso")).unwrap();
    std::fs::write(dir.join("tricks.tape"), tape(240)).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&dir)
        .args(["--headless", "--steady-clock", "--sim-thread", "off", "--no-sound", "--size", "640x448"])
        .args(["--track", "free06", "--play", "tricks.tape", "--frames", "240"])
        .args(extra)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(out.status.success(), "{name}: the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some((dir, said))
}

#[test]
fn a_landed_trick_takes_one_picture_and_none_without_the_switch() {
    let Some((dir, said)) = ride("tricks", &["--auto-photos", "tricks"]) else { return };
    let lines: Vec<&str> = said.lines().collect();
    let landed: Vec<usize> = (0..lines.len()).filter(|&k| lines[k].starts_with("landed: ")).collect();
    assert!(landed.len() >= 2, "the tape landed fewer than two tricks:\n{said}");
    let shots: Vec<usize> = (0..lines.len()).filter(|&k| lines[k].starts_with("auto photo: Trick at sim tick ")).collect();
    assert_eq!(shots.len(), 1, "one picture for tricks landed inside the gap:\n{said}");
    assert!(shots[0] > landed[0] && shots[0] < landed[1], "the picture is the first landing's:\n{said}");
    let saved = lines[shots[0] + 1];
    assert!(saved.starts_with("photo: Photo saved: "), "{saved}");
    // Taken on the tick it was due: the one the shell wrote it at.
    assert_eq!(number_after(lines[shots[0]], "sim tick"), number_after(saved, "sim tick"));
    let path = dir.join("photos").join("photo-0001.png");
    assert_eq!(png_size(&path), (640, 448));
    assert!(!dir.join("photos").join("photo-0002.png").exists());

    let Some((dir, said)) = ride("off", &[]) else { return };
    assert!(said.lines().any(|l| l.starts_with("landed: ")), "{said}");
    assert!(!said.contains("auto photo"), "{said}");
    assert!(!dir.join("photos").exists(), "nothing written without the switch");
}
