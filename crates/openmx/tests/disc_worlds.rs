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

/// The most CPU time (user and system, over every thread) a first ride on a world off the disc may
/// spend, its joins built from cold (P260): the user's MX WORLD that "is not loading". K24: the limits
/// read the shell's CPU time, not the wall clock, which a busy gate stretches without the shell doing
/// any more (world_mxu_nat 518 s of wall clock against 480 at load ~48, P226). In the test build at load
/// 25-55 (K24, 11-04) world_mxu spent 1379-1475 s of CPU (178-296 s of wall clock, its stitcher on every
/// core) and world_mxu_nat 801-843 s. P299: each starts where its last pass began
/// (`crates/stitch/worlds/<id>.start.json`) and builds in that pass alone: world_mxu 1312 s of CPU in a
/// release build at load ~60 from cold, 208 s from its start, world_mxu_nat 729 s and 291 s.
const COLD: Duration = Duration::from_secs(900);
/// The most CPU time a ride on a built or installed world may spend, its joins read back and nothing
/// built, with `--seams` walking every piece's edges: world_mxu_free 29-52 s (K24).
const WARM: Duration = Duration::from_secs(240);
/// The most CPU time a plain ride on an installed world may spend to start (P260): its 27 tiles and
/// every join read. Installed world_mxu spent 43 s (K24) where the wall clock read 15-79 s.
const INSTALLED: Duration = Duration::from_secs(120);

/// [`shell`], held to `most` of the shell's CPU time.
fn shell_within(mods: &Path, track: &str, most: Duration) -> String {
    let (said, cpu) = shell_cpu(mods, track, &["--seams"]);
    assert!(cpu <= most, "{track} spent {:.0} s of CPU to ride, at most {:.0}", cpu.as_secs_f32(), most.as_secs_f32());
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
    shell_cpu(mods, track, more).0
}

/// The shell riding `track` off `mods` for two frames: what it said, and the CPU time it spent (user
/// and system over all its threads, from `wait4`'s rusage of that one child, so the other tests'
/// shells running beside it are not counted). The wall clock is printed beside it, not held to.
fn shell_cpu(mods: &Path, track: &str, more: &[&str]) -> (String, Duration) {
    use std::io::Read;
    use std::process::Stdio;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let at = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(more)
        .args(["--headless", "--no-sound", "--frames", "2", "--track", track, "--mods"])
        .arg(mods)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the shell runs");
    let (mut out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let reader = std::thread::spawn(move || {
        let mut s = Vec::new();
        out.read_to_end(&mut s).unwrap();
        s
    });
    let mut e = Vec::new();
    err.read_to_end(&mut e).unwrap();
    let o = reader.join().unwrap();
    let (mut status, mut usage) = (0, unsafe { std::mem::zeroed::<libc::rusage>() });
    let pid = child.id() as libc::pid_t;
    let reaped = loop {
        let r = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
        if r == -1 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        break r;
    };
    assert_eq!(reaped, pid, "wait4 on the shell: {}", std::io::Error::last_os_error());
    let tv = |t: libc::timeval| Duration::new(t.tv_sec as u64, t.tv_usec as u32 * 1000);
    let cpu = tv(usage.ru_utime) + tv(usage.ru_stime);
    let said = format!("{}{}", String::from_utf8_lossy(&o), String::from_utf8_lossy(&e));
    eprintln!("{track} from {}: {:.1} s of CPU, {:.1} s of wall clock", mods.display(), cpu.as_secs_f32(), at.elapsed().as_secs_f32());
    assert!(libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0, "the shell failed ({status:#x}):\n{said}");
    (said, cpu)
}

/// The ride's lines that say what world `id` is: its size and pieces, its wrap, holes and seams.
fn world_lines(said: &str, id: &str) -> Vec<String> {
    let head = format!("world: {id} (");
    let keep = [head.as_str(), "world wraps:", "world holes:", "world seams:", "world spawn:"];
    said.lines().filter(|l| keep.iter().any(|k| l.starts_with(k))).map(str::to_string).collect()
}

/// P299: the first ride on `id` built its joins in one pass, from the start its spec's
/// `<id>.start.json` keeps for this disc and stitcher, and the start the build kept in `built` is
/// that file's. A stitcher that builds other joins (a new `stitch::VERSION`) or ends at another start
/// passes from cold, minutes on a phone: copy the kept one over the file.
fn started_where_it_ended(said: &str, id: &str, built: &Path) {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../stitch/worlds").join(format!("{id}.start.json"));
    let kept = built.join("start.json");
    let fix = format!("copy {} to {}", kept.display(), file.display());
    assert!(!said.contains(&format!("{id}: no start kept")), "{id} built from cold: its start file is stale ({fix})\n{said}");
    assert!(said.contains(&format!("{id}: joins built in 1 passes")), "{id} took more than its last pass ({fix}):\n{said}");
    assert_eq!(std::fs::read_to_string(&kept).unwrap(), std::fs::read_to_string(&file).unwrap(), "{id}'s start ({fix})");
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

fn rides_off_the_disc(id: &str, tiles: usize, joins: usize) -> Option<String> {
    if !disc() {
        return None;
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
    started_where_it_ended(&first, id, built);

    // The second reads it back.
    let second = shell_within(&mods, id, WARM);
    assert!(!second.contains("building its joins"), "{second}");
    assert_eq!(world_lines(&second, id), lines);

    // It is openmx-mods' world, and riding that one installed is the same ride.
    let Some(installed) = reference().filter(|r| r.join("tracks").join(id).join("mod.json").is_file()) else {
        eprintln!("skipped the comparison: no {id} in $OPENMX_WORLDS or the main checkout's mods/");
        return Some(first);
    };
    let want = installed.join("tracks").join(id);
    // The installed copy is the user's, rebuilt by hand, so it lags a stitcher or spec that lays the
    // tiles elsewhere (MD61's first line over the seam, MD95's units): that is a reinstall to do, not
    // this build gone wrong. Laid out alike, every byte must agree.
    let tiles_of = |dir: &Path| {
        let json = std::fs::read_to_string(dir.join("mod.json")).unwrap();
        json.lines().filter(|l| l.trim_start().starts_with("{\"tile\":") && l.contains("\"at\":")).map(str::to_string).collect::<Vec<_>>()
    };
    if tiles_of(built) != tiles_of(&want) {
        eprintln!("skipped the comparison: the installed {id} lays its tiles out otherwise (another stitcher or spec); reinstall it from {}", built.display());
        return Some(first);
    }
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
    Some(first)
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
        let (said, cpu) = shell_cpu(&installed, id, &[]);
        assert!(cpu <= INSTALLED, "the installed {id} spent {:.0} s of CPU to ride, at most {:.0}", cpu.as_secs_f32(), INSTALLED.as_secs_f32());
        assert!(!said.contains("building its joins"), "the installed world is ridden:\n{said}");
        assert!(said.lines().any(|l| l.starts_with(&format!("world: {id} ("))), "{said}");
        assert!(said.lines().any(|l| l.starts_with("ridden for")), "{said}");
    }
}

/// MD95: the five freeride maps are one row round the wrap, so no cell of the world is empty. The
/// 3 x 2 grid left one, a map's width of join north of Dallas and west of Cleveland, which the user
/// rode as "Cleveland flattened": Bridgeport crossed 6,606 ft of it to Cleveland over the wrap.
#[test]
fn world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it() {
    let Some(said) = rides_off_the_disc("world_mxu_free", 5, 36 + 16) else { return };
    assert!(after(&said, "world: world_mxu_free ").contains(", 5 tiles and 52 joins, 28503 x 6462 feet,"), "{said}");
    let crossings = crossings(&said);
    assert_eq!(crossings.len(), 5, "each map crosses along the row to the next: {crossings:?}");
    for (a, b, seam, ft) in &crossings {
        let most = if *seam { 2900.0 } else { 1800.0 };
        assert!(*ft <= most, "{a} > {b}: {ft} ft of join (at most {most})");
    }
}

/// Each "world crossing: x A > B[ across the seam], N ft of join, ..." line of `said`: A, B, whether
/// it crosses the wrap, and the feet of join.
fn crossings(said: &str) -> Vec<(&str, &str, bool, f64)> {
    said.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("world crossing: ")?.get(2..)?;
            let (pair, rest) = rest.split_once(", ")?;
            let (a, b) = pair.split_once(" > ")?;
            let (b, seam) = b.strip_suffix(" across the seam").map_or((b, false), |b| (b, true));
            Some((a, b, seam, rest.split(" ft").next()?.parse().ok()?))
        })
        .collect()
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
    started_where_it_ended(&said, id, &keys[0]);
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
