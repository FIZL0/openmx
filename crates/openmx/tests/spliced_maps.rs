//! P71: every freeride map spliced into one, `free_all` (crates/openmx/src/spliced.rs).
//!
//! The five maps lie west to east, each cut at the peak of the ridge it shares with its
//! neighbour, and a strip one leaf wide joins the two peaks. Nothing between two maps is a hole,
//! so the only shootoff is the world's outer edge, which is each outer map's own plateau. Proved
//! by riding: across every seam both ways on the gas, never thrown and over into the next map;
//! off each of the world's four sides, thrown at the edge's margin. Skips without the disc, since
//! the maps are the disc's.

use std::path::Path;
use std::process::Command;

/// The shootoff's margin, `game_sim::off_map::MARGIN`.
const MARGIN: f32 = 150.0;

/// Whether the disc is there; the tests skip without it.
fn disc() -> bool {
    match game_render::disc::Disc::open_configured() {
        Ok(_) => true,
        Err(e) => {
            eprintln!("skipped: {e}");
            false
        }
    }
}

fn shell(args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--no-sound", "--track", "free_all"])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    said
}

/// The world's size and each seam's join strip in `x`, from the load's lines.
struct Layout {
    width: f32,
    depth: f32,
    seams: Vec<(String, f32, f32)>,
}

fn layout() -> Layout {
    let said = shell(&["--trace", "1", "--trace-feel"]);
    let line = |p: &str| said.lines().find(|l| l.starts_with(p)).unwrap_or_else(|| panic!("no `{p}` line:\n{said}")).to_string();
    assert_eq!(line("world holes:"), "world holes: none, so only the outer edge throws");
    let world = line("world: free_all");
    assert!(world.contains("5 tiles"), "{world}");
    let size: Vec<f32> = world.split(", ").nth(2).unwrap().split(' ').filter_map(|w| w.parse().ok()).collect();
    // "spliced: join free06_free07_0 x 6129..6264 z 0..2295": one a seam, its first chunk.
    let seams = said
        .lines()
        .filter_map(|l| l.strip_prefix("spliced: join "))
        .filter(|l| l.split(' ').next().unwrap().ends_with("_0"))
        .map(|l| {
            let w: Vec<&str> = l.split(' ').collect();
            let (x0, x1) = w[2].split_once("..").unwrap();
            (w[0].trim_end_matches("_0").to_string(), x0.parse().unwrap(), x1.parse().unwrap())
        })
        .collect();
    Layout { width: size[0], depth: size[1], seams }
}

/// One traced tick.
struct Tick {
    x: f32,
    z: f32,
    wreck: bool,
}

/// A ride on the gas from `(x, z)` facing `degrees` (90 along `+x`): its ticks up to its first
/// wreck, and where the edge threw it if it did.
fn ride(x: f32, z: f32, degrees: f32, frames: usize) -> (Vec<Tick>, Option<(usize, f32, f32)>) {
    let at = format!("{x},{z},{degrees}");
    let said = shell(&["--trace", &frames.to_string(), "--trace-gas", "--trace-feel", "--trace-at", &at]);
    let mut ticks = Vec::new();
    let mut thrown = None;
    for l in said.lines() {
        if let Some(rest) = l.strip_prefix("thrown: tick ") {
            let w: Vec<f32> = rest.replace(" at", "").replace(',', "").split_whitespace().map(|w| w.parse().unwrap()).collect();
            thrown.get_or_insert((w[0] as usize, w[1], w[2]));
            continue;
        }
        let w: Vec<&str> = l.split_whitespace().collect();
        if w.len() != 12 || w[0].parse::<usize>().is_err() || ticks.last().is_some_and(|t: &Tick| t.wreck) {
            continue;
        }
        let f = |i: usize| w[i].parse::<f32>().unwrap();
        ticks.push(Tick { x: f(5), z: f(7), wreck: w[11] == "1" });
    }
    assert!(!ticks.is_empty(), "no trace rows:\n{said}");
    (ticks, thrown)
}

/// Over every seam's ridge, both ways, from 150 feet short of the join strip to 150 feet past it:
/// never thrown, and the far side reached before any wreck (the way down off a ridge is steep, and
/// a ride on the gas wrecks at its foot well past the seam).
#[test]
fn a_ride_crosses_every_seam_with_no_shootoff() {
    if !disc() {
        return;
    }
    let l = layout();
    assert_eq!(l.seams.len(), 4, "seams: {:?}", l.seams.iter().map(|s| &s.0).collect::<Vec<_>>());
    let mut report = Vec::new();
    for (name, x0, x1) in &l.seams {
        for z in [l.depth * 0.25, l.depth * 0.5] {
            for (from, degrees, goal) in [(x0 - MARGIN, 90.0, x1 + MARGIN), (x1 + MARGIN, 270.0, x0 - MARGIN)] {
                let (ticks, thrown) = ride(from, z, degrees, 600);
                let what = format!("{name} from ({from}, {z:.0}) at {degrees}");
                assert!(thrown.is_none(), "{what}: thrown {thrown:?}");
                let reached = ticks.iter().position(|t| if degrees == 90.0 { t.x >= goal } else { t.x <= goal });
                let last = ticks.last().unwrap();
                assert!(reached.is_some(), "{what}: never at x {goal}; ended at ({}, {}) wreck {}", last.x, last.z, last.wreck);
                report.push(format!("{what}: past the seam on tick {}", reached.unwrap()));
            }
        }
    }
    println!("{}", report.join("\n"));
}

/// Off each of the world's four sides from 400 feet inside it, on the gas: thrown at the margin.
/// The north and south rides are on the middle map's ground, the west and east on the outer two.
#[test]
fn riding_off_the_outer_edge_shoots_you_off() {
    if !disc() {
        return;
    }
    let l = layout();
    let (cx, cz) = (l.width / 2.0, l.depth / 2.0);
    for (x, z, degrees, along_x, edge) in [
        (400.0, cz, 270.0, true, MARGIN),
        (l.width - 400.0, cz, 90.0, true, l.width - MARGIN),
        (cx, 400.0, 180.0, false, MARGIN),
        (cx, l.depth - 400.0, 0.0, false, l.depth - MARGIN),
    ] {
        let (ticks, thrown) = ride(x, z, degrees, 600);
        let last = ticks.last().unwrap();
        let Some((tick, tx, tz)) = thrown else { panic!("from ({x}, {z}) at {degrees}: never thrown; ended at ({}, {})", last.x, last.z) };
        let at = if along_x { tx } else { tz };
        assert!((at - edge).abs() < 12.0, "from ({x}, {z}) at {degrees}: thrown at ({tx}, {tz}), the margin is at {edge}");
        println!("from ({x}, {z}) at {degrees}: thrown on tick {tick} at ({tx:.1}, {tz:.1})");
    }
}
