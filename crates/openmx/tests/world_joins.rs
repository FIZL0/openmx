//! MD27: a world (MD8) throws a rider only at its outer edge, and its maps meet ridge to ridge.
//!
//! What can throw inside a world is the edge test's second half, [`game_sim::world::World::off_world`]:
//! a hole — ground no tile and no join owns — within `off_map::MARGIN` of the bike. A tile's own edge
//! is not asked (the world's rectangle is), and the original places nothing that throws: its
//! shootoff is the margin test alone. So the proof is in three parts.
//!
//! * [`Tiled::holes`] finds every hole exactly, including the few-feet strip a grid of points steps
//!   over (the one openmx-mods C11 found between joins), and finds none where the pieces overlap.
//! * A generated world of two tiles, each with the ring of mountains an MX Unleashed freeride map has
//!   at its edge — a ridge, and outside it a plateau out to the map's own edge — is laid as a ridge
//!   cut wants it (openmx-mods M23): each tile's `keep` ends at its ridge, and a join holds the ridge
//!   top between them. A ride across it either way is on the ground and thrown only off the far
//!   outer edge. Tile `a` places a collision shape on its own plateau, outside its keep and on the
//!   join's ground, square across the ride's line: the world leaves it out, so it is no invisible
//!   wall. The rides skip without the disc, since the bike and the shape are the disc's.
//! * The same on world_mxu_free as openmx-mods' stitch builds it off the user's disc: no holes, a ride
//!   across every join between two tiles never thrown, a ride off each outer side thrown. That world
//!   is not in this repository or its context, so that test is ignored; it reads
//!   `$OPENMX_WORLDS/tracks/world_mxu_free` (a `mods/` folder), and the journal entry has its run.

use game_sim::tiles::{generate, Piece, Reach, Source, Tile, Tiled};
use game_sim::track::TerrainLayout;
use ps2_float::F;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The shootoff's margin, `game_sim::off_map::MARGIN`.
const MARGIN: f32 = 150.0;

/// Feet a sample, every piece.
const SPACING: f32 = 9.0;

/// Where the two ridges are, in world `x`: `a`'s keep ends on the first and `b`'s begins on the
/// second, with the join's flat ridge top between.
const RIDGE_A: f32 = 432.0;
const RIDGE_B: f32 = 576.0;

/// The world's far corner: `b`'s last kept sample, and every piece's.
const WIDTH: f32 = 432.0 + 63.0 * SPACING;
const DEPTH: f32 = 63.0 * SPACING;

/// The ground the world should ride on, in world feet: a valley floor at 40 inside each tile rising
/// 20 feet over 108 to its ridge, and the ridge's top at 60 between the two.
fn ridden(x: f64) -> f64 {
    let (a, b) = (f64::from(RIDGE_A), f64::from(RIDGE_B));
    if x <= a {
        40.0 + 20.0 * (1.0 - (a - x) / 108.0).max(0.0)
    } else if x < b {
        60.0
    } else {
        40.0 + 20.0 * (1.0 - (x - b) / 108.0).max(0.0)
    }
}

/// A tile's own ground at world `x`, its ridge at `ridge`, and `outward` +1 if its plateau is past
/// it in `+x`: the ridden side as above, the other falling from the ridge to a plateau at 50, as an
/// MX Unleashed map's shootoff ground lies outside its mountains.
fn tile_ground(x: f64, ridge: f64, outward: f64) -> f64 {
    let past = (x - ridge) * outward;
    if past <= 0.0 { 40.0 + 20.0 * (1.0 - -past / 108.0).max(0.0) } else { 50.0 + 10.0 * (1.0 - past / 54.0).max(0.0) }
}

struct Spec {
    name: &'static str,
    stem: &'static str,
    at: u32,
    cells: (u32, u32),
    keep: Option<[u32; 4]>,
    height: fn(f64) -> f64,
}

/// `a` at the origin keeps to its ridge, sample 48; the join `ab` covers the ridge top and on under
/// `b`; `b` at 432 keeps from its ridge, its sample 16, to the world's far edge.
const SPECS: [Spec; 3] = [
    Spec { name: "a", stem: "MRA", at: 0, cells: (4, 4), keep: Some([0, 0, 48, 63]), height: |x| tile_ground(x, 432.0, 1.0) },
    Spec { name: "b", stem: "MRB", at: 432, cells: (4, 4), keep: Some([16, 0, 63, 63]), height: |x| tile_ground(x, 576.0, -1.0) },
    Spec { name: "ab", stem: "ab", at: 432, cells: (2, 4), keep: None, height: ridden },
];

/// Where `a` stands its shape: on its plateau, past its ridge and its keep, on the ride's line.
const WALL: [f32; 2] = [500.0, 283.0];

fn terrain(s: &Spec) -> Vec<u8> {
    let at = f64::from(s.at);
    generate::terrain(s.name, s.cells, SPACING, |x, _| (s.height)(at + f64::from(x) * f64::from(SPACING)), |_, _| 1)
}

/// A tile's scene: its terrain and a start line; `a`'s places the disc's shipping container at
/// [`WALL`], across the line.
fn scene(s: &Spec) -> String {
    let mut lines = vec![
        "[SceneInfo]".to_string(),
        format!("SceneName=MD27 {}", s.name),
        "DefaultStartPosition=200.00,40.00,283.00".into(),
        "DefaultStartDirection=1.00,0.00,0.00".into(),
        "[Environment]".into(),
        format!("TerrainFile={}.trn", s.stem),
        format!("TerrainWidth={}", s.cells.0),
        format!("TerrainBreadth={}", s.cells.1),
        format!("TerrainWidthScale={SPACING:.2}"),
        format!("TerrainBreadthScale={SPACING:.2}"),
        "TerrainBorderWidth=0.00".into(),
        "MinDivisionSize=96.00".into(),
        "[Fog]".into(),
        "ColorRGB=150,151,139".into(),
    ];
    if s.name == "a" {
        // On the join's ground, which the world rides there, not buried in the plateau under it.
        let y = ridden(f64::from(WALL[0]));
        lines.extend([
            "[CollisionObjects]".to_string(),
            "NumberOfCollisionObjects=1".into(),
            "[Object1]".into(),
            "COL=free06_container.col".into(),
            format!("Position={:.2},{y:.2},{:.2}", WALL[0], WALL[1]),
            "LookVector=1.000000,0.000000,0.000000".into(),
        ]);
    }
    lines.push(String::new());
    lines.join("\r\n")
}

fn track_json(id: &str) -> String {
    format!(
        r#"{{
  "standard": 1, "kind": "track", "id": "{id}", "name": "MD27 {id}",
  "source": {{"game": "Real elevation", "data": "generated by openmx's MD27 test", "date": "2026-10-23"}},
  "version": "1.0.0", "author": "crates/openmx/tests/world_joins.rs", "permission": "public-domain", "todo": []
}}"#
    )
}

fn world_json() -> String {
    let tiles: Vec<String> =
        SPECS[..2].iter().map(|s| format!(r#"{{"tile": "{}", "at": [{}.0, 0.0], "keep": {:?}}}"#, s.name, s.at, s.keep.unwrap())).collect();
    format!(
        r#"{{
  "standard": 1, "kind": "world", "id": "md27_world", "name": "MD27 ridge to ridge",
  "source": {{"game": "Combined", "games": ["Real elevation"]}},
  "version": "1.0.0", "author": "crates/openmx/tests/world_joins.rs", "permission": "public-domain",
  "tiles": [{}],
  "joins": [{{"join": "ab", "at": [432.0, 0.0], "between": ["a", "b"]}}],
  "drop": [], "sky": "a", "spawn": {{"tile": "a"}}, "todo": []
}}"#,
        tiles.join(", ")
    )
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// The disc's shipping container, a collision shape a bike cannot ride through.
fn container() -> Option<Vec<u8>> {
    let mut disc = match game_render::disc::Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipped: {e}");
            return None;
        }
    };
    let name = disc.entries().iter().find(|e| e.name.to_ascii_lowercase().ends_with("free06.res")).expect("free06.res").name.clone();
    let res = disc.res(&name).expect("free06.res");
    Some(res.read_suffix("free06_container.col").expect("free06_container.col"))
}

/// A `mods/` folder holding the world, `md27_world`. Each test writes its own.
fn mods_folder(test: &str, container: &[u8]) -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md27").join(test).join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let world = mods.join("tracks").join("md27_world");
    write(&world, "mod.json", world_json());
    for s in &SPECS[..2] {
        let dir = world.join("tiles").join(s.name);
        write(&dir, "mod.json", track_json(&format!("md27_{}", s.name)));
        write(&dir, &format!("Env/{0}/{0}.scn", s.stem), scene(s));
        write(&dir, &format!("Env/{0}/{0}.ps2.trn", s.stem), terrain(s));
        write(&dir, "Col/free06_container.col", container);
    }
    write(&world, "joins/ab/ab.ps2.trn", terrain(&SPECS[2]));
    mods
}

/// The shell's output for a world in a `mods/` folder.
fn shell(mods: &Path, track: &str, args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", track, "--mods", mods.to_str().unwrap()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    said
}

/// One traced tick.
struct Tick {
    frame: usize,
    x: f32,
    y: f32,
    z: f32,
    wreck: bool,
}

/// A ride on the gas from `(x, z)` facing `degrees` (90 along `+x`): its ticks up to its first
/// wreck, and where the edge threw it if that wreck was the throw.
fn ride(mods: &Path, track: &str, x: f32, z: f32, degrees: f32, frames: usize) -> (Vec<Tick>, Option<(usize, f32, f32)>) {
    let at = format!("{x},{z},{degrees}");
    let said = shell(mods, track, &["--trace", &frames.to_string(), "--trace-gas", "--trace-feel", "--trace-at", &at]);
    let mut ticks = Vec::new();
    let mut thrown = None;
    for l in said.lines() {
        if let Some(rest) = l.strip_prefix("thrown: tick ") {
            let w: Vec<f32> = rest.replace(" at", "").replace(',', "").split_whitespace().map(|w| w.parse().unwrap()).collect();
            thrown.get_or_insert((w[0] as usize, w[1], w[2]));
            continue;
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        let Some(frame) = w.first().and_then(|f| f.parse::<usize>().ok()) else { continue };
        if w.len() != 12 || ticks.last().is_some_and(|t: &Tick| t.wreck) {
            continue;
        }
        let f = |i: usize| w[i].parse::<f32>().unwrap();
        ticks.push(Tick { frame, x: f(5), y: f(6), z: f(7), wreck: w[11] == "1" });
    }
    assert!(!ticks.is_empty(), "no trace rows:\n{said}");
    (ticks, thrown)
}

/// A source for [`Tiled`] that is never asked: holes are a matter of who owns what.
struct Unread;

impl Source for Unread {
    fn load(&mut self, _: usize) -> Result<Piece, String> {
        Err("not read".into())
    }
}

fn tile(s: &Spec, cells: (u32, u32)) -> Tile {
    let layout = TerrainLayout { width: cells.0, breadth: cells.1, width_scale: F(SPACING), breadth_scale: F(SPACING), ..TerrainLayout::default() };
    let mut t = Tile { name: s.name.into(), at: [F(s.at as f32), F::ZERO], layout, spacing: F(SPACING), levels: 1, keep: [0; 4], first_owner: 0 };
    t.keep = s.keep.unwrap_or_else(|| t.every_cell());
    t
}

/// The world as laid has no hole; a join one root cell wide in its place leaves the strip its last
/// cell is (openmx-mods C11's), 9 feet across, and [`Tiled::holes`] finds it exactly.
#[test]
fn holes_are_found_exactly_and_the_ridge_world_has_none() {
    let pieces = |join_cells: (u32, u32)| SPECS.iter().map(|s| tile(s, if s.keep.is_none() { join_cells } else { s.cells })).collect::<Vec<_>>();
    let laid = Tiled::new(pieces((2, 4)), Box::new(Unread), Reach::DESKTOP).unwrap();
    assert_eq!((laid.area.width.0, laid.area.depth.0), (WIDTH, DEPTH));
    assert_eq!(laid.holes(), Vec::<[F; 4]>::new());
    let short = Tiled::new(pieces((1, 4)), Box::new(Unread), Reach::DESKTOP).unwrap();
    let holes: Vec<[f32; 4]> = short.holes().iter().map(|h| h.map(|v| v.0)).collect();
    assert_eq!(holes, vec![[567.0, 0.0, 576.0, DEPTH]]);
    // A grid of points every 20 feet steps over it, which is why the scan is not one.
    assert!((0..50).all(|i| !short.hole(F(i as f32 * 20.0), F(283.0))));
}

/// The world loads with no hole to throw from and without `a`'s shape on the join's ground.
#[test]
fn the_ridge_world_loads_with_no_holes_and_without_the_cut_shape() {
    let Some(col) = container() else { return };
    let mods = mods_folder("loads", &col);
    let said = shell(&mods, "md27_world", &["--trace", "1", "--trace-feel"]);
    let line = |p: &str| said.lines().find(|l| l.starts_with(p)).unwrap_or_else(|| panic!("no `{p}` line:\n{said}")).to_string();
    assert_eq!(line("world holes:"), "world holes: none, so only the outer edge throws");
    assert!(line("world: md27_world").contains(&format!("{WIDTH:.0} x {DEPTH:.0} feet")), "{said}");
}

/// Across the ridge from `a` into `b` and on: on the ground all the way, through the place `a`'s
/// shape stood, and thrown only at the world's far edge.
#[test]
fn a_ride_across_the_ridge_is_thrown_only_off_the_outer_edge() {
    let Some(col) = container() else { return };
    let mods = mods_folder("east", &col);
    for (from, degrees, edge) in [(200.0, 90.0, WIDTH - MARGIN), (WIDTH - 200.0, 270.0, MARGIN)] {
        let (ticks, thrown) = ride(&mods, "md27_world", from, WALL[1], degrees, 900);
        let last = ticks.last().unwrap();
        let Some((tick, x, _)) = thrown else {
            panic!("from x {from}: never thrown; first wreck {} on tick {} at ({}, {})", last.wreck, last.frame, last.x, last.z)
        };
        assert_eq!(tick, last.frame, "from x {from}: wrecked on tick {} at ({}, {}) before the throw", last.frame, last.x, last.z);
        assert!((x - edge).abs() < 12.0, "from x {from}: thrown at x {x}, the edge's margin is at {edge}");
        let riding = &ticks[..ticks.len() - 1];
        let on = riding.iter().filter(|t| t.x > RIDGE_A && t.x < RIDGE_B).count();
        assert!(on > 3, "from x {from}: only {on} ticks on the ridge top");
        assert!(riding.iter().any(|t| (t.x - WALL[0]).abs() < 5.0), "from x {from}: never at the cut shape's place");
        for t in riding {
            let ground = ridden(f64::from(t.x)) as f32;
            assert!(t.y > ground - 6.0, "from x {from}, tick {}: at y {} under the ground at {ground} ({}, {})", t.frame, t.y, t.x, t.z);
        }
        println!("from x {from}: thrown on tick {tick} at x {x:.1}");
    }
}

// ---- world_mxu_free as openmx-mods' stitch builds it off the user's disc ----

/// Ridden from 150 feet inside one tile's keep across to the next, and off each outer side.
#[test]
#[ignore = "needs world_mxu_free from openmx-mods' stitch (C11 or later) in $OPENMX_WORLDS/tracks, and the disc"]
fn world_mxu_free_throws_only_off_its_outer_edge() {
    let mods = PathBuf::from(std::env::var_os("OPENMX_WORLDS").expect("OPENMX_WORLDS: a mods/ folder with tracks/world_mxu_free"));
    let said = shell(&mods, "world_mxu_free", &["--trace", "1", "--trace-feel"]);
    let line = |p: &str| said.lines().find(|l| l.starts_with(p)).unwrap_or_else(|| panic!("no `{p}` line:\n{said}")).to_string();
    assert_eq!(line("world holes:"), "world holes: none, so only the outer edge throws");
    let size: Vec<f32> = line("world: world_mxu_free").split(", ").nth(2).unwrap().split(' ').filter_map(|w| w.parse().ok()).collect();
    let (width, depth) = (size[0], size[1]);
    // The tiles' kept rectangles, from the world's file.
    let json = std::fs::read_to_string(mods.join("tracks/world_mxu_free/mod.json")).unwrap();
    let kept: Vec<(String, [f32; 4])> = json
        .lines()
        .filter(|l| l.contains("\"tile\"") && l.contains("\"keep\""))
        .map(|l| {
            let n: Vec<f32> = l.split(|c: char| !(c.is_ascii_digit() || c == '-')).filter_map(|w| w.parse().ok()).collect();
            let name = l.split('"').nth(3).unwrap().to_string();
            let (at, k) = ([n[n.len() - 6], n[n.len() - 5]], &n[n.len() - 4..]);
            (name, [at[0] + k[0] * 9.0, at[1] + k[1] * 9.0, at[0] + k[2] * 9.0, at[1] + k[3] * 9.0])
        })
        .collect();
    // Every two tiles facing each other across a join, along x or z: a ride from 150 feet inside the
    // one's edge to the other, both ways.
    let mut crossings = Vec::new();
    for (a, ka) in &kept {
        for (b, kb) in &kept {
            let z = (ka[1].max(kb[1]) + ka[3].min(kb[3])) / 2.0;
            let x = (ka[0].max(kb[0]) + ka[2].min(kb[2])) / 2.0;
            if kb[0] >= ka[2] && kb[0] - ka[2] < 3000.0 && ka[1].max(kb[1]) < ka[3].min(kb[3]) {
                crossings.push((format!("{a} -> {b}"), ka[2] - 150.0, z, 90.0, kb[0] + 150.0));
                crossings.push((format!("{b} -> {a}"), kb[0] + 150.0, z, 270.0, ka[2] - 150.0));
            }
            if kb[1] >= ka[3] && kb[1] - ka[3] < 3000.0 && ka[0].max(kb[0]) < ka[2].min(kb[2]) {
                crossings.push((format!("{a} -> {b}"), x, ka[3] - 150.0, 0.0, kb[1] + 150.0));
                crossings.push((format!("{b} -> {a}"), x, kb[1] + 150.0, 180.0, ka[3] - 150.0));
            }
        }
    }
    assert!(crossings.len() >= 8, "{} crossings", crossings.len());
    let mut report = Vec::new();
    for (what, x, z, degrees, goal) in &crossings {
        let (ticks, thrown) = ride(&mods, "world_mxu_free", *x, *z, *degrees, 1200);
        let last = ticks.last().unwrap();
        let along = |t: &Tick| if *degrees == 90.0 || *degrees == 270.0 { t.x } else { t.z };
        let reached = ticks.iter().any(|t| if *degrees == 90.0 || *degrees == 0.0 { along(t) >= *goal } else { along(t) <= *goal });
        if let Some((tick, tx, tz)) = thrown {
            let off = tx < MARGIN + 12.0 || tz < MARGIN + 12.0 || tx > width - MARGIN - 12.0 || tz > depth - MARGIN - 12.0;
            assert!(off, "{what}: thrown on tick {tick} at ({tx}, {tz}), inside the world");
        }
        report.push(format!("{what}: {} ({} ticks, ended at {:.0}, {:.0})", if reached { "reached" } else if last.wreck { "wrecked" } else { "short" }, ticks.len(), last.x, last.z));
    }
    // Off each outer side, from 400 feet inside it, on a tile's ground.
    let (cx, cz) = (width / 2.0, depth / 2.0);
    let mut sides = 0;
    for (x, z, degrees) in [(400.0, cz, 270.0), (width - 400.0, cz, 90.0), (cx, 400.0, 180.0), (cx, depth - 400.0, 0.0)] {
        let (ticks, thrown) = ride(&mods, "world_mxu_free", x, z, degrees, 900);
        let last = ticks.last().unwrap();
        match thrown {
            Some((tick, tx, tz)) => {
                let off = tx < MARGIN + 12.0 || tz < MARGIN + 12.0 || tx > width - MARGIN - 12.0 || tz > depth - MARGIN - 12.0;
                assert!(off, "off the side from ({x}, {z}): thrown on tick {tick} at ({tx}, {tz}), inside");
                sides += 1;
                report.push(format!("off the side from ({x:.0}, {z:.0}) at {degrees}: thrown on tick {tick} at ({tx:.0}, {tz:.0})"));
            }
            None => report.push(format!("off the side from ({x:.0}, {z:.0}) at {degrees}: not thrown, ended at ({:.0}, {:.0}) wreck {}", last.x, last.z, last.wreck)),
        }
    }
    println!("{}", report.join("\n"));
    assert!(report.iter().all(|r| !r.contains(": wrecked") && !r.contains(": short")), "a crossing did not reach the next tile:\n{}", report.join("\n"));
    assert_eq!(sides, 4, "not thrown off every outer side:\n{}", report.join("\n"));
}
