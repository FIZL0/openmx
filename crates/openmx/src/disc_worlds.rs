//! MX Unleashed's worlds off the player's own disc (P232): `world_mxu`, the WORLD, every free-ride map
//! and every national track in one wrapped ground with the free-ride maps in its middle (P253);
//! `world_mxu_free`, the five free-ride maps; and `world_mxu_nat`, the 22 nationals; built the first
//! time one is picked, with nothing to install.
//!
//! **The tiles** are the disc's maps as they are: tile `mxu_<track>` is `Tracks\<track>.res` with its
//! `Tracks\<track>_2.res`, the two archives a ride on that track reads, so every piece of the world
//! is a [`Origin::Disc`] one and nothing of the disc is copied anywhere.
//!
//! **The joins** are the stitcher's (`crates/stitch`), made from the world specs in
//! `crates/stitch/worlds/` exactly as openmx-mods' `stitch` makes them from imported tile folders —
//! the same tile cuts, the same blend, the same files. They are written once under the mods folder,
//! `cache/stitch/<id>/<key>/` (`mod.json` and `joins/`), and read from there after. The key is
//! [`stitch::VERSION`], the spec and every tile's scene and terrain bytes, hashed: a new stitcher or
//! another disc builds afresh and the old key's folder goes; an app update with the same stitcher
//! keeps them (on iOS the mods folder is in Documents, which updates leave alone).
//!
//! A build from cold widens and re-solves until its join ground holds, three minutes and 1,300 s of
//! CPU for `world_mxu` (30 plans, ten whole solves), so each world's build starts where its last pass
//! began for this key ([`STARTS`], P299): one plan, one solve, 20 s and 210 s of CPU, the same bytes.
//!
//! An installed `mods/tracks/<id>` world of the same id is ridden in place of this one (the shell
//! asks [`crate::worlds::is_world`] first), and mods go on making worlds of their own.

use crate::track_mods::{self, Json};
use crate::worlds::{self, Origin, WorldMod};
use game_render::disc::{Disc, Res};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The worlds and their specs, `(id, spec)`, in the track pick's order.
pub const SPECS: [(&str, &str); 3] = [
    ("world_mxu", include_str!("../../stitch/worlds/world_mxu.json")),
    ("world_mxu_free", include_str!("../../stitch/worlds/world_mxu_free.json")),
    ("world_mxu_nat", include_str!("../../stitch/worlds/world_mxu_nat.json")),
];

/// Where each world's build starts (P299), `(id, start)`: `{"key", "start"}`, the [`stitch::build::Start`]
/// a cold build of that cache key ended at, written by a build into its cache folder as `start.json`.
/// With the key this disc and stitcher make, the build does that last pass alone and writes the same
/// joins (MX Unleashed: WORLD's 30 plans and ten solves are one of each); with another key, an older
/// file or another disc, it passes from cold. `tests/disc_worlds.rs` holds them to the key.
pub const STARTS: [(&str, &str); 3] = [
    ("world_mxu", include_str!("../../stitch/worlds/world_mxu.start.json")),
    ("world_mxu_free", include_str!("../../stitch/worlds/world_mxu_free.start.json")),
    ("world_mxu_nat", include_str!("../../stitch/worlds/world_mxu_nat.start.json")),
];

/// The build's start for world `id` from its [`STARTS`] file, where that file was written for `key`.
fn start_for(id: &str, key: &str) -> Option<stitch::build::Start> {
    let (_, text) = STARTS.iter().find(|(w, _)| *w == id)?;
    let j = Json::parse(text).ok()?;
    if j.get("key").and_then(Json::str) != Some(key) {
        return None;
    }
    stitch::build::Start::from_json(j.get("start")?).ok()
}

/// A [`STARTS`] file's text: the build of `key` ended at `start`.
pub fn start_file(key: &str, start: &stitch::build::Start) -> String {
    format!("{{\"key\": \"{key}\", \"start\": {}}}\n", start.to_json())
}

/// `free_all`, P71's "Every free ride", as a word `--track` and a saved setting may still hold: it is
/// `world_mxu_free` now.
pub const OLD_ID: &str = crate::menu::OLD_SPLICED;

/// What `--track` or a saved setting names, with [`OLD_ID`] read as `world_mxu_free`.
pub fn id_of(track: &str) -> &str {
    if track == OLD_ID {
        "world_mxu_free"
    } else {
        track
    }
}

/// Whether `id` is one of these worlds.
pub fn is_disc_world(id: &str) -> bool {
    SPECS.iter().any(|(w, _)| *w == id)
}

/// The spec's `sky` tile's track (`free06`, `nat01`): the map the ride is lit and dressed from, and
/// whose saved markers and setup a ride on the world starts from.
pub fn home_track(id: &str) -> Option<String> {
    let (_, spec) = SPECS.iter().find(|(w, _)| *w == id)?;
    let sky = Json::parse(spec).ok()?.get("sky").and_then(Json::str)?.to_string();
    sky.strip_prefix("mxu_").map(str::to_string)
}

/// Where a world's built joins are kept: `<mods>/cache/stitch/<id>/`.
pub fn cache_dir(mods: &Path, id: &str) -> PathBuf {
    mods.join("cache").join("stitch").join(id)
}

/// One map off the disc, as far as the stitcher and the world read it.
struct Map {
    tile: String,
    member: String,
    second: Option<String>,
    res: Option<Res>,
    scene_file: String,
    scene: Vec<u8>,
    terrain_file: String,
    terrain: Vec<u8>,
}

fn read_map(disc: &mut Disc, tile: &str) -> Result<Map, String> {
    let track = tile.strip_prefix("mxu_").ok_or_else(|| format!("{tile}: not an MX Unleashed map's tile"))?;
    let ending = |suffix: String| {
        disc.entries()
            .iter()
            .filter(|e| e.name.to_ascii_lowercase().ends_with(&suffix))
            .min_by_key(|e| e.name.len())
            .map(|e| e.name.clone())
    };
    let member = ending(format!("\\{track}.res")).ok_or_else(|| format!("the disc has no {track}.res"))?;
    let second = ending(format!("\\{track}_2.res"));
    let res = disc.res(&member)?;
    let (scene_file, _, scene) = worlds::scene_of(&res).map_err(|e| format!("{track}: {e}"))?;
    let terrain_file = game_sim::track::Scene::parse(&scene).map_err(|e| format!("{scene_file}: {e}"))?.terrain_file;
    let terrain = track_mods::terrain_bytes(&res, &terrain_file).map_err(|e| format!("{track}: {e}"))?;
    Ok(Map { tile: tile.to_string(), member, second, res: Some(res), scene_file, scene, terrain_file, terrain })
}

/// The tile's mod.json, as openmx-mods' import of the disc map writes it as far as the stitcher reads
/// one: a track, MX Unleashed's, ridden from the player's own disc, starting where its scene does.
fn meta(tile: &str) -> Json {
    let s = |v: &str| Json::Str(v.to_string());
    Json::Obj(vec![
        ("kind".into(), s("track")),
        ("id".into(), s(tile)),
        ("source".into(), Json::Obj(vec![("game".into(), s("MX Unleashed"))])),
        ("permission".into(), s("own-disc")),
    ])
}

/// The cache key: the stitcher's version, the spec and every tile's scene and terrain, hashed.
fn key(spec: &str, maps: &[Map]) -> String {
    let mut h = Sha256::new();
    h.update(stitch::VERSION.to_le_bytes());
    let mut part = |b: &[u8]| {
        h.update((b.len() as u64).to_le_bytes());
        h.update(b);
    };
    part(spec.as_bytes());
    for m in maps {
        part(m.tile.as_bytes());
        part(&m.scene);
        part(&m.terrain);
    }
    h.finalize().iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// World `id`'s tiles off `disc` as the stitcher reads them, and its spec's build parameters: what
/// [`world`] builds from, for a test to lay out without building (P251).
pub fn stitch_tiles(disc: &mut Disc, id: &str) -> Result<(Vec<stitch::Tile>, stitch::build::Params), String> {
    let (_, spec) = SPECS.iter().find(|(w, _)| *w == id).ok_or_else(|| format!("{id}: not a disc world"))?;
    let spec_json = Json::parse(spec).map_err(|e| format!("{id}.json: {e}"))?;
    let params = stitch::build::Params::from_spec(&spec_json).map_err(|e| format!("{id}.json: {e}"))?;
    let tiles = spec_json.get("tiles").and_then(Json::arr).unwrap_or_default();
    let tiles = tiles
        .iter()
        .filter_map(Json::str)
        .map(|t| {
            let m = read_map(disc, t)?;
            stitch::Tile::from_parts(&m.tile, meta(&m.tile), &m.scene_file, m.scene, &m.terrain_file, &m.terrain)
        })
        .collect::<Result<_, _>>()?;
    Ok((tiles, params))
}

/// World `id` off `disc`, its joins built under `mods` the first time (`say` hears that it is
/// building, which takes a while) and read from there after.
pub fn world(disc: &mut Disc, mods: &Path, id: &str, say: &mut dyn FnMut(&str)) -> Result<WorldMod, String> {
    world_watched(disc, mods, id, say, &|_, _, _| Ok(()))
}

/// The line under the loading bar while a world's maps are read off the disc.
pub const READING_MAPS: &str = "Reading the maps";
/// The line while its joins are built, before the pass's number.
pub const JOINING: &str = "Joining the maps";

/// [`world`], saying to `watch` what it is at as it goes (P260): the maps read, each pass of the
/// joins' build and its pieces, the world read. `watch`'s error — a cancelled load — stops it at
/// the next step, with nothing half-built left in the cache.
pub fn world_watched(disc: &mut Disc, mods: &Path, id: &str, say: &mut dyn FnMut(&str), watch: worlds::Watch) -> Result<WorldMod, String> {
    let (_, spec) = SPECS.iter().find(|(w, _)| *w == id).ok_or_else(|| format!("{id}: not a disc world"))?;
    let spec_json = Json::parse(spec).map_err(|e| format!("{id}.json: {e}"))?;
    let tiles: Vec<String> =
        spec_json.get("tiles").and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::str).map(str::to_string).collect();
    let mut maps: Vec<Map> = Vec::with_capacity(tiles.len());
    for (n, t) in tiles.iter().enumerate() {
        watch(READING_MAPS, n, tiles.len())?;
        maps.push(read_map(disc, t)?);
    }
    let root = cache_dir(mods, id);
    let key = key(spec, &maps);
    let dir = root.join(&key);
    if !dir.join("mod.json").is_file() {
        say(&format!("{id}: building its joins from the disc's maps, once (a minute or two)"));
        let tiles: Vec<stitch::Tile> = maps
            .iter()
            .enumerate()
            .map(|(n, m)| {
                watch(JOINING, n, maps.len())?;
                stitch::Tile::from_parts(&m.tile, meta(&m.tile), &m.scene_file, m.scene.clone(), &m.terrain_file, &m.terrain)
            })
            .collect::<Result<_, _>>()?;
        let mut params = stitch::build::Params::from_spec(&spec_json).map_err(|e| format!("{id}.json: {e}"))?;
        params.start = start_for(id, &key);
        if params.start.is_none() {
            say(&format!("{id}: no start kept for key {key}: every pass from cold"));
        }
        let part = root.join(format!("{key}.part-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&part);
        forget_dead_parts(&root);
        let built_at = std::time::Instant::now();
        let built = stitch::build::build_watched(&tiles, &part, &params, false, &|step| match step {
            stitch::build::Step::Measuring => watch(JOINING, 0, 0),
            stitch::build::Step::Planning { pass } => watch(&format!("{JOINING}, pass {pass}"), 0, 0),
            stitch::build::Step::Solving { pass, done, of } => watch(&format!("{JOINING}, pass {pass}"), done, of),
        });
        // A stopped build's half (a cancel, P260) is nobody's: it goes.
        let built = match built {
            Ok(b) => b,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&part);
                return Err(e);
            }
        };
        say(&format!("{id}: joins built in {} passes, {:.1} s", built.passes, built_at.elapsed().as_secs_f64()));
        // Where its last pass began, for STARTS (P299).
        let _ = std::fs::write(part.join(id).join("start.json"), start_file(&key, &built.start));
        // Other keys' joins are another disc's or another stitcher's: they go.
        if let Ok(list) = std::fs::read_dir(&root) {
            for e in list.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name != key && !name.contains(".part-") {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        // Another shell building the same key at once got there first: its joins are these.
        let moved = std::fs::rename(part.join(id), &dir);
        let _ = std::fs::remove_dir_all(&part);
        if let Err(e) = moved {
            if !dir.join("mod.json").is_file() {
                return Err(format!("{}: {e}", dir.display()));
            }
        }
    }
    let text = std::fs::read_to_string(dir.join("mod.json")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let json = Json::parse(&text).map_err(|e| format!("{id}: mod.json: {e}"))?;
    let mut open = |tile: &str| -> Result<(Res, String, Option<String>), String> {
        let m = maps.iter_mut().find(|m| m.tile == tile).ok_or_else(|| format!("{tile} is not one of the spec's tiles"))?;
        let res = match m.res.take() {
            Some(r) => r,
            None => disc.res(&m.member)?,
        };
        Ok((res, m.member.clone(), m.second.clone()))
    };
    let mut w = worlds::read_watched(&dir, id, &json, Some(&mut open), watch)?;
    debug_assert!(w.pieces[..w.tile_count].iter().all(|p| matches!(p.origin, Origin::Disc { .. })));
    w.dir = dir;
    Ok(w)
}

/// Take out the half-built joins of shells that are gone (P260): a build killed outright leaves its
/// `<key>.part-<pid>` folder, a world's worth of terrain, which no later build reads. Where a
/// process cannot be looked up by its id (no `/proc`), they stay.
fn forget_dead_parts(root: &Path) {
    if !Path::new("/proc/self").is_dir() {
        return;
    }
    let Ok(list) = std::fs::read_dir(root) else { return };
    for e in list.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(pid) = name.rsplit_once(".part-").and_then(|(_, p)| p.parse::<u32>().ok()) else { continue };
        if pid != std::process::id() && !Path::new("/proc").join(pid.to_string()).exists() {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiles(id: &str) -> Vec<String> {
        let (_, spec) = SPECS.iter().find(|(w, _)| *w == id).unwrap();
        let j = Json::parse(spec).unwrap();
        j.get("tiles").and_then(Json::arr).unwrap().iter().filter_map(Json::str).map(str::to_string).collect()
    }

    const FREE: [&str; 5] = ["free06", "free07", "free09", "free10", "free11"];

    fn nationals() -> Vec<String> {
        (1..=22).map(|n| format!("mxu_nat{n:02}")).collect()
    }

    /// P253: the pick's worlds are the WORLD first, then the freeride maps, then the nationals, and
    /// the menu lists them so.
    #[test]
    fn the_worlds_are_world_freerides_nationals() {
        assert_eq!(SPECS.map(|s| s.0), ["world_mxu", "world_mxu_free", "world_mxu_nat"]);
        assert_eq!(crate::menu::DISC_WORLDS.iter().map(|w| w.0).collect::<Vec<_>>(), SPECS.map(|s| s.0));
        for ((id, spec), (_, name)) in SPECS.iter().zip(crate::menu::DISC_WORLDS) {
            let j = Json::parse(spec).unwrap();
            assert_eq!(j.get("id").and_then(Json::str), Some(*id));
            assert_eq!(j.get("name").and_then(Json::str), Some(*name), "{id}");
        }
        assert_eq!(crate::menu::DISC_WORLDS[0].1, "MX Unleashed: WORLD");
        assert_eq!(id_of(OLD_ID), "world_mxu_free");
    }

    /// The WORLD is every freeride and every national, the freerides laid in its middle; the
    /// nationals' world is the 22 of them, lit from nat01.
    #[test]
    fn the_worlds_hold_their_maps() {
        let free: Vec<String> = FREE.iter().map(|t| format!("mxu_{t}")).collect();
        let world = tiles("world_mxu");
        assert_eq!(world.len(), 27);
        assert_eq!(world[..5], free[..]);
        assert_eq!(world[5..], nationals()[..]);
        let (_, spec) = SPECS[0];
        let middle: Vec<String> = Json::parse(spec).unwrap().get("middle").and_then(Json::arr).unwrap().iter().filter_map(Json::str).map(str::to_string).collect();
        assert_eq!(middle, free);
        assert_eq!(tiles("world_mxu_free"), free);
        assert_eq!(tiles("world_mxu_nat"), nationals());
        assert_eq!(home_track("world_mxu").as_deref(), Some("free06"));
        assert_eq!(home_track("world_mxu_free").as_deref(), Some("free06"));
        assert_eq!(home_track("world_mxu_nat").as_deref(), Some("nat01"));
    }

    /// P251: MX Unleashed: WORLD's maps sit close. Every side of every map faces another map's
    /// interior across at most 1,800 ft of join (the widest was 20,020 ft, a row of empty cells, and
    /// beside every national ~2,200 ft of its own empty ground besides), and at most 2,900 ft round the
    /// wrap, where a national cannot start before its own trimmed ground does (MD61). Laying it out is
    /// the build's coarse pass, not the build; without the disc it skips.
    #[test]
    fn world_mxus_widest_join_is_narrow() {
        let Ok(mut disc) = Disc::open_configured() else {
            eprintln!("skipped: no disc");
            return;
        };
        let (tiles, p) = stitch_tiles(&mut disc, "world_mxu").unwrap();
        let plan = stitch::build::layout(&tiles, &p).unwrap();
        let joins = plan.join_widths(p.wrap.as_deref());
        assert_eq!(joins.len(), 4 * 27);
        for wrapped in [false, true] {
            let w = joins.iter().filter(|j| j.wrapped == wrapped).max_by(|a, b| a.width.total_cmp(&b.width)).unwrap();
            let over = if wrapped { "over the wrap" } else { "inside" };
            eprintln!("world_mxu: widest join {over} {:.0} ft, {} side {}", w.width, tiles[w.tile].id, w.side);
        }
        for j in &joins {
            let name = |t: usize| tiles[t].id.as_str();
            let to = j.to.unwrap_or_else(|| panic!("{} faces no map on side {}", name(j.tile), j.side));
            let most = if j.wrapped { 2900.0 } else { 1800.0 };
            assert!(j.width <= most, "{} side {} to {}: {:.0} ft of join (at most {most})", name(j.tile), j.side, name(to), j.width);
        }
    }

    /// The most join between two neighbouring maps of MX Unleashed: WORLD (MD155).
    const EVEN_GAP: f64 = 1500.0;

    /// MD155: MX Unleashed: WORLD is laid out evenly. Every listed tile is placed once, and every side
    /// of every map faces another map within [`EVEN_GAP`] of join, round the wrap as well as inside it
    /// (the wrap joins were up to 2,740 ft, nationals that could not start before their own trimmed
    /// ground), so riding off any map reaches the next one across about the same stretch. With
    /// `$OPENMX_LAYOUT_DUMP` set it writes each tile's interior there, `id x0 z0 x1 z1`, and the
    /// world's extent, to draw. Without the disc it skips.
    #[test]
    fn world_mxu_is_laid_out_evenly() {
        let Ok(mut disc) = Disc::open_configured() else {
            eprintln!("skipped: no disc");
            return;
        };
        let (maps, p) = stitch_tiles(&mut disc, "world_mxu").unwrap();
        let plan = stitch::build::layout(&maps, &p).unwrap();
        let mut ids: Vec<&str> = plan.placed.iter().enumerate().map(|(t, _)| maps[t].id.as_str()).collect();
        assert_eq!(ids, tiles("world_mxu"));
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), 27, "a tile placed twice");
        if let Ok(out) = std::env::var("OPENMX_LAYOUT_DUMP") {
            let mut s = format!("extent {} {}\n", f64::from(plan.size[0]) * plan.spacing, f64::from(plan.size[1]) * plan.spacing);
            for src in &plan.sources[..plan.placed.len()] {
                let r = src.rect;
                s += &format!("{} {} {} {} {}\n", maps[src.tile].id, r[0], r[1], r[2], r[3]);
            }
            std::fs::write(out, s).unwrap();
        }
        let name = |t: usize| maps[t].id.as_str();
        for j in plan.join_widths(p.wrap.as_deref()) {
            let to = j.to.unwrap_or_else(|| panic!("{} faces no map on side {}", name(j.tile), j.side));
            assert!(j.width <= EVEN_GAP, "{} side {} to {}: {:.0} ft of join (at most {EVEN_GAP})", name(j.tile), j.side, name(to), j.width);
        }
    }
}
