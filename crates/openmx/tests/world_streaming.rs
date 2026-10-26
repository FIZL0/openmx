//! MD17: a world's ground goes to the GPU a piece at a time — the pieces in reach of the start
//! before the first frame, the rest on a thread of their own as the rider comes near, and the ones
//! left behind let go again.
//!
//! The world is written by the test: nine tiles in a three-by-three grid, 720 feet apart, and four
//! join strips through the gaps between them, all from one rolling height function in world feet
//! ([`game_sim::tiles::generate`]). The shell rides it with a reach of 200 feet to load and 300 to
//! drop (`--world-reach`), so a ride of a few hundred ticks along the first row passes several of
//! them. The rides skip without the disc, since the bike is the disc's, and without a GPU, since the
//! streamed ground is the GPU's.
//!
//! What is proved: the pieces built before the ride are exactly those in reach of the start
//! (counted); riding along the row builds each piece ahead before the rider is over it (the shell
//! waits for one it is over and says so, and it never says so) and drops those left behind; and the
//! piece under the rider at the end, streamed in mid-ride, is handed to the GPU as the same chunks
//! the whole world built before the first frame hands it (`--world-ground all`).

use game_sim::tiles::generate;
use std::path::{Path, PathBuf};
use std::process::Command;

const LOAD: f32 = 200.0;
const DROP: f32 = 300.0;
const SPACING: f32 = 9.0;
/// Where the ride starts, the first tile's own start line, facing `+x` along the first row.
const START: [f32; 2] = [200.0, 200.0];
/// Far enough along the row to leave the first tile and its join behind, short of the world's edge.
const TICKS: usize = 540;

/// Gentle rolling ground: a bike rides it on the gas without leaving it for long.
fn relief(x: f64, z: f64) -> f64 {
    40.0 + 1.5 * (x / 140.0).sin() * (z / 90.0).cos()
}

/// One piece of the world: its name, where it is, and its root cells (sixteen samples each).
struct Spec {
    name: String,
    at: [u32; 2],
    cells: (u32, u32),
    tile: bool,
}

impl Spec {
    /// Its terrain's rectangle in world feet.
    fn rect(&self) -> [f32; 4] {
        let (w, b) = ((self.cells.0 * 16) as f32 * SPACING, (self.cells.1 * 16) as f32 * SPACING);
        [self.at[0] as f32, self.at[1] as f32, self.at[0] as f32 + w, self.at[1] as f32 + b]
    }

    /// How far a point is from its terrain, the larger axis' gap.
    fn gap(&self, x: f32, z: f32) -> f32 {
        let [x0, z0, x1, z1] = self.rect();
        (x0 - x).max(x - x1).max(0.0).max((z0 - z).max(z - z1).max(0.0))
    }
}

/// The nine tiles `t<column><row>`, 64 samples (567 feet) a side, then the joins: two strips along
/// `z` through the gaps between the columns and two along `x` between the rows, each overlapping
/// the tiles on both sides (the tiles own their cells first).
fn specs() -> Vec<Spec> {
    let mut out = Vec::new();
    for row in 0..3 {
        for col in 0..3 {
            out.push(Spec { name: format!("t{col}{row}"), at: [720 * col, 720 * row], cells: (4, 4), tile: true });
        }
    }
    for (i, x) in [432, 1152].into_iter().enumerate() {
        out.push(Spec { name: format!("v{i}"), at: [x, 0], cells: (3, 15), tile: false });
    }
    for (i, z) in [432, 1152].into_iter().enumerate() {
        out.push(Spec { name: format!("h{i}"), at: [0, z], cells: (15, 3), tile: false });
    }
    out
}

fn terrain(s: &Spec) -> Vec<u8> {
    let (ox, oz) = (f64::from(s.at[0]), f64::from(s.at[1]));
    let sp = f64::from(SPACING);
    generate::terrain(&s.name, s.cells, SPACING, |x, z| relief(ox + f64::from(x) * sp, oz + f64::from(z) * sp), |_, _| 1)
}

fn scene(s: &Spec) -> String {
    [
        "[SceneInfo]",
        &format!("SceneName=MD17 {}", s.name),
        "DefaultStartPosition=200.00,40.00,200.00",
        "DefaultStartDirection=1.00,0.00,0.00",
        "[Environment]",
        &format!("TerrainFile={}.trn", s.name.to_uppercase()),
        &format!("TerrainWidth={}", s.cells.0),
        &format!("TerrainBreadth={}", s.cells.1),
        &format!("TerrainWidthScale={SPACING:.2}"),
        &format!("TerrainBreadthScale={SPACING:.2}"),
        "TerrainBorderWidth=0.00",
        "MinDivisionSize=96.00",
        "[Fog]",
        "ColorRGB=150,151,139",
        "",
    ]
    .join("\r\n")
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A `mods/` folder holding the world, `md17_world`. Each test writes its own.
fn mods_folder(test: &str) -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md17").join(test).join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let world = mods.join("tracks").join("md17_world");
    let specs = specs();
    let mut tiles = Vec::new();
    let mut joins = Vec::new();
    for s in &specs {
        if s.tile {
            let dir = world.join("tiles").join(&s.name);
            let stem = s.name.to_uppercase();
            write(
                &dir,
                "mod.json",
                format!(
                    r#"{{"standard": 1, "kind": "track", "id": "md17_{0}", "name": "MD17 {0}", "source": {{"game": "Real elevation"}}, "version": "1.0.0", "author": "crates/openmx/tests/world_streaming.rs", "permission": "public-domain", "todo": []}}"#,
                    s.name
                ),
            );
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene(s));
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), terrain(s));
            tiles.push(format!(r#"{{"tile": "{}", "at": [{}.0, {}.0], "keep": [0, 0, 63, 63]}}"#, s.name, s.at[0], s.at[1]));
        } else {
            write(&world, &format!("joins/{0}/{0}.ps2.trn", s.name), terrain(s));
            joins.push(format!(r#"{{"join": "{}", "at": [{}.0, {}.0]}}"#, s.name, s.at[0], s.at[1]));
        }
    }
    write(
        &world,
        "mod.json",
        format!(
            r#"{{
  "standard": 1, "kind": "world", "id": "md17_world", "name": "MD17 nine tiles",
  "source": {{"game": "Combined", "games": ["Real elevation"]}},
  "version": "1.0.0", "author": "crates/openmx/tests/world_streaming.rs", "permission": "public-domain",
  "tiles": [{}],
  "joins": [{}],
  "drop": [], "sky": "t00", "spawn": {{"tile": "t00"}}, "todo": []
}}"#,
            tiles.join(", "),
            joins.join(", ")
        ),
    );
    mods
}

/// Full throttle and nothing else: Cross down, at full pressure, under the original's controls.
fn throttle_tape(frames: usize) -> String {
    let mut text = format!("# game-sim pad tape: {frames} frames\nformat 1\ntape full throttle\n");
    let report = format!("0079ff{:02x}7f7f7f7f000000000000{:02x}{}", 0xbf, 0xff, "00".repeat(17));
    for _ in 0..frames {
        text += &format!("frame 0x3d08850a 0x{report} -\n");
    }
    text
}

/// The shell on the world from its start line, headless and small, with these arguments:
/// what it said, or `None` (and why) without the disc or a GPU.
fn shell(test: &str, args: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let mods = mods_folder(test);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let reach = format!("{LOAD},{DROP}");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "md17_world", "--mods", mods.to_str().unwrap()])
        .args(["--size", "320x224", "--overlay", "none", "--controls", "original"])
        .args(["--world-reach", &reach])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    if !said.lines().any(|l| l == "renderer: gpu") {
        eprintln!("skipped: no GPU, so no streamed ground");
        return None;
    }
    Some(said)
}

/// The shell ridden on the gas for [`TICKS`] ticks.
fn ride(test: &str, args: &[&str]) -> Option<String> {
    let tape = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md17").join(format!("{test}.tape"));
    write(tape.parent().unwrap(), tape.file_name().unwrap().to_str().unwrap(), throttle_tape(TICKS));
    let n = TICKS.to_string();
    let mut all = vec!["--frames", &n, "--play", tape.to_str().unwrap()];
    all.extend_from_slice(args);
    shell(test, &all)
}

/// The pieces the shell said it built before the ride, and the count its summary gave.
fn built_before(said: &str) -> (Vec<String>, usize) {
    let mut names = Vec::new();
    for l in said.lines() {
        if let Some(rest) = l.strip_prefix("world ground: ") {
            if let Some((n, _)) = rest.split_once(" of ") {
                if rest.contains("built before the ride") {
                    return (names, n.parse().unwrap());
                }
            }
            if let Some((name, tail)) = rest.split_once(" built, ") {
                assert!(!tail.contains(" at tick "), "a piece built mid-ride before the summary: {l}");
                names.push(name.to_string());
            }
        }
    }
    panic!("no `built before the ride` line:\n{said}")
}

/// Every `world ground: <piece> <what> ... at tick N` line: the piece, what, and the tick.
fn during(said: &str) -> Vec<(String, String, usize)> {
    said.lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("world ground: ")?;
            let (head, tick) = rest.rsplit_once(" at tick ")?;
            let mut w = head.split_whitespace();
            let (first, second) = (w.next()?, w.next()?);
            let (piece, what) = if first == "waited" { (head.split(" for ").nth(1)?, "waited") } else { (first, second.trim_end_matches(',')) };
            Some((piece.to_string(), what.to_string(), tick.parse().ok()?))
        })
        .collect()
}

fn end_x(said: &str) -> f32 {
    let l = said.lines().find(|l| l.starts_with("end: ")).unwrap_or_else(|| panic!("no end line:\n{said}"));
    l["end: ".len()..].split(',').next().unwrap().trim().parse().unwrap()
}

#[test]
fn only_the_pieces_in_reach_of_the_start_are_built_before_the_ride() {
    let Some(said) = shell("start", &["--frames", "1"]) else { return };
    let mut want: Vec<String> = specs().into_iter().filter(|s| s.gap(START[0], START[1]) <= LOAD).map(|s| s.name).collect();
    let (mut got, n) = built_before(&said);
    want.sort();
    got.sort();
    assert_eq!(got, want, "the pieces built before the first frame");
    assert_eq!(n, want.len());
    assert!(n < specs().len() / 2, "most of the world waits: {n} of {}", specs().len());
    assert!(said.contains(&format!("world ground: {n} of {} pieces built before the ride", specs().len())), "{said}");
}

#[test]
fn riding_along_the_row_builds_ahead_and_drops_behind() {
    let Some(said) = ride("ride", &[]) else { return };
    let x = end_x(&said);
    // Far enough to have left the first tile and its join behind: past both their drop distances.
    let specs = specs();
    let by = |n: &str| specs.iter().find(|s| s.name == n).unwrap();
    assert!(x > by("v0").rect()[2] + DROP, "the ride ended at x {x}, not past v0's drop distance:\n{said}");
    let events = during(&said);
    let waited: Vec<_> = events.iter().filter(|(_, w, _)| w == "waited").collect();
    assert!(waited.is_empty(), "the rider got onto a piece before its ground was built: {waited:?}");
    // The ride runs along `z` = START's: every piece that line comes within reach of and was not
    // built before the ride is built during it, and no other piece is built at all.
    let (before, _) = built_before(&said);
    for s in &specs {
        let nearest = s.rect()[0].clamp(START[0], x);
        let reached = s.gap(nearest, START[1]) <= LOAD;
        let built = events.iter().any(|(p, w, t)| p == &s.name && w == "built" && *t > 0);
        assert_eq!(built || before.contains(&s.name), reached, "{}: built, and in reach of the ride's line:\n{said}", s.name);
    }
    for gone in ["t00", "v0"] {
        assert!(events.iter().any(|(p, w, _)| p == gone && w == "dropped"), "{gone} was left behind and not dropped:\n{said}");
    }
    let dropped_ahead = events.iter().filter(|(p, w, _)| w == "dropped" && by(p).gap(x, START[1]) <= DROP).count();
    assert_eq!(dropped_ahead, 0, "a piece within drop reach of the end was dropped:\n{said}");
}

#[test]
fn the_streamed_ground_under_the_rider_is_the_whole_builds() {
    let Some(near) = ride("near", &["--world-ground", "near"]) else { return };
    let Some(all) = ride("all", &["--world-ground", "all"]) else { return };
    let under = |said: &str| said.lines().find(|l| l.starts_with("world ground under the rider: ")).unwrap_or_else(|| panic!("no ground line:\n{said}")).to_string();
    let (n, a) = (under(&near), under(&all));
    assert!(!n.ends_with("not held"), "{n}");
    assert_eq!(n, a, "the piece under the rider, streamed and whole");
    // The piece was one the stream built mid-ride, not one it had before the first frame.
    let piece = n["world ground under the rider: ".len()..].split_whitespace().next().unwrap();
    assert!(during(&near).iter().any(|(p, w, t)| p == piece && w == "built" && *t > 0), "{piece} was not built mid-ride:\n{near}");
    let (_, every) = built_before(&all);
    assert_eq!(every, specs().len(), "--world-ground all builds every piece first");
    assert_eq!(end_x(&near), end_x(&all), "the ground the GPU holds does not change the ride");
}

/// MD28: the most ground chunks one frame of the ride hands the GPU. A world's piece is written to
/// the GPU on the main thread, and a big one in one frame was a frame of 5-10 ms; so a piece built
/// mid-ride goes over a few frames, this many chunks a frame (`--world-chunks`, small here so the
/// test's small pieces are split as a national's are under the shell's own budget).
const CHUNKS_A_FRAME: usize = 8;

#[test]
fn a_streamed_ride_writes_its_ground_a_few_chunks_a_frame() {
    let n = CHUNKS_A_FRAME.to_string();
    let Some(said) = ride("chunks", &["--world-chunks", &n]) else { return };
    let line = said.lines().find(|l| l.starts_with("world ground: at most ")).unwrap_or_else(|| panic!("no chunks line:\n{said}"));
    let words: Vec<&str> = line.split_whitespace().collect();
    let (most, all, frames): (usize, usize, usize) = (words[4].parse().unwrap(), words[13].trim_start_matches('(').parse().unwrap(), words[15].parse().unwrap());
    assert!(line.ends_with(&format!("budget {CHUNKS_A_FRAME})")), "{line}");
    assert!(most <= CHUNKS_A_FRAME, "a frame wrote {most} chunks, over the budget of {CHUNKS_A_FRAME}: {line}");
    // The budget did bite: the pieces built mid-ride took more frames than there were pieces, the
    // joins (thirteen chunks) two each.
    let events = during(&said);
    let built = events.iter().filter(|(_, w, t)| w == "built" && *t > 0).count();
    assert!(built >= 2 && frames > built && frames * CHUNKS_A_FRAME >= all, "{built} pieces built mid-ride: {line}\n{said}");
    let waited: Vec<_> = events.into_iter().filter(|(_, w, _)| w == "waited").collect();
    assert!(waited.is_empty(), "the rider got onto a piece before its ground was built: {waited:?}");
    // And the ride is the same ride, however its ground went up.
    let Some(whole) = ride("chunks_whole", &[]) else { return };
    assert_eq!(end_x(&said), end_x(&whole), "the chunks a frame do not change the ride");
}

/// MD23: `--world-sweep` streams the ground about a point going across the world over the ride
/// instead of about the rider, which is how the ultra world's 116,000 feet were crossed in a few
/// minutes. Here it goes corner to corner: every piece the line comes within reach of is built, the
/// first corner's are let go, the GPU never holds the whole world, and the ride is the same ride.
#[test]
fn a_sweep_streams_the_ground_along_its_line_and_leaves_the_ride_alone() {
    const TO: [f32; 2] = [1800.0, 1800.0];
    let sweep = format!("{},{},{},{}", START[0], START[1], TO[0], TO[1]);
    let Some(said) = ride("sweep", &["--world-sweep", &sweep]) else { return };
    let at = |tick: usize| {
        let t = tick as f32 / TICKS as f32;
        [START[0] + (TO[0] - START[0]) * t, START[1] + (TO[1] - START[1]) * t]
    };
    let (before, _) = built_before(&said);
    let events = during(&said);
    for s in specs() {
        let reached = (0..=TICKS).any(|k| {
            let [x, z] = at(k);
            s.gap(x, z) <= LOAD
        });
        let built = before.contains(&s.name) || events.iter().any(|(p, w, t)| p == &s.name && w == "built" && *t > 0);
        assert_eq!(built, reached, "{}: built, and in reach of the sweep's line:\n{said}", s.name);
    }
    for gone in ["t00", "v0", "h0"] {
        assert!(events.iter().any(|(p, w, _)| p == gone && w == "dropped"), "{gone} was left behind and not dropped:\n{said}");
    }
    let line = said.lines().find(|l| l.starts_with("world ground: held at most ")).unwrap_or_else(|| panic!("no held line:\n{said}"));
    let held: usize = line.split_whitespace().nth(5).unwrap().parse().unwrap();
    let Some(all) = ride("sweep_all", &["--world-ground", "all", "--frames", "1"]) else { return };
    let whole: usize = all
        .lines()
        .filter_map(|l| l.strip_prefix("world ground: ")?.split_once(" built, ")?.1.strip_suffix(" chunks")?.parse::<usize>().ok())
        .sum();
    assert!(held > 0 && held < whole, "held {held} chunks at once of the world's {whole}: {line}");
    let Some(rider) = ride("sweep_rider", &[]) else { return };
    let hash = |s: &str| s.lines().find(|l| l.starts_with("ride hash ")).map(str::to_string);
    assert_eq!(hash(&said), hash(&rider), "the sweep does not change the ride");
}
