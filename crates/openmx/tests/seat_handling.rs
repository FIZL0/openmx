//! P315d: each seat's bike handles as its own Garage pick says (the Bike tab's Handling row, kept in
//! `garage.txt` as `seat<n>.handling = bike <handling> [<pack>]`): MX Unleashed's, or MX vs. ATV's or
//! Untamed's steps on a trick pack's bike files. The ride says what each seat rides.
//!
//! This rides the binary headless with `--garage`, as mxvatv_fliers.rs does. It skips with no disc,
//! and with no trick pack in this checkout's `mods/tricks/`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The trick pack the rides read, if this checkout has it; says SKIPPED when not.
fn pack() -> bool {
    let there = root().join("mods/tricks/mxvatv_tricks/mod.json").is_file();
    if !there {
        eprintln!("SKIPPED: no mods/tricks/mxvatv_tricks (openmx-mods' import-tricks makes it)");
    }
    there
}

/// Ride free09 headless for `players` on `garage`, a `garage.txt` beside a link to the disc in a
/// scratch folder of its own (the shell keeps its files beside the disc it was given), with `more`
/// flags, or `None` with no disc to ride.
fn ride(name: &str, players: usize, garage: &str, more: &[&str]) -> Option<String> {
    let iso = match game_render::disc::Disc::configured_path() {
        Ok(iso) => iso,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("p315d-seat-handling").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    let link = dir.join("disc.iso");
    std::os::unix::fs::symlink(std::path::absolute(&iso).expect("a path"), &link).expect("the disc is linked");
    std::fs::write(dir.join("garage.txt"), garage).expect("the garage is written");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(root())
        .arg(&link)
        .args(["--headless", "--track", "free09", "--frames", "10", "--steady-clock", "--mods", "mods", "--garage"])
        .args(["--players", &players.to_string()])
        .args(more)
        .output()
        .expect("the shell runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "the shell failed: {}\n{said}", String::from_utf8_lossy(&out.stderr));
    Some(said)
}

fn seat_lines(said: &str) -> Vec<&str> {
    said.lines().filter(|l| l.starts_with("handling: P")).collect()
}

#[test]
fn each_seat_rides_its_own_handling() {
    if !pack() {
        return;
    }
    // P1 on MX vs. ATV's, P2 on Untamed's with no pack named (the first in the folder), P3 none.
    let garage = "seat1.handling = bike mxvatv mxvatv_tricks\nseat2.handling = bike untamed\n";
    let Some(said) = ride("three", 3, garage, &[]) else { return };
    assert_eq!(
        seat_lines(&said),
        ["handling: P1 on MX vs. ATV's from mxvatv_tricks", "handling: P2 on Untamed's from mxvatv_tricks", "handling: P3 on the original's"],
        "\n{said}"
    );
    assert!(said.lines().any(|l| l == "player 2: handling: on Untamed's steps"), "\n{said}");
    assert!(!said.lines().any(|l| l == "handling: on Untamed's steps"), "P1's is MX vs. ATV's:\n{said}");

    // `--handling` is over every seat's pick.
    let Some(said) = ride("over", 2, garage, &["--handling", "original"]) else { return };
    assert!(seat_lines(&said).is_empty(), "\n{said}");

    // A pick naming a pack that is gone reads the folder's first, and says so.
    let Some(said) = ride("gone", 1, "seat1.handling = bike untamed gone_pack\n", &[]) else { return };
    assert_eq!(seat_lines(&said), ["handling: P1 on Untamed's from mxvatv_tricks"], "\n{said}");
    assert!(said.lines().any(|l| l.starts_with("handling: ") && l.contains("no trick pack gone_pack")), "\n{said}");

    // No picks: no line, as before.
    let Some(said) = ride("none", 2, "", &[]) else { return };
    assert!(seat_lines(&said).is_empty(), "\n{said}");
}
