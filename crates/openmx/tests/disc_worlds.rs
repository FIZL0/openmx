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
//! skips. world_mxu (P253's WORLD, the freerides together, its maps close since P251) and
//! world_mxu_nat are built and ridden once each, a minute or so and some 400 MB apiece, removed
//! after.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

/// The most a first ride on a world off the disc may take, its joins built from cold (P260): the user's
/// MX WORLD that "is not loading". world_mxu was 92 s in a release build on 2026-11-02 (132 s before
/// its tiles were cut side by side), world_mxu_free some 10 s; the room is for a test build on a
/// machine that runs three gates at once.
const COLD: Duration = Duration::from_secs(480);
/// The most a ride on a built or installed world may take, its joins read back and nothing built, with
/// `--seams` walking every piece's edges (installed world_mxu's 790 joins: 88 s beside the cold builds).
const WARM: Duration = Duration::from_secs(240);
/// The most a plain ride on an installed world may take to start (P260): its 27 tiles and every join
/// read. world_mxu was some 5 s in a release build.
const INSTALLED: Duration = Duration::from_secs(60);

/// [`shell`], held to `most`.
fn shell_within(mods: &Path, track: &str, most: Duration) -> String {
    let at = Instant::now();
    let said = shell(mods, track);
    let took = at.elapsed();
    eprintln!("{track} from {}: {:.1} s", mods.display(), took.as_secs_f32());
    assert!(took <= most, "{track} took {:.0} s to ride, at most {:.0}", took.as_secs_f32(), most.as_secs_f32());
    said
}

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
    shell_with(mods, track, &["--seams"])
}

fn shell_with(mods: &Path, track: &str, more: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(more)
        .args(["--headless", "--no-sound", "--frames", "2", "--track", track, "--mods"])
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
    let first = shell_within(&mods, id, COLD);
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
    let second = shell_within(&mods, id, WARM);
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
    let theirs = shell_within(&installed, id, WARM);
    assert!(!theirs.contains("building its joins"), "the installed world is ridden:\n{theirs}");
    assert_eq!(world_lines(&theirs, id), lines, "the installed {id}'s ride");
    let _ = std::fs::remove_dir_all(&mods);
}

/// P260: the worlds the user has installed (the main checkout's mods/, where openmx-mods puts them)
/// ride, and within [`INSTALLED`]: an installed world of a disc world's id is ridden in its place, its 27
/// tiles and every join read. Skips what is not installed.
#[test]
fn the_installed_mx_unleashed_worlds_ride_promptly() {
    if !disc() {
        return;
    }
    let Some(installed) = reference() else { return };
    for id in ["world_mxu", "world_mxu_free"] {
        if !installed.join("tracks").join(id).join("mod.json").is_file() {
            eprintln!("skipped {id}: not in $OPENMX_WORLDS or the main checkout's mods/");
            continue;
        }
        let at = Instant::now();
        let said = shell_with(&installed, id, &[]);
        let took = at.elapsed();
        eprintln!("{id} installed: {:.1} s", took.as_secs_f32());
        assert!(took <= INSTALLED, "the installed {id} took {:.0} s to ride, at most {:.0}", took.as_secs_f32(), INSTALLED.as_secs_f32());
        assert!(!said.contains("building its joins"), "the installed world is ridden:\n{said}");
        assert!(said.lines().any(|l| l.starts_with(&format!("world: {id} ("))), "{said}");
        assert!(said.lines().any(|l| l.starts_with("ridden for")), "{said}");
    }
}

#[test]
fn world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it() {
    rides_off_the_disc("world_mxu_free", 5, 35 + 13);
}

/// The first ride on `id` off the disc into an empty mods folder: what it said, and the built
/// world's `mod.json`. `None` without the disc.
fn first_ride(id: &str) -> Option<(String, String, PathBuf)> {
    if !disc() {
        return None;
    }
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("p253-{id}"));
    let _ = std::fs::remove_dir_all(&mods);
    std::fs::create_dir_all(&mods).unwrap();
    let said = shell_within(&mods, id, COLD);
    assert!(said.contains(&format!("{id}: building its joins")), "{said}");
    let keys: Vec<PathBuf> = std::fs::read_dir(mods.join("cache/stitch").join(id)).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(keys.len(), 1, "one build kept: {keys:?}");
    let json = std::fs::read_to_string(keys[0].join("mod.json")).unwrap();
    Some((said, json, mods))
}

/// The line of `said` that starts with `head`, less it.
fn after<'a>(said: &'a str, head: &str) -> &'a str {
    said.lines().find_map(|l| l.strip_prefix(head)).unwrap_or_else(|| panic!("no {head:?} line:\n{said}"))
}

/// Each tile's `at` in the built world's `mod.json`, in its order.
fn placed(json: &str) -> Vec<(String, [f64; 2])> {
    json.lines()
        .filter_map(|l| {
            let tile = l.split("\"tile\": \"").nth(1)?.split('"').next()?.to_string();
            let at = l.split("\"at\": [").nth(1)?.split(']').next()?;
            let mut n = at.split(',').map(|v| v.trim().parse::<f64>().unwrap());
            Some((tile, [n.next()?, n.next()?]))
        })
        .collect()
}

const FREE: [(&str, &str); 5] =
    [("mxu_free06", "Dallas"), ("mxu_free07", "Norfolk"), ("mxu_free09", "Bridgeport"), ("mxu_free10", "Rochester"), ("mxu_free11", "Cleveland")];

/// P253: MX Unleashed: WORLD is every freeride and national map. All 27 are placed; the five
/// freerides are one cluster, so the ride starts on one with the other four drawn around it, and
/// the radar names all 27 maps. Its joins are no longer openmx-mods' world_mxu (that one has the
/// freerides along an edge), so there is nothing to compare with.
///
/// P251: its maps sit close: every crossing the seams walk makes is at most 1,800 ft of join, or
/// 2,900 ft over the wrap (a national's own ground before its interior is in that, MD61), where they
/// were up to 20,020 ft. The world wraps both ways, so "in its middle" (P253's first test, every
/// freeride clear of the outer rows) is any cluster; the unit layout puts the seam where the world
/// is shortest, which can run between two freerides.
#[test]
fn world_mxu_has_every_freeride_and_national_with_the_freerides_together() {
    let Some((said, json, mods)) = first_ride("world_mxu") else { return };
    assert!(after(&said, "world: world_mxu ").starts_with("(MX Unleashed: WORLD), 27 tiles and "), "{said}");
    let tiles = placed(&json);
    assert_eq!(tiles.len(), 27, "{json}");
    for (name, _) in FREE {
        assert!(tiles.iter().any(|(t, _)| t == name), "{name} not placed");
    }
    for n in 1..=22 {
        assert!(tiles.iter().any(|(t, _)| *t == format!("mxu_nat{n:02}")), "nat{n:02} not placed");
    }
    // Each crossing: "world crossing: x A > B[ across the seam], N ft of join, ...".
    let crossings: Vec<(&str, &str, bool, f64)> = said
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("world crossing: ")?.get(2..)?;
            let (pair, rest) = rest.split_once(", ")?;
            let (a, b) = pair.split_once(" > ")?;
            let (b, seam) = b.strip_suffix(" across the seam").map_or((b, false), |b| (b, true));
            Some((a, b, seam, rest.split(" ft").next()?.parse().ok()?))
        })
        .collect();
    assert_eq!(crossings.len(), 2 * 27, "{said}");
    for (a, b, seam, ft) in &crossings {
        let most = if *seam { 2900.0 } else { 1800.0 };
        assert!(*ft <= most, "{a} > {b}: {ft} ft of join (at most {most})");
    }
    // The freerides are one cluster: each crosses straight to another, the five joined up.
    let mut together = vec![FREE[0].0];
    while let Some(next) = crossings.iter().find_map(|(a, b, ..)| match (together.contains(a), together.contains(b)) {
        (true, false) if b.starts_with("mxu_free") => Some(*b),
        (false, true) if a.starts_with("mxu_free") => Some(*a),
        _ => None,
    }) {
        together.push(next);
    }
    assert_eq!(together.len(), 5, "the freerides are not one cluster: {together:?} of {crossings:?}");
    assert!(after(&said, "world spawn: ").starts_with("mxu_free"), "{said}");
    let radar = after(&said, "radar maps: ").strip_prefix("27: ").unwrap_or_else(|| panic!("not 27 maps:\n{said}"));
    for (name, town) in FREE {
        assert!(said.contains(&format!("world props drawn: {name} ")), "{name} not drawn:\n{said}");
        assert!(said.contains(&format!("world ground: {name} built")), "{name}'s ground not built");
        assert!(radar.split(", ").any(|n| n == town), "{town} not among the radar's maps: {radar}");
    }
    let _ = std::fs::remove_dir_all(&mods);
}

/// P253: MX Unleashed: every national is the 22 national tracks, ridden from nat01's archives.
#[test]
fn world_mxu_nat_is_the_22_nationals() {
    let Some((said, json, mods)) = first_ride("world_mxu_nat") else { return };
    assert!(after(&said, "world: world_mxu_nat ").starts_with("(MX Unleashed: every national), 22 tiles and "), "{said}");
    assert!(said.lines().any(|l| l.starts_with("world: world_mxu_nat ") && l.ends_with("lit by mxu_nat01")), "{said}");
    let tiles: Vec<String> = placed(&json).into_iter().map(|t| t.0).collect();
    assert_eq!(tiles, (1..=22).map(|n| format!("mxu_nat{n:02}")).collect::<Vec<_>>());
    assert!(after(&said, "world spawn: ").starts_with("mxu_nat"), "{said}");
    assert!(after(&said, "radar maps: ").starts_with("22: "), "{said}");
    assert!(said.lines().any(|l| l.starts_with("world seams:") && l.contains("no gaps")), "{said}");
    let _ = std::fs::remove_dir_all(&mods);
}
