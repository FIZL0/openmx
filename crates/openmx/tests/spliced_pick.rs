//! P149: every free-ride map spliced into one (P71) ridden from the front end, as the pick's
//! Cross on its row hands the shell `free_all` (tests/menu.rs walks the pick to it). The shell is
//! run as a user runs it, from a settings file of its own whose track is the splice, the front end
//! opened on the pause: that starts the settings' track through the same ride the pick's Cross
//! starts, and the ride has to be the spliced world. Skips without the disc.

// the browser's own clock, threads and files, which are std's here (P196)
#[allow(unused)]
#[path = "../src/files.rs"]
mod files;
#[path = "../src/menu.rs"]
#[allow(dead_code)]
mod menu;
#[path = "../src/vehicle.rs"]
#[allow(dead_code)]
mod vehicle;
#[path = "../src/roster.rs"]
#[allow(dead_code)]
mod roster;
#[path = "../src/dress.rs"]
#[allow(dead_code)]
mod dress;
#[path = "../src/join.rs"]
#[allow(dead_code)]
mod join;
#[path = "../src/split.rs"]
#[allow(dead_code)]
mod split;

use menu::{Controls, Settings};
use std::path::Path;
use std::process::Command;

#[test]
fn the_front_end_rides_the_spliced_world() {
    let iso = match game_render::disc::Disc::configured_path() {
        Ok(p) if p.is_file() => p,
        _ => {
            eprintln!("skipped: no disc");
            return;
        }
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p149-spliced-pick");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kit.toml"), "[game]\niso = \"disc.iso\"\n").unwrap();
    std::os::unix::fs::symlink(&iso, dir.join("disc.iso")).unwrap();
    let settings = Settings { track: menu::SPLICED.0.into(), ..Settings::default() };
    assert_eq!(Settings::parse(&settings.text()).track, "free_all", "the settings keep the splice");
    menu::save(&dir, &settings, &Controls::default()).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&dir)
        .args(["--headless", "--menu", "--screen", "pause", "--frames", "2", "--no-sound"])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    let line = |p: &str| said.lines().find(|l| l.starts_with(p)).unwrap_or_else(|| panic!("no {p:?} line in:\n{said}"));
    let world = line("world: free_all");
    assert!(world.contains("5 tiles"), "{world}");
    assert_eq!(line("world holes:"), "world holes: none, so only the outer edge throws");
    line("ride hash");
    let _ = std::fs::remove_dir_all(&dir);
}
