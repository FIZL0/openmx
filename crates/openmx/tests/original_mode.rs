//! P126's Original switch reaches the ride: a settings.txt and controls.txt saved with it on, beside
//! the disc the shell opens, start a ride from the front end with the original's fog and far clip,
//! the original's layout for every seat, the original's plants and the 4:3 picture; the same files
//! with it off ride every enhancement.
//!
//! The shell keeps its files beside the disc (`menu::store_dir`), so each run gets a directory of its
//! own: a `kit.toml` naming a link to the user's disc, and the files saved there, never the user's.

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

use menu::{Controls, Menu, Settings};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A store of its own for `name`, with the menu's files as `menu` has them; none without a disc.
fn store(name: &str, menu: &Menu) -> Option<PathBuf> {
    let iso = match game_render::disc::Disc::configured_path() {
        Ok(p) if p.is_file() => p,
        _ => {
            eprintln!("skipped: no disc");
            return None;
        }
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p126-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("kit.toml"), "[game]\niso = \"disc.iso\"\n").unwrap();
    std::os::unix::fs::symlink(&iso, dir.join("disc.iso")).unwrap();
    menu::save(&dir, &menu.settings, &menu.controls).unwrap();
    Some(dir)
}

/// What the shell says starting a ride from the paused front end in `dir`.
fn ride(dir: &Path) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(dir)
        .args(["--headless", "--menu", "--screen", "pause", "--frames", "2", "--no-sound"])
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    said
}

/// The first line the shell said that starts with `what`.
fn line<'a>(said: &'a str, what: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(what)).unwrap_or_else(|| panic!("no {what:?} line in:\n{said}"))
}

#[test]
fn the_original_switch_rides_the_originals_fog_layout_plants_and_picture() {
    let mut on = Menu::new(Settings::default(), Controls::default());
    on.set_original(true);
    let Some(dir) = store("on", &on) else { return };
    let said = ride(&dir);
    assert!(line(&said, "graphics:").starts_with("graphics: original (fog"), "{said}");
    assert_eq!(line(&said, "plants:"), "plants: original");
    // V56's leaves are a switch of their own since V57.
    assert_eq!(line(&said, "foliage:"), "foliage: original");
    assert_eq!(line(&said, "picture shape:"), "picture shape: 4:3");
    assert_eq!(line(&said, "controls:"), "controls: original");
    assert_eq!(line(&said, "biplane controls:"), "biplane controls: original");
    // And off, every enhancement.
    let off = Menu::new(Settings::default(), Controls::default());
    let dir_off = store("off", &off).unwrap();
    let said = ride(&dir_off);
    assert!(line(&said, "graphics:").starts_with("graphics: full (fog"), "{said}");
    assert_eq!(line(&said, "plants:"), "plants: smooth");
    assert_eq!(line(&said, "foliage:"), "foliage: cutout");
    assert_eq!(line(&said, "picture shape:"), "picture shape: fill");
    assert_eq!(line(&said, "controls:"), "controls: new");
    assert_eq!(line(&said, "biplane controls:"), "biplane controls: new");
    std::fs::remove_dir_all(dir).ok();
    std::fs::remove_dir_all(dir_off).ok();
}
