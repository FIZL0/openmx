//! V50 — the shell's ride has the scene's trees in it: the trunks S97 built from the `.esb`'s solid
//! records stand in the shell's world (`game_sim::tiles::Land::Woods`, `game_sim::eco::Wooded` around
//! the track), so the shell's bike stops at a free09 tree instead of riding through it.
//!
//! The rides go through the shell's own binary headless, `--trace-feel` on the gas from a spot
//! `--trace-at` names, and skip without the disc, saying so. The tree is found in the disc's own
//! `free09.esb`, read again here with the sim's reader ([`Ecosystem::parse`]).

use std::path::Path;
use std::process::Command;

use game_render::Disc;
use game_sim::eco::{self, Ecosystem};
use ps2_float::F;

/// free09's map side: 768 samples at 9 feet ([`eco::cell_size`] of it is the plants' cell).
const EXTENT: f32 = 6912.0;
/// free09's start line, which the tree is looked for near: the shell says `start: 4310, 208, 3316`.
const START: (f32, f32) = (4310.0, 3316.0);
/// How far from the trunk the ride starts (short enough that the slope does not walk the bike off the
/// trunk's line: from 80 feet it passes 2.5 feet to the side), and how far either side the trunk is
/// from any other plant with a trunk.
const RUN_UP: f32 = 40.0;
const CLEAR: f32 = 40.0;

/// free09's ecosystem, off the disc.
fn free09() -> Option<Ecosystem> {
    let mut disc = match Disc::open_configured() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("SKIP: no disc ({e})");
            return None;
        }
    };
    let names: Vec<String> = disc.entries().iter().map(|e| e.name.clone()).collect();
    let bytes = names
        .iter()
        .filter(|n| n.to_ascii_lowercase().contains("free09") && n.to_ascii_lowercase().ends_with(".res"))
        .find_map(|n| disc.res(n).ok()?.read_suffix("free09.esb").ok())
        .expect("free09's archives hold free09.esb");
    Some(Ecosystem::parse(&bytes).expect("free09.esb parses"))
}

/// Every plant with a trunk, at its place on the ground plane.
fn trunks(eco: &Ecosystem) -> Vec<(f32, f32)> {
    let cell = eco::cell_size(F(EXTENT));
    eco.plants
        .iter()
        .filter(|p| eco.species[usize::from(p.species)].as_ref().is_some_and(|s| !s.solids.is_empty()))
        .map(|p| {
            let [x, _, z] = eco::place(p.cell, cell);
            (x.0, z.0)
        })
        .collect()
}

/// The shell headless on free09 with these arguments.
fn shell(args: &[&str]) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_openmx"))
        .current_dir(&root)
        .args(["--headless", "--track", "free09"])
        .args(args)
        .output()
        .expect("the shell runs");
    let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "the shell failed:\n{said}");
    said
}

/// One `--trace-feel` row: the speed and where the bike is on the ground plane.
struct Tick {
    speed: f32,
    x: f32,
    z: f32,
}

/// Ride on the gas from `from` straight at `to`, `frames` ticks; the shell's words and its rows.
fn ride(from: (f32, f32), to: (f32, f32), frames: usize) -> (String, Vec<Tick>) {
    let degrees = (to.0 - from.0).atan2(to.1 - from.1).to_degrees();
    let at = format!("{},{},{degrees}", from.0, from.1);
    let said = shell(&["--trace", &frames.to_string(), "--trace-gas", "--trace-feel", "--trace-at", &at]);
    let rows: Vec<Tick> = said
        .lines()
        .filter_map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            w.first()?.parse::<usize>().ok()?;
            if w.len() != 12 {
                return None;
            }
            Some(Tick { speed: w[1].parse().ok()?, x: w[5].parse().ok()?, z: w[7].parse().ok()? })
        })
        .collect();
    assert!(!rows.is_empty(), "no trace rows:\n{said}");
    (said, rows)
}

/// How far along the line from `from` to `to` a point is.
fn along(from: (f32, f32), to: (f32, f32), x: f32, z: f32) -> f32 {
    let (dx, dz) = (to.0 - from.0, to.1 - from.1);
    let len = (dx * dx + dz * dz).sqrt();
    ((x - from.0) * dx + (z - from.1) * dz) / len
}

#[test]
fn the_shell_rides_into_a_free09_tree_and_stops_at_its_trunk() {
    let Some(eco) = free09() else { return };
    let all = trunks(&eco);
    // The shell stood every trunk of the file: one prop per plant of a species with a solid.
    let (said, _) = ride(START, (START.0, START.1 + 1.0), 1);
    let solids: usize = eco.plants.iter().map(|p| eco.species[usize::from(p.species)].as_ref().map_or(0, |s| s.solids.len())).sum();
    let line = format!("trees: {solids} trunks under {} plants", eco.plants.len());
    assert!(said.lines().any(|l| l == line), "the shell's ride has free09's trees: want `{line}` in\n{said}");
    assert!(solids > 100, "free09 has trees with trunks: {solids}");
    // The tree nearest the start line with no other trunk within `CLEAR` feet, ridden at from the
    // start's side.
    let lonely = |t: &(f32, f32)| all.iter().all(|o| o == t || (o.0 - t.0).hypot(o.1 - t.1) > CLEAR);
    let tree = all
        .iter()
        .copied()
        .filter(lonely)
        .min_by(|a, b| (a.0 - START.0).hypot(a.1 - START.1).total_cmp(&(b.0 - START.0).hypot(b.1 - START.1)))
        .expect("a tree on its own");
    let from = (tree.0, tree.1 - RUN_UP);
    println!("riding at the tree at {tree:?} from {from:?}");
    let (_, hit) = ride(from, tree, 150);
    // Past it by ten feet to one side, nothing in the way.
    let (by, beside) = ((tree.0 + 10.0, tree.1), (tree.0 + 10.0, tree.1 - RUN_UP));
    let (_, missed) = ride(beside, by, 150);
    let reach = |rows: &[Tick], from, to| rows.iter().map(|t| along(from, to, t.x, t.z)).fold(f32::MIN, f32::max);
    let (hit_reach, missed_reach) = (reach(&hit, from, tree), reach(&missed, beside, by));
    let fastest = hit.iter().map(|t| t.speed).fold(0.0, f32::max);
    let last = hit.last().unwrap();
    println!(
        "at the tree: {hit_reach:.1} ft of {RUN_UP} at most, {fastest:.1} ft/s at best, {:.1} ft/s at the end; beside it: {missed_reach:.1} ft",
        last.speed
    );
    assert!(missed_reach > RUN_UP + 20.0, "ten feet to the side the bike rides on past the tree: {missed_reach} ft");
    assert!(fastest > 30.0, "the bike gets going before the tree: {fastest} ft/s");
    let stop = hit.windows(2).position(|w| w[0].speed > 30.0 && w[1].speed < 10.0);
    assert!(stop.is_some(), "the trunk stops the bike in one tick, from over 30 ft/s to under 10");
    assert!(hit_reach < RUN_UP && hit_reach > RUN_UP - 10.0, "the bike stops at the trunk, {hit_reach} ft of {RUN_UP}");
}
