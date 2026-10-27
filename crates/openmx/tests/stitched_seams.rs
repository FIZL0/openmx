//! MD47: a stitched world's ground has no gap and no step where two of its pieces meet, wrapped
//! and not.
//!
//! openmx-mods' stitch lays a world as tiles with bands of join terrains between them, each join a
//! whole terrain whose last cell no query reads (the field ends a cell short of its samples: see
//! `Tile::local`). Stitched before its C11 the joins stood a whole terrain apart, so that last
//! cell was a strip no piece owned at every join (4.5 feet every 1152 on world_mxu): a rider
//! dropped into it, or the edge test threw them from it. Since C11 the joins overlap by a leaf.
//!
//! The world here is that layout, generated: four tiles of 9-foot samples round a cross of join
//! bands, each join one root cell of 16 samples, every piece written from one smooth relief so the
//! data is welded. [`Tiled::seam_walk`] walks every piece's edge, and on a wrapped axis the world's
//! own, a ray down a foot a step an eighth of a foot to either side, through the ride's own
//! ground query.
//!
//! * Laid as stitch lays joins now, a join every 15 cells: every ray finds ground and the two sides
//!   of every seam are within a sixteenth of a foot, wrapped and not (wrapped, with the seam joins
//!   [`generate::seam_joins`] makes). Laid a cell apart as before C11, the walk finds the gaps.
//! * The same world as a mod, through the shell's own loader (`--seams`): the same, and the
//!   shell's seam joins close the wrap with no gap.
//! * A ride on the gas across the join band, through the other tile and over the wrap onto the
//!   first again: never down, never off the ground by more than its suspension, and no tick's
//!   height off the ground differs from the last by more than it does on the tiles alone. The
//!   rides skip without the disc, since the bike is the disc's.
//!
//! The worlds openmx-mods builds off the user's disc are not in this repository; the ignored test
//! walks them from `$OPENMX_WORLDS`, and the journal entry has its run.

use game_sim::tiles::{generate, Piece, Reach, SeamWalk, Source, Tile, Tiled};
use game_sim::track::TerrainLayout;
use ps2_float::F;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Feet a sample, every piece.
const SPACING: f32 = 9.0;

/// A tile's kept side, 60 cells; a join band's width, one root cell's 15; the world's side.
const KEPT: f32 = 540.0;
const BAND: f32 = 135.0;
const SIDE: f32 = KEPT + BAND + KEPT;

/// The wrapped world's side: the seam join's 15 cells past the far edge.
const ROUND: f32 = SIDE + 15.0 * SPACING;

/// Rolling ground, the same function for every piece, coming round on both axes every [`ROUND`]
/// feet as a stitched world's field does since openmx-mods' C12 (a torus on a wrapped axis): the
/// seam join across the wrap is then a straight run between two edges that already agree, and
/// bends with the ground rather than ramping over a step.
fn relief(x: f64, z: f64) -> f64 {
    let turn = std::f64::consts::TAU / f64::from(ROUND);
    40.0 + 3.0 * (turn * x).sin() * (turn * z).cos() + 1.5 * (2.0 * turn * x + 1.0).sin() + (2.0 * turn * z).cos()
}

/// One piece of the world: a name, where its sample (0, 0) is, its root cells, and what it keeps
/// (`None`: every cell, a join).
struct Spec {
    name: String,
    at: [u32; 2],
    cells: (u32, u32),
    keep: Option<[u32; 4]>,
}

/// The tiles, `a` to `d`, and the joins between them `step` cells apart along each band: 15 is
/// stitch's layout since C11, 16 the one before it.
fn specs(step: u32) -> Vec<Spec> {
    let mut out = Vec::new();
    for (name, x, z) in [("a", 0, 0), ("b", 1, 0), ("c", 0, 1), ("d", 1, 1)] {
        let at = [x * (KEPT + BAND) as u32, z * (KEPT + BAND) as u32];
        out.push(Spec { name: name.into(), at, cells: (4, 4), keep: Some([0, 0, 60, 60]) });
    }
    let reach = (SIDE / SPACING) as u32;
    let mut k = 0;
    while k * step < reach {
        let along = k * step * SPACING as u32;
        out.push(Spec { name: format!("x{k:02}"), at: [KEPT as u32, along], cells: (1, 1), keep: None });
        out.push(Spec { name: format!("z{k:02}"), at: [along, KEPT as u32], cells: (1, 1), keep: None });
        k += 1;
    }
    out
}

fn terrain(s: &Spec) -> Vec<u8> {
    let [ax, az] = s.at.map(f64::from);
    let sp = f64::from(SPACING);
    generate::terrain(&s.name, s.cells, SPACING, |x, z| relief(ax + f64::from(x) * sp, az + f64::from(z) * sp), |_, _| 1)
}

fn tile(s: &Spec) -> Tile {
    let layout = TerrainLayout {
        width: s.cells.0,
        breadth: s.cells.1,
        width_scale: F(SPACING),
        breadth_scale: F(SPACING),
        ..TerrainLayout::default()
    };
    let mut t = Tile {
        name: s.name.clone(),
        at: s.at.map(|v| F(v as f32)),
        layout,
        spacing: F(SPACING),
        levels: 1,
        keep: [0; 4],
        first_owner: 0,
    };
    t.keep = s.keep.unwrap_or_else(|| t.every_cell());
    t
}

/// The pieces' terrains, in memory.
struct Terrains(Vec<Vec<u8>>);

impl Source for Terrains {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        Ok(Piece { terrain: self.0[index].clone(), shapes: Vec::new() })
    }
}

/// The world laid with joins `step` cells apart, and closed round on both axes when `wrap` is, by
/// the seam joins made from its edges' ground (as the shell's `close_seams` makes them).
fn world(step: u32, wrap: bool) -> Tiled {
    let specs = specs(step);
    let mut tiles: Vec<Tile> = specs.iter().map(tile).collect();
    let mut bytes: Vec<Vec<u8>> = specs.iter().map(terrain).collect();
    if wrap {
        let open = Tiled::new(tiles.clone(), Box::new(Terrains(bytes.clone())), Reach::DESKTOP).unwrap();
        let extent = open.extent().map(|v| v.0);
        let ground = |x: f64, z: f64| {
            let (x, z) = ((x as f32).clamp(0.0, extent[0] - 0.01), (z as f32).clamp(0.0, extent[1] - 0.01));
            open.at(F(x), F(z)).map_or((0.0, 0), |h| (f64::from(h.y.0), h.surface))
        };
        for (t, b) in generate::seam_joins(extent.map(|v| v as u32), [true, true], SPACING as u32, &ground).unwrap() {
            tiles.push(t);
            bytes.push(b);
        }
    }
    let t = Tiled::new(tiles, Box::new(Terrains(bytes)), Reach::DESKTOP).unwrap();
    if wrap { t.wrapped([true, true]).unwrap() } else { t }
}

fn walk(step: u32, wrap: bool) -> SeamWalk {
    world(step, wrap).seam_walk(F(1.0), F(0.125))
}

/// Laid as stitch lays it now, every seam is closed and level across, wrapped and not; the wrap's
/// own seam too.
#[test]
fn every_seam_of_a_stitched_world_has_ground_on_both_sides() {
    for wrap in [false, true] {
        let w = walk(15, wrap);
        println!("wrap {wrap}: {w:?}");
        assert_eq!((w.gaps, &w.first_gaps), (0, &Vec::new()), "wrap {wrap}: rays with no ground");
        assert!(w.points > 20_000 && w.met > 5_000, "wrap {wrap}: walked too little: {w:?}");
        assert!(w.steepest < F(0.0625), "wrap {wrap}: a step of {} ft at {:?} between {:?}", w.steepest.0, w.steepest_at, w.steepest_between);
        assert!(w.wrap_steepest < F(0.0625), "wrap {wrap}: a step of {} ft across the wrap", w.wrap_steepest.0);
    }
    assert_eq!(world(15, false).extent().map(|v| v.0), [SIDE, SIDE]);
    assert_eq!(world(15, true).extent().map(|v| v.0), [ROUND, ROUND]);
}

/// Laid a whole join apart, as stitch did before C11, each join's last cell is a strip nothing
/// owns, and the walk finds it: so the test above can tell.
#[test]
fn joins_a_terrain_apart_leave_the_gaps_the_walk_finds() {
    for wrap in [false, true] {
        let mut world = world(16, wrap);
        let holes: Vec<[f32; 4]> = world.holes().iter().map(|h| h.map(|v| v.0)).collect();
        // The first strip of each band: its first join ends at 135 feet, the next starts at 144.
        let in_strip = |[a0, b0, a1, b1]: [f32; 4]| a0 >= 135.0 && a1 <= 144.0 && b0 >= KEPT && b1 <= KEPT + BAND;
        assert!(holes.iter().any(|&[x0, z0, x1, z1]| in_strip([x0, z0, x1, z1])), "wrap {wrap}: {holes:?}");
        assert!(holes.iter().any(|&[x0, z0, x1, z1]| in_strip([z0, x0, z1, x1])), "wrap {wrap}: {holes:?}");
        let w = world.seam_walk(F(1.0), F(0.125));
        assert!(w.gaps > 100, "wrap {wrap}: {w:?}");
    }
    let w = walk(16, false);
    let strip = |a: F, b: F| a.0 > 135.0 && a.0 < 144.0 && b.0 > KEPT && b.0 < KEPT + BAND;
    assert!(w.first_gaps.iter().all(|[x, z]| strip(*x, *z) || strip(*z, *x)), "{:?}", w.first_gaps);
}

// ---- the same world as a mod, through the shell ----

fn scene(s: &Spec) -> String {
    [
        "[SceneInfo]".to_string(),
        format!("SceneName=MD47 {}", s.name),
        "DefaultStartPosition=200.00,40.00,270.00".into(),
        "DefaultStartDirection=1.00,0.00,0.00".into(),
        "[Environment]".into(),
        format!("TerrainFile=MD47{}.trn", s.name),
        format!("TerrainWidth={}", s.cells.0),
        format!("TerrainBreadth={}", s.cells.1),
        format!("TerrainWidthScale={SPACING:.2}"),
        format!("TerrainBreadthScale={SPACING:.2}"),
        "TerrainBorderWidth=0.00".into(),
        "MinDivisionSize=96.00".into(),
        "[Fog]".into(),
        "ColorRGB=150,151,139".into(),
        String::new(),
    ]
    .join("\r\n")
}

fn track_json(id: &str) -> String {
    format!(
        r#"{{
  "standard": 1, "kind": "track", "id": "{id}", "name": "MD47 {id}",
  "source": {{"game": "Real elevation", "data": "generated by openmx's MD47 test", "date": "2026-10-27"}},
  "version": "1.0.0", "author": "crates/openmx/tests/stitched_seams.rs", "permission": "public-domain", "todo": []
}}"#
    )
}

fn world_json(id: &str, specs: &[Spec], wrap: bool) -> String {
    let tiles: Vec<String> = specs
        .iter()
        .filter_map(|s| s.keep.map(|k| format!(r#"{{"tile": "{}", "at": [{}.0, {}.0], "keep": {k:?}}}"#, s.name, s.at[0], s.at[1])))
        .collect();
    let joins: Vec<String> = specs
        .iter()
        .filter(|s| s.keep.is_none())
        .map(|s| format!(r#"{{"join": "{}", "at": [{}.0, {}.0], "between": ["a", "b"]}}"#, s.name, s.at[0], s.at[1]))
        .collect();
    let wrap = if wrap { r#""wrap": "xz", "# } else { "" };
    format!(
        r#"{{
  "standard": 1, "kind": "world", "id": "{id}", "name": "MD47 stitched",
  "source": {{"game": "Combined", "games": ["Real elevation"]}},
  "version": "1.0.0", "author": "crates/openmx/tests/stitched_seams.rs", "permission": "public-domain",
  "tiles": [{}],
  "joins": [{}],
  {wrap}"drop": [], "sky": "a", "spawn": {{"tile": "a"}}, "todo": []
}}"#,
        tiles.join(", "),
        joins.join(", ")
    )
}

fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A `mods/` folder holding the world wrapped, `md47_round`, and without the switch, `md47_open`.
fn mods_folder(test: &str) -> PathBuf {
    let mods = Path::new(env!("CARGO_TARGET_TMPDIR")).join("md47").join(test).join("mods");
    let _ = std::fs::remove_dir_all(&mods);
    let specs = specs(15);
    for (id, wrap) in [("md47_round", true), ("md47_open", false)] {
        let world = mods.join("tracks").join(id);
        write(&world, "mod.json", world_json(id, &specs, wrap));
        for s in &specs {
            if s.keep.is_some() {
                let dir = world.join("tiles").join(&s.name);
                write(&dir, "mod.json", track_json(&format!("md47_{}", s.name)));
                write(&dir, &format!("Env/MD47{0}/MD47{0}.scn", s.name), scene(s));
                write(&dir, &format!("Env/MD47{0}/MD47{0}.ps2.trn", s.name), terrain(s));
            } else {
                write(&world, &format!("joins/{0}/{0}.ps2.trn", s.name), terrain(s));
            }
        }
    }
    mods
}

/// The shell's output for a world in a `mods/` folder, or `None` without the disc.
fn shell(mods: &Path, track: &str, args: &[&str]) -> Option<String> {
    if let Err(e) = game_render::disc::Disc::open_configured() {
        eprintln!("skipped: {e}");
        return None;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", track, "--mods", mods.to_str().unwrap()])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    Some(said)
}

fn line<'a>(said: &'a str, prefix: &str) -> &'a str {
    said.lines().find(|l| l.starts_with(prefix)).unwrap_or_else(|| panic!("no `{prefix}` line:\n{said}"))
}

/// The shell loads the world, wrapped and not, with no hole and no gap along any seam.
#[test]
fn the_shell_walks_the_worlds_seams_and_finds_no_gap() {
    let mods = mods_folder("walk");
    for id in ["md47_open", "md47_round"] {
        let Some(said) = shell(&mods, id, &["--trace", "1", "--seams"]) else { return };
        assert_eq!(line(&said, "world holes:"), "world holes: none, so only the outer edge throws", "{id}");
        let seams = line(&said, "world seams:");
        println!("{id}: {seams}");
        assert!(seams.contains(", no gaps, "), "{id}: {seams}");
        let step = |after: &str| -> f32 { seams.split(after).nth(1).unwrap().split(' ').next().unwrap().parse().unwrap() };
        assert!(step("steepest step ") < 0.0625, "{id}: {seams}");
        assert!(step("), ") < 0.0625, "{id}: {seams}");
    }
}

/// One traced tick: where the bike is, and whether the rider is down.
struct Tick {
    frame: usize,
    x: f32,
    y: f32,
    z: f32,
    wreck: bool,
}

fn ride(mods: &Path, track: &str, x: f32, z: f32, degrees: f32, frames: usize) -> Option<Vec<Tick>> {
    let at = format!("{x},{z},{degrees}");
    let said = shell(mods, track, &["--trace", &frames.to_string(), "--trace-gas", "--trace-feel", "--trace-at", &at])?;
    let mut out = Vec::new();
    for l in said.lines() {
        let w: Vec<&str> = l.split_whitespace().collect();
        let Some(frame) = w.first().and_then(|f| f.parse::<usize>().ok()) else { continue };
        if w.len() != 12 {
            continue;
        }
        let f = |i: usize| w[i].parse::<f32>().unwrap();
        out.push(Tick { frame, x: f(5), y: f(6), z: f(7), wreck: w[11] == "1" });
    }
    assert!(!out.is_empty(), "no trace rows:\n{said}");
    assert!(!said.contains("thrown: tick"), "thrown:\n{}", line(&said, "thrown: tick"));
    Some(out)
}

/// The ground the wrapped world rides on: the relief, and across the seam join a straight run from
/// the far edge's height to the near edge's.
fn ground(x: f32, z: f32) -> f32 {
    let (x, z) = (f64::from(x.rem_euclid(ROUND)), f64::from(z.rem_euclid(ROUND)));
    let side = f64::from(SIDE);
    let strip = f64::from(ROUND - SIDE);
    let along_x = |x: f64, z: f64| if x < side { relief(x, z) } else { relief(side, z) + (relief(0.0, z) - relief(side, z)) * (x - side) / strip };
    (if z < side { along_x(x, z) } else { along_x(x, side) + (along_x(x, 0.0) - along_x(x, side)) * (z - side) / strip }) as f32
}

/// Whether a point is within `reach` feet of a seam the ride crosses along `x`: a tile's edge, the
/// band's joins' or the wrap's.
fn near_seam(x: f32, reach: f32) -> bool {
    let x = x.rem_euclid(ROUND);
    [0.0, KEPT, KEPT + BAND, SIDE, ROUND].iter().any(|s| (x - s).abs() < reach)
}

/// On the gas east across the join band, through `b`, over the seam join and the wrap onto `a`
/// again: never down, never thrown, and the bike's height over the ground as steady at every seam
/// as on the tiles between them.
#[test]
fn a_ride_across_the_seams_at_speed_neither_drops_nor_bounces() {
    let mods = mods_folder("ride");
    for (id, frames) in [("md47_round", 900), ("md47_open", 220)] {
        let Some(ticks) = ride(&mods, id, 200.0, 270.0, 90.0, frames) else { return };
        if let Some(t) = ticks.iter().find(|t| t.wreck) {
            panic!("{id}: down on tick {} at ({}, {}, {})", t.frame, t.x, t.y, t.z);
        }
        let last = ticks.last().unwrap();
        // The first ticks settle the bike on its springs; from then on it is riding.
        let riding = &ticks[30..];
        let over: Vec<f32> = riding.iter().map(|t| t.y - ground(t.x, t.z)).collect();
        let mut jolt = [0.0f32; 2];
        let mut seams_crossed = 0;
        for (i, w) in riding.windows(2).enumerate() {
            let change = (over[i + 1] - over[i]).abs();
            let at_seam = near_seam(w[1].x, 15.0);
            jolt[usize::from(at_seam)] = jolt[usize::from(at_seam)].max(change);
            seams_crossed += usize::from(at_seam && !near_seam(w[0].x, 15.0));
            assert!(over[i + 1] > -1.0, "{id}: tick {} at ({}, {}): {} ft under the ground", w[1].frame, w[1].x, w[1].z, -over[i + 1]);
        }
        println!(
            "{id}: {} ticks to x {:.0}, {seams_crossed} seams, height over the ground {:.2}..{:.2} ft, \
             largest jolt a tick {:.3} ft on the tiles and {:.3} ft at a seam",
            ticks.len(),
            last.x,
            over.iter().cloned().fold(f32::MAX, f32::min),
            over.iter().cloned().fold(f32::MIN, f32::max),
            jolt[0],
            jolt[1]
        );
        assert!(seams_crossed >= if id == "md47_round" { 5 } else { 2 }, "{id}: crossed {seams_crossed} seams");
        assert!(jolt[1] <= jolt[0] * 1.5 + 0.02, "{id}: a jolt of {} ft a tick at a seam against {} on the tiles", jolt[1], jolt[0]);
    }
}

/// openmx-mods' worlds as stitch built them off the user's disc, in `$OPENMX_WORLDS/tracks`
/// (a `mods/` folder): every seam walked, wrapped as their files say. Not in this repository.
#[test]
#[ignore = "needs openmx-mods' stitched worlds in $OPENMX_WORLDS/tracks, and the disc"]
fn the_stitched_worlds_have_no_gap_along_a_seam() {
    let mods = PathBuf::from(std::env::var_os("OPENMX_WORLDS").expect("OPENMX_WORLDS: a mods/ folder with tracks/world_*"));
    let mut failed = Vec::new();
    for id in ["world_mxu_free", "world_mxu", "world_mxvatv", "world_untamed", "world_ultra"] {
        if !mods.join("tracks").join(id).is_dir() {
            continue;
        }
        let said = shell(&mods, id, &["--trace", "1", "--seams"]).expect("the disc");
        let seams = line(&said, "world seams:");
        println!("{id}: {}\n{id}: {seams}", line(&said, "world holes:"));
        if !seams.contains(", no gaps, ") {
            failed.push(id);
        }
    }
    assert!(failed.is_empty(), "gaps along the seams of {failed:?}");
}
