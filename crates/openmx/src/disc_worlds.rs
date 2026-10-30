//! MX Unleashed's worlds off the player's own disc (P232): `world_mxu_free`, the five free-ride maps
//! in one wrapped ground, and `world_mxu`, those and every national track, built the first time one
//! is picked, with nothing to install.
//!
//! **The tiles** are the disc's maps as they are: tile `mxu_<track>` is `Tracks\<track>.res` with its
//! `Tracks\<track>_2.res`, the two archives a ride on that track reads, so every piece of the world
//! is a [`Origin::Disc`] one and nothing of the disc is copied anywhere.
//!
//! **The joins** are the stitcher's (`crates/stitch`), made from the world specs in
//! `crates/stitch/worlds/` exactly as openmx-mods' `stitch` makes them from imported tile folders —
//! the same tile cuts, the same blend, the same files. Making them is a minute or two for `world_mxu`
//! (its 1222 joins), so they are written once under the mods folder, `cache/stitch/<id>/<key>/`
//! (`mod.json` and `joins/`), and read from there after. The key is [`stitch::VERSION`], the spec and
//! every tile's scene and terrain bytes, hashed: a new stitcher or another disc builds afresh and the
//! old key's folder goes.
//!
//! An installed `mods/tracks/<id>` world of the same id is ridden in place of this one (the shell
//! asks [`crate::worlds::is_world`] first), and mods go on making worlds of their own.

use crate::track_mods::{self, Json};
use crate::worlds::{self, Origin, WorldMod};
use game_render::disc::{Disc, Res};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The worlds and their specs, `(id, spec)`, in the track pick's order.
pub const SPECS: [(&str, &str); 2] = [
    ("world_mxu_free", include_str!("../../stitch/worlds/world_mxu_free.json")),
    ("world_mxu", include_str!("../../stitch/worlds/world_mxu.json")),
];

/// `free_all`, P71's "Every free ride", as a word `--track` and a saved setting may still hold: it is
/// `world_mxu_free` now.
pub const OLD_ID: &str = crate::menu::OLD_SPLICED;

/// What `--track` or a saved setting names, with [`OLD_ID`] read as `world_mxu_free`.
pub fn id_of(track: &str) -> &str {
    if track == OLD_ID {
        SPECS[0].0
    } else {
        track
    }
}

/// Whether `id` is one of these worlds.
pub fn is_disc_world(id: &str) -> bool {
    SPECS.iter().any(|(w, _)| *w == id)
}

/// The spec's `sky` tile's track (`free06`): the map the ride is lit and dressed from, and whose saved
/// markers and setup a ride on the world starts from.
pub fn home_track(id: &str) -> Option<&'static str> {
    let (_, spec) = SPECS.iter().find(|(w, _)| *w == id)?;
    let sky = Json::parse(spec).ok()?.get("sky").and_then(Json::str)?.to_string();
    let track = sky.strip_prefix("mxu_")?;
    ["free06", "free07", "free09", "free10", "free11"].into_iter().find(|t| *t == track)
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

/// World `id` off `disc`, its joins built under `mods` the first time (`say` hears that it is
/// building, which takes a while) and read from there after.
pub fn world(disc: &mut Disc, mods: &Path, id: &str, say: &mut dyn FnMut(&str)) -> Result<WorldMod, String> {
    let (_, spec) = SPECS.iter().find(|(w, _)| *w == id).ok_or_else(|| format!("{id}: not a disc world"))?;
    let spec_json = Json::parse(spec).map_err(|e| format!("{id}.json: {e}"))?;
    let tiles: Vec<String> =
        spec_json.get("tiles").and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::str).map(str::to_string).collect();
    let mut maps: Vec<Map> = tiles.iter().map(|t| read_map(disc, t)).collect::<Result<_, _>>()?;
    let root = cache_dir(mods, id);
    let key = key(spec, &maps);
    let dir = root.join(&key);
    if !dir.join("mod.json").is_file() {
        say(&format!("{id}: building its joins from the disc's maps, once (a minute or two)"));
        let tiles: Vec<stitch::Tile> = maps
            .iter()
            .map(|m| stitch::Tile::from_parts(&m.tile, meta(&m.tile), &m.scene_file, m.scene.clone(), &m.terrain_file, &m.terrain))
            .collect::<Result<_, _>>()?;
        let params = stitch::build::Params::from_spec(&spec_json).map_err(|e| format!("{id}.json: {e}"))?;
        let part = root.join(format!("{key}.part-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&part);
        stitch::build::build_tiles(&tiles, &part, &params, false)?;
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
    let mut w = worlds::read(&dir, id, &json, Some(&mut open))?;
    debug_assert!(w.pieces[..w.tile_count].iter().all(|p| matches!(p.origin, Origin::Disc { .. })));
    w.dir = dir;
    Ok(w)
}
