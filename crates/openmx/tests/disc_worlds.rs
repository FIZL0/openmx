//! P232: MX Unleashed's worlds built off the disc by the shell itself, with an empty mods folder.
//!
//! The first ride on one builds its joins with the stitcher (`crates/stitch`) from the disc's own
//! maps and keeps them under the mods folder; the second reads them back and builds nothing. What it
//! built is openmx-mods' world of the same id to the byte (its `mod.json` and every join's files), and
//! the ride on it is the ride on that installed world: the same world, wrap, holes and seams lines,
//! the seams walk reading every piece's edges. An installed world of the id is ridden instead.
//!
//! openmx-mods' worlds are looked for in `$OPENMX_WORLDS` (a mods folder), else in the main
//! checkout's `mods/`; without them the comparison is skipped and says so. Without the disc it all
//! skips. world_mxu's build is minutes and 850 MB, so its test is ignored: run it by hand.

use std::path::{Path, PathBuf};
use std::process::Command;

fn disc() -> bool {
    match game_render::disc::Disc::open_configured() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("skipped: {e}");
            false
        }
    }
}

/// openmx-mods' installed worlds: `$OPENMX_WORLDS`, else the main checkout's mods folder.
fn reference() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("OPENMX_WORLDS") {
        return Some(PathBuf::from(dir));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("git").current_dir(&root).args(["rev-parse", "--path-format=absolute", "--git-common-dir"]).output().ok()?;
    let common = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());
    Some(common.parent()?.join("mods"))
}

fn shell(mods: &Path, track: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--frames", "2", "--seams", "--track", track, "--mods"])
        .arg(mods)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    said
}

/// The ride's lines that say what world `id` is: its size and pieces, its wrap, holes and seams.
fn world_lines(said: &str, id: &str) -> Vec<String> {
    let head = format!("world: {id} (");
    let keep = [head.as_str(), "world wraps:", "world holes:", "world seams:", "world spawn:"];
    said.lines().filter(|l| keep.iter().any(|k| l.starts_with(k))).map(str::to_string).collect()
}

/// Every file under `dir`, by its path from there, with its bytes.
fn files(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push((p.strip_prefix(dir).unwrap().to_path_buf(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

fn rides_off_the_disc(id: &str, tiles: usize, joins: usize) {
    if !disc() {
        return;
    }
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p232-{id}"));
    let _ = std::fs::remove_dir_all(&mods);
    std::fs::create_dir_all(&mods).unwrap();

    // The first ride builds it.
    let first = shell(&mods, id);
    assert!(first.contains(&format!("{id}: building its joins")), "{first}");
    let lines = world_lines(&first, id);
    let world = lines.iter().find(|l| l.starts_with("world: ")).unwrap_or_else(|| panic!("no world line:\n{first}"));
    assert!(world.starts_with(&format!("world: {id} (")) && world.contains(&format!("{tiles} tiles and {joins} joins")), "{world}");
    assert!(lines.iter().any(|l| l == "world holes: none, so only the outer edge throws"), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("world seams:") && l.contains("no gaps")), "{lines:?}");
    let keys: Vec<PathBuf> = std::fs::read_dir(mods.join("cache/stitch").join(id)).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(keys.len(), 1, "one build kept: {keys:?}");
    let built = &keys[0];

    // The second reads it back.
    let second = shell(&mods, id);
    assert!(!second.contains("building its joins"), "{second}");
    assert_eq!(world_lines(&second, id), lines);

    // It is openmx-mods' world, and riding that one installed is the same ride.
    let Some(installed) = reference().filter(|r| r.join("tracks").join(id).join("mod.json").is_file()) else {
        eprintln!("skipped the comparison: no {id} in $OPENMX_WORLDS or the main checkout's mods/");
        return;
    };
    let want = installed.join("tracks").join(id);
    assert_eq!(std::fs::read(built.join("mod.json")).unwrap(), std::fs::read(want.join("mod.json")).unwrap(), "mod.json");
    let (got, want_joins) = (files(&built.join("joins")), files(&want.join("joins")));
    assert_eq!(got.len(), want_joins.len(), "join files");
    for ((p, b), (q, c)) in got.iter().zip(&want_joins) {
        assert_eq!(p, q);
        assert!(b == c, "{} differs from openmx-mods' {id}", p.display());
    }
    let theirs = shell(&installed, id);
    assert!(!theirs.contains("building its joins"), "the installed world is ridden:\n{theirs}");
    assert_eq!(world_lines(&theirs, id), lines, "the installed {id}'s ride");
    let _ = std::fs::remove_dir_all(&mods);
}

#[test]
fn world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it() {
    rides_off_the_disc("world_mxu_free", 5, 38 + 14);
}

#[test]
#[ignore = "builds world_mxu's 1222 joins (850 MB, minutes): run by hand"]
fn world_mxu_is_built_off_the_disc_as_openmx_mods_builds_it() {
    rides_off_the_disc("world_mxu", 27, 1222 + 328);
}
