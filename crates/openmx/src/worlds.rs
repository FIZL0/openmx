//! World mods (MD8): several track mods laid side by side as one ground, with join terrains between.
//!
//! The layout is openmx-mods' standard §3: `mods/tracks/<id>/mod.json` of kind `"world"`, a complete
//! track mod folder for each tile under `tiles/<tile>/`, and a generated terrain for each join under
//! `joins/<join>/<join>.ps2.trn`. This module reads that into the simulation's [`Tiled`] — which asks
//! each tile's own terrain in the tile's own coordinates — and a [`Source`] that reads a tile's
//! terrain and collision shapes from its folder when the ride first needs them.
//!
//! What the shell draws and lights a world with is one tile's, the `sky` tile ([`WorldMod::home`]):
//! its archive is the ride's first, so its sky, fog, lights and the props it places (moved by its
//! offset) are the ride's. Every tile's ground is drawn ([`crate::ground`]'s chunks, moved and cut to
//! what each owns), unpainted.

use crate::track_mods::{self, Json, ModTrack};
use game_render::disc::Res;
use game_sim::tiles::{Piece, Reach, Source, Tile, Tiled};
use game_sim::track::{self, Placement, Terrain, TerrainHead, TerrainLayout, TerrainNode};
use game_render::gpu::{GroundChunk, GroundPiece};
use game_render::mat::v3;
use ps2_float::F;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Condvar, Mutex};
use crate::clock::Instant;

/// One tile or join of a world, as the world's file and its own files say.
#[derive(Clone, Debug)]
pub struct WorldPiece {
    /// The simulation's statement of it.
    pub tile: Tile,
    /// Its folder.
    pub dir: PathBuf,
    /// A tile's scene, with the `drop`ped sections taken out; `None` for a join.
    pub scene: Option<track::Scene>,
    /// The scene's file stem (`MC01`), or the join's name.
    pub stem: String,
    /// The scene's sections the world's `drop` takes out, for what draws it too (MD15).
    pub dropped: Vec<String>,
    /// A tile's scene file as [`find`] read it, before the `drop`: what the ride dresses and lights
    /// the tile from, so the file is not read again (MD18). `None` for a join.
    pub scene_bytes: Option<Arc<[u8]>>,
    /// Where its files are read from: its folder, or the disc's own archives (P71).
    pub origin: Origin,
}

/// Where a piece's files come from.
#[derive(Clone, Debug)]
pub enum Origin {
    /// A world mod's: the piece's folder, [`WorldPiece::dir`].
    Folder,
    /// A disc track's (P71): its archive (`tracks\free06.res`) and the second one the track is
    /// ridden with (`tracks\free06_2.res`), read off the configured disc when they are needed.
    Disc { track: String, second: Option<String> },
    /// A join's terrain made in memory (P71), not read from anywhere.
    Made(Arc<[u8]>),
}

impl WorldPiece {
    /// The archives its files are read out of, the first its own: its folder, or its disc track's two.
    pub fn archives(&self) -> Result<Vec<Res>, String> {
        match &self.origin {
            Origin::Folder => Ok(vec![Res::folder(&self.dir)?]),
            Origin::Disc { track, second } => {
                let mut disc = game_render::disc::Disc::open_configured()?;
                let mut out = vec![disc.res(track)?];
                if let Some(second) = second {
                    out.push(disc.res(second)?);
                }
                Ok(out)
            }
            Origin::Made(_) => Err(format!("{} is a made join, with no archive", self.stem)),
        }
    }

    /// Its own archive, [`WorldPiece::archives`]' first.
    pub fn res(&self) -> Result<Res, String> {
        Ok(self.archives()?.swap_remove(0))
    }

    /// Its terrain file's bytes.
    pub fn terrain(&self) -> Result<Vec<u8>, String> {
        match (&self.origin, &self.scene) {
            (Origin::Made(bytes), _) => Ok(bytes.to_vec()),
            (_, None) => join_terrain(&self.dir, &self.stem),
            (_, Some(scene)) => track_mods::terrain_bytes(&self.res()?, &scene.terrain_file),
        }
    }
}

/// A world mod that reads.
#[derive(Clone, Debug)]
pub struct WorldMod {
    pub id: String,
    pub name: String,
    pub dir: PathBuf,
    /// Tiles, then joins, in the file's order: the order a point's owner is looked for in.
    pub pieces: Vec<WorldPiece>,
    /// How many of `pieces` are tiles.
    pub tile_count: usize,
    /// The tile whose archive lights and dresses the ride (`sky`, the first tile by default).
    pub home: usize,
    /// Where the ride starts: a tile's own spawn moved by its offset, or the world's own in world feet.
    pub spawn: Spawn,
    /// Which axes wrap round, `[x, z]` (MD29): the world file's `wrap`, off unless it says so. A
    /// wrapped world's last pieces are its seam joins ([`WorldMod::close_seams`]).
    pub wrap: [bool; 2],
}

#[derive(Clone, Debug)]
pub enum Spawn {
    Tile(usize),
    At([String; 3], [String; 3]),
}

/// Whether `mods/tracks/<id>` is a world mod: its `mod.json` says `"kind": "world"`.
pub fn is_world(mods: &Path, id: &str) -> bool {
    let path = mods.join("tracks").join(id).join("mod.json");
    let Ok(text) = std::fs::read_to_string(path) else { return false };
    Json::parse(&text).ok().and_then(|j| j.get("kind").and_then(Json::str).map(|k| k == "world")).unwrap_or(false)
}

/// The world mods under `mods` for the track pick, `(id, name)` in folder-name order: only each
/// `mod.json` is read, since [`find`] reads every tile's terrain and the ride checks the rest.
pub fn listed(mods: &Path) -> Vec<(String, String)> {
    let Ok(list) = std::fs::read_dir(mods.join("tracks")) else { return Vec::new() };
    let mut out: Vec<(String, String)> = list
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().into_string().ok()?;
            let text = std::fs::read_to_string(e.path().join("mod.json")).ok()?;
            let json = Json::parse(&text).ok()?;
            let string = |key: &str| json.get(key).and_then(Json::str);
            let name = string("name")?.trim();
            (string("kind")? == "world" && string("id")? == id && track_mods::valid_id(&id) && !name.is_empty()).then(|| (id.clone(), name.to_string()))
        })
        .collect();
    out.sort();
    out
}

/// Two numbers of a JSON array as whole feet, or as samples.
fn pair(v: Option<&Json>, what: &str) -> Result<[f64; 2], String> {
    match v {
        Some(Json::Arr(a)) if a.len() == 2 => {
            let n: Vec<f64> = a.iter().filter_map(|n| n.num().and_then(|s| s.parse().ok())).collect();
            n.try_into().map_err(|_| format!("{what} is not two numbers"))
        }
        _ => Err(format!("{what} is not two numbers")),
    }
}

/// Take the `[Section]`s named in `drop` out of a scene's text: their header and every line up to
/// the next header.
pub(crate) fn without_sections(text: &str, drop: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skipping = false;
    for line in text.split_inclusive('\n') {
        if let Some(name) = line.trim().strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            skipping = drop.iter().any(|d| d.eq_ignore_ascii_case(name.trim()));
        }
        if !skipping {
            out.push_str(line);
        }
    }
    out
}

/// The one `.scn` in a tile's folder: its path in the folder, its file stem, and its bytes.
pub(crate) fn scene_of(res: &Res) -> Result<(String, String, Vec<u8>), String> {
    let scenes: Vec<&str> = res.entries().iter().map(|e| e.name.as_str()).filter(|n| n.to_ascii_lowercase().ends_with(".scn")).collect();
    let [scn] = scenes.as_slice() else { return Err(format!("{} .scn files, and a tile has one", scenes.len())) };
    let stem = scn.rsplit(['\\', '/']).next().unwrap_or(scn);
    let stem = stem[..stem.len() - 4].to_string();
    Ok((scn.to_string(), stem, res.read(scn)?))
}

/// A terrain's spacing, tree levels and the root cells its tree has, across and along, read from
/// the file's header and its tree's root alone (MD18): [`TerrainHead::MOST`] bytes at most, the root
/// node and its children's table — a few kilobytes of a file of megabytes. What [`shape`] makes of
/// the whole file, without the check that its blocks fill the space in front of the tree, which the
/// ride's own read of the file makes.
fn shape_of_file(path: &Path) -> Result<(F, u32, u32, u32), String> {
    use std::io::{Read, Seek, SeekFrom};
    let name = || path.display().to_string();
    let mut file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", name()))?;
    let len = file.metadata().map_err(|e| format!("{}: {e}", name()))?.len();
    let mut read_at = |at: u64, n: usize| -> Result<Vec<u8>, String> {
        let n = len.saturating_sub(at).min(n as u64) as usize;
        let mut out = vec![0; n];
        file.seek(SeekFrom::Start(at)).and_then(|_| file.read_exact(&mut out)).map_err(|e| format!("{}: {e}", name()))?;
        Ok(out)
    };
    let head = TerrainHead::parse(&read_at(0, TerrainHead::MOST)?)?;
    head.fits(len)?;
    let root = TerrainNode::read(&read_at(u64::from(head.root_at), TerrainNode::BYTES)?).ok_or("no tree")?;
    let (mut w, mut b) = (0, 0);
    if root.children_at != 0 {
        let table = read_at(u64::from(root.children_at), TerrainNode::TABLE_BYTES)?;
        for cz in 0..track::TREE_ARITY {
            for cx in 0..track::TREE_ARITY {
                let at = (cx + cz * track::TREE_ARITY) as usize * 4;
                if table.get(at..at + 4).is_some_and(|w| w != [0; 4]) {
                    w = w.max(cx + 1);
                    b = b.max(cz + 1);
                }
            }
        }
    }
    Ok((head.scale, head.levels, w.max(1), b.max(1)))
}

/// A terrain's spacing, tree levels and the root cells its tree has, across and along, out of the
/// whole file: what [`shape_of_file`] reads from its header and root alone. A disc tile's (P232),
/// whose file is in an archive.
fn shape(bytes: &[u8]) -> Result<(F, u32, u32, u32), String> {
    let t = Terrain::parse(bytes)?;
    let root = t.root().ok_or("no tree")?;
    let (mut w, mut b) = (0, 0);
    for cz in 0..track::TREE_ARITY {
        for cx in 0..track::TREE_ARITY {
            if t.child(&root, cx, cz).is_some() {
                w = w.max(cx + 1);
                b = b.max(cz + 1);
            }
        }
    }
    Ok((t.scale, t.levels, w.max(1), b.max(1)))
}

/// One world mod by its id: its `mod.json`, every tile's scene, and every terrain's header and its
/// tree's root (MD18: [`shape_of_file`]) read and checked, nothing else loaded. The scenes are kept
/// for the ride ([`WorldPiece::scene_bytes`]); a terrain whose blocks do not read is the ride's to
/// find, a tile's as `has no ground` and the home tile's as the ride's own error.
pub fn find(mods: &Path, id: &str) -> Result<WorldMod, String> {
    let dir = mods.join("tracks").join(id);
    if !track_mods::valid_id(id) || !dir.is_dir() {
        return Err(format!("no world mod {id} in {}", mods.join("tracks").display()));
    }
    let text = std::fs::read_to_string(dir.join("mod.json")).map_err(|e| format!("{id}: mod.json: {e}"))?;
    let json = Json::parse(&text).map_err(|e| format!("{id}: mod.json: {e}"))?;
    read(&dir, id, &json, None)
}

/// [`find`], saying to `watch` how many of its tiles and joins are read (P260): a shell's loading
/// line, and its cancel, which is `watch`'s error.
pub fn find_watched(mods: &Path, id: &str, watch: Watch) -> Result<WorldMod, String> {
    let dir = mods.join("tracks").join(id);
    if !track_mods::valid_id(id) || !dir.is_dir() {
        return Err(format!("no world mod {id} in {}", mods.join("tracks").display()));
    }
    let text = std::fs::read_to_string(dir.join("mod.json")).map_err(|e| format!("{id}: mod.json: {e}"))?;
    let json = Json::parse(&text).map_err(|e| format!("{id}: mod.json: {e}"))?;
    read_watched(&dir, id, &json, None, watch)
}

/// What a world's reading tells as it goes, `(what, n, of)`; its error stops the read.
pub type Watch<'a> = &'a (dyn Fn(&str, usize, usize) -> Result<(), String> + Sync);

/// The line a world's reading says.
pub const READING: &str = "Reading the world";

/// Where a world's tile's archives are when they are not its folder's (P232, `crate::disc_worlds`):
/// for a tile's name, its own archive opened, its disc member (`Tracks\free06.res`) and the second
/// one the track is ridden with.
pub type DiscTile<'a> = &'a mut dyn FnMut(&str) -> Result<(Res, String, Option<String>), String>;

/// [`find`]'s reading of a world's `mod.json` (`json`, in `dir`): its tiles from `dir/tiles/<tile>/`,
/// or, given `disc`, each from the disc's own archives as [`Origin::Disc`] (P232); its joins from
/// `dir/joins/<join>/` either way.
pub fn read(dir: &Path, id: &str, json: &Json, disc: Option<DiscTile>) -> Result<WorldMod, String> {
    read_watched(dir, id, json, disc, &|_, _, _| Ok(()))
}

/// [`read`], saying to `watch` how many of the tiles and joins are read.
pub fn read_watched(dir: &Path, id: &str, json: &Json, mut disc: Option<DiscTile>, watch: Watch) -> Result<WorldMod, String> {
    let dir = dir.to_path_buf();
    let string = |key: &str| json.get(key).and_then(Json::str).ok_or_else(|| format!("{id}: mod.json has no {key}"));
    if string("kind")? != "world" {
        return Err(format!("{id}: not a world"));
    }
    if string("id")? != id {
        return Err(format!("{id}: mod.json's id is not the folder's name"));
    }
    let name = string("name")?.to_string();
    let list = |key: &str| match json.get(key) {
        Some(Json::Arr(a)) => Ok(a.clone()),
        None => Ok(Vec::new()),
        _ => Err(format!("{id}: mod.json: {key} is not a list")),
    };
    let drop = list("drop")?;
    let mut pieces = Vec::new();
    let mut first_owner = 0i32;
    let (tiles, joins) = (list("tiles")?, list("joins")?);
    let of = tiles.len() + joins.len();
    for (n, t) in tiles.into_iter().enumerate() {
        watch(READING, n, of)?;
        let tile = t.get("tile").and_then(Json::str).ok_or_else(|| format!("{id}: a tile with no name"))?.to_string();
        let at = pair(t.get("at"), &format!("{id}: {tile}'s at"))?;
        let keep = match t.get("keep") {
            Some(Json::Arr(a)) if a.len() == 4 => {
                let n: Vec<u32> = a.iter().filter_map(|n| n.num().and_then(|s| s.parse::<f64>().ok())).map(|n| n as u32).collect();
                <[u32; 4]>::try_from(n).map_err(|_| format!("{id}: {tile}'s keep is not four numbers"))?
            }
            _ => return Err(format!("{id}: {tile}'s keep is not four numbers")),
        };
        let (res, folder, origin) = match disc.as_mut() {
            None => {
                let folder = dir.join("tiles").join(&tile);
                if !folder.is_dir() {
                    return Err(format!("{id}: {tile} is not installed: the world needs {}'s files", needs(json)));
                }
                (Res::folder(&folder).map_err(|e| format!("{id}: {tile}: {e}"))?, folder, Origin::Folder)
            }
            Some(open) => {
                let (res, track, second) = open(&tile).map_err(|e| format!("{id}: {tile}: {e}"))?;
                (res, PathBuf::from(&track), Origin::Disc { track, second })
            }
        };
        let (scn, stem, bytes) = scene_of(&res).map_err(|e| format!("{id}: {tile}: {e}"))?;
        let sections: Vec<String> = drop
            .iter()
            .filter(|d| d.get("tile").and_then(Json::str) == Some(tile.as_str()))
            .filter_map(|d| d.get("section").and_then(Json::str).map(str::to_string))
            .collect();
        let text = String::from_utf8_lossy(&bytes);
        let mut scene = track::Scene::parse(without_sections(&text, &sections).as_bytes()).map_err(|e| format!("{id}: {tile}: {scn}: {e}"))?;
        let (spacing, levels, _, _) = match origin {
            Origin::Folder => {
                let terrain = track_mods::terrain_path(&res, &folder, &scene.terrain_file).map_err(|e| format!("{id}: {tile}: {e}"))?;
                shape_of_file(&terrain)
            }
            _ => track_mods::terrain_bytes(&res, &scene.terrain_file).and_then(|b| shape(&b)),
        }
        .map_err(|e| format!("{id}: {tile}: {e}"))?;
        // What stands outside the keep stands on ground the tile no longer owns (MD27): a shape
        // there is an invisible wall on a join or a neighbour, so neither it nor a model is kept.
        let kept = keep_feet(keep, spacing);
        scene.collision.retain(|p| inside(kept, p.position[0].0, p.position[2].0));
        scene.models.retain(|p| inside(kept, p.position[0].0, p.position[2].0));
        let t = Tile {
            name: tile.clone(),
            at: at.map(|a| F(a as f32)),
            layout: scene.terrain,
            spacing,
            levels,
            keep,
            first_owner,
        };
        first_owner = first_owner.saturating_add(scene.collision.len() as i32);
        pieces.push(WorldPiece { tile: t, dir: folder, scene: Some(scene), stem, dropped: sections, scene_bytes: Some(bytes.into()), origin });
    }
    let tile_count = pieces.len();
    if tile_count == 0 {
        return Err(format!("{id}: a world with no tiles"));
    }
    for (n, j) in joins.into_iter().enumerate() {
        if n % 32 == 0 {
            watch(READING, tile_count + n, of)?;
        }
        let join = j.get("join").and_then(Json::str).ok_or_else(|| format!("{id}: a join with no name"))?.to_string();
        let at = pair(j.get("at"), &format!("{id}: {join}'s at"))?;
        let folder = dir.join("joins").join(&join);
        let (spacing, levels, width, breadth) = shape_of_file(&join_terrain_path(&folder, &join)).map_err(|e| format!("{id}: {join}: {e}"))?;
        let layout = TerrainLayout { width, breadth, width_scale: spacing, breadth_scale: spacing, ..TerrainLayout::default() };
        let mut t = Tile { name: join.clone(), at: at.map(|a| F(a as f32)), layout, spacing, levels, keep: [0; 4], first_owner };
        t.keep = t.every_cell();
        pieces.push(WorldPiece { tile: t, dir: folder, scene: None, stem: join, dropped: Vec::new(), scene_bytes: None, origin: Origin::Folder });
    }
    let named = |tile: &str| pieces[..tile_count].iter().position(|p| p.tile.name == tile);
    let home = match json.get("sky").and_then(Json::str) {
        Some(s) => named(s).ok_or_else(|| format!("{id}: sky {s} is not one of its tiles"))?,
        None => 0,
    };
    let spawn = match json.get("spawn") {
        None => Spawn::Tile(0),
        Some(s) => match s.get("tile").and_then(Json::str) {
            Some(t) => Spawn::Tile(named(t).ok_or_else(|| format!("{id}: spawn tile {t} is not one of its tiles"))?),
            None => {
                let three = |key: &str| -> Result<[String; 3], String> {
                    match s.get(key) {
                        Some(Json::Arr(v)) if v.len() == 3 => v
                            .iter()
                            .filter_map(|n| n.num().map(str::to_string))
                            .collect::<Vec<_>>()
                            .try_into()
                            .map_err(|_| format!("{id}: spawn {key} is not three numbers")),
                        _ => Err(format!("{id}: spawn {key} is not three numbers")),
                    }
                };
                Spawn::At(three("position")?, three("direction")?)
            }
        },
    };
    let wrap = wrap_switch(json.get("wrap")).map_err(|e| format!("{id}: mod.json: {e}"))?;
    let mut world = WorldMod { id: id.to_string(), name, dir, pieces, tile_count, home, spawn, wrap: [false, false] };
    if wrap != [false, false] {
        world.close_seams(wrap).map_err(|e| format!("{id}: {e}"))?;
        // A wrapped world starts as far from its seams as it can (MD50): on the middle tile, whatever
        // tile the file names. A start given in world feet is the file's to keep.
        if let Spawn::Tile(_) = world.spawn {
            if let Some(i) = world.middle_tile().map_err(|e| format!("{id}: {e}"))? {
                world.spawn = Spawn::Tile(i);
            }
        }
    }
    Ok(world)
}

/// The game whose files a world's tiles are (MD97), from its `mod.json`'s `source`: its `games`, or
/// its `game`, or "another game" where it says neither.
fn needs(json: &Json) -> String {
    let source = json.get("source");
    let games: Vec<&str> = match source.and_then(|s| s.get("games")) {
        Some(Json::Arr(a)) => a.iter().filter_map(Json::str).collect(),
        _ => Vec::new(),
    };
    match (games.is_empty(), source.and_then(|s| s.get("game")).and_then(Json::str)) {
        (false, _) => games.join(" and "),
        (true, Some(game)) => game.to_string(),
        (true, None) => "another game".to_string(),
    }
}

/// The world file's `wrap` (MD29): which axes of the world come round, `[x, z]`. Absent, `null`,
/// `false` or `"none"` is neither, the original's world with its edge; `"x"`, `"z"` or `"xz"` names
/// them, `true` is both, and `[true, false]` says each.
pub fn wrap_switch(v: Option<&Json>) -> Result<[bool; 2], String> {
    match v {
        None | Some(Json::Null) | Some(Json::Bool(false)) => Ok([false, false]),
        Some(Json::Bool(true)) => Ok([true, true]),
        Some(Json::Str(s)) => match s.to_ascii_lowercase().as_str() {
            "none" | "" => Ok([false, false]),
            "x" => Ok([true, false]),
            "z" => Ok([false, true]),
            "xz" | "zx" | "both" => Ok([true, true]),
            _ => Err(format!("wrap \"{s}\" is not \"x\", \"z\" or \"xz\"")),
        },
        Some(Json::Arr(a)) if a.len() == 2 => match (&a[0], &a[1]) {
            (Json::Bool(x), Json::Bool(z)) => Ok([*x, *z]),
            _ => Err("wrap is not two true-or-falses".into()),
        },
        _ => Err("wrap is not \"x\", \"z\", \"xz\", true or two true-or-falses".into()),
    }
}

/// Where a join's terrain file is: `<join>.ps2.trn`, or `<join>.trn` where that is not there.
fn join_terrain_path(folder: &Path, join: &str) -> PathBuf {
    let tagged = folder.join(format!("{join}.ps2.trn"));
    if tagged.is_file() { tagged } else { folder.join(format!("{join}.trn")) }
}

/// A tile's `keep` in its own feet, `[x0, z0, x1, z1]`, by the scene's sample spacing.
pub(crate) fn keep_feet(keep: [u32; 4], spacing: F) -> [f32; 4] {
    keep.map(|k| k as f32 * spacing.0)
}

/// Whether a point in a tile's own feet stands inside its kept rectangle.
pub(crate) fn inside(kept: [f32; 4], x: f32, z: f32) -> bool {
    x >= kept[0] && z >= kept[1] && x < kept[2] && z < kept[3]
}

/// Leave out of the home tile's drawn scene what stands outside its keep, as [`find`] leaves it
/// out of the tile's own scene (MD27): the cut band is a neighbour's ground, or the join's. Before
/// [`move_look`], in the tile's own feet; how many went.
pub fn cut_look(look: &mut game_render::scene::Scene, tile: &Tile) -> usize {
    let kept = keep_feet(tile.keep, tile.spacing);
    let before = look.models.len();
    look.models.retain(|m| inside(kept, m.pos.x, m.pos.z));
    before - look.models.len()
}

/// A join's terrain file: `<join>.ps2.trn`, or `<join>.trn`.
fn join_terrain(folder: &Path, join: &str) -> Result<Vec<u8>, String> {
    std::fs::read(folder.join(format!("{join}.ps2.trn")))
        .or_else(|_| std::fs::read(folder.join(format!("{join}.trn"))))
        .map_err(|e| format!("{}: {e}", folder.join(format!("{join}.ps2.trn")).display()))
}

impl WorldMod {
    /// The home tile as a track mod: the archive the ride lights and dresses itself from.
    pub fn home_track(&self) -> ModTrack {
        let p = &self.pieces[self.home];
        ModTrack { id: self.id.clone(), name: self.name.clone(), dir: p.dir.clone(), stem: p.stem.clone(), spawn: None }
    }

    /// The home tile's offset, `x` and `z`.
    pub fn home_at(&self) -> [F; 2] {
        self.pieces[self.home].tile.at
    }

    /// Put the world's spawn into the home scene the ride reads its start from: a tile's own start
    /// moved by the tile's offset, or the world's own.
    pub fn place_spawn(&self, scene: &mut track::Scene) -> Result<(), String> {
        match &self.spawn {
            Spawn::Tile(i) => {
                let p = &self.pieces[*i];
                let own = p.scene.as_ref().ok_or("a join has no spawn")?;
                scene.start = [own.start[0] + p.tile.at[0], own.start[1], own.start[2] + p.tile.at[1]];
                scene.start_direction = own.start_direction;
            }
            Spawn::At(at, dir) => {
                let t = ModTrack { id: String::new(), name: String::new(), dir: PathBuf::new(), stem: String::new(), spawn: Some((at.clone(), dir.clone())) };
                t.place_spawn(scene)?;
            }
        }
        Ok(())
    }

    /// Each disc track tile's sound emitters as [`crate::audio::Sound::set_map_emitters`] reads them
    /// (P71): none for a world mod's, whose `.aud`s name another game's sounds (MD2).
    pub fn map_sounds(&self) -> Vec<crate::audio::MapSounds> {
        let regions = self.regions();
        self.pieces[..self.tile_count]
            .iter()
            .zip(regions.owned)
            .filter_map(|(p, owned)| match &p.origin {
                Origin::Disc { track, .. } => {
                    Some(crate::audio::MapSounds { res: track.clone(), track: p.stem.clone(), at: p.tile.at.map(|a| a.0), owned })
                }
                _ => None,
            })
            .collect()
    }

    /// The simulation's world, reading each piece from its folder when it is first needed.
    pub fn tiled(&self, reach: Reach) -> Result<Tiled, String> {
        let files = Ahead::new(Files { pieces: self.pieces.clone() });
        Tiled::new(self.pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(files), reach)?.wrapped(self.wrap)
    }

    /// **Close the world round** on the axes `wrap` names (MD29): a seam join past the far edge of
    /// each ([`game_sim::tiles::generate::seam_joins`]), made in memory from the ground along the
    /// world's edges, and the world wrapped. The joins' spacing is the coarsest whole spacing of the
    /// pieces that divides each wrapped side, or a foot. Where an edge has no ground (a hole reaching
    /// it), the strip takes the ground in from it toward the middle, up to a cell, or else 0 feet.
    /// A side that ends part way into a foot (a 4.5-foot tile's odd sample, 40099.5 feet) has its
    /// strip start at the whole foot short of the edge: the tile still owns the ground up to its edge
    /// (a tile comes before a join), and the wrapped width, the strip's far end, is whole.
    pub fn close_seams(&mut self, wrap: [bool; 2]) -> Result<(), String> {
        use game_sim::tiles::generate;
        let open = Tiled::new(self.pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(Files { pieces: self.pieces.clone() }), Reach::DESKTOP)?;
        let extent = open.extent().map(|v| v.0);
        let whole = extent.map(|v| v as u32);
        let mut spacings: Vec<u32> = self.pieces.iter().map(|p| p.tile.spacing.0).filter(|s| s.fract() == 0.0 && *s >= 1.0).map(|s| s as u32).collect();
        spacings.sort_unstable_by(|a, b| b.cmp(a));
        let spacing = spacings.into_iter().find(|s| (0..2).all(|a| !wrap[a] || whole[a] % s == 0)).unwrap_or(1);
        let ground = |x: f64, z: f64| {
            // Just inside the far edges, which no piece owns; then in toward the middle, a sixteenth
            // of a cell a step, past a hole at the edge.
            let (x, z) = ((x as f32).clamp(0.0, extent[0] - 0.01), (z as f32).clamp(0.0, extent[1] - 0.01));
            for k in 0..=16u8 {
                let t = f32::from(k) * spacing as f32 / 16.0;
                let step = |v: f32, e: f32| if v < e / 2.0 { v + t } else { v - t };
                if let Some(h) = open.at(F(step(x, extent[0])), F(step(z, extent[1]))) {
                    return (f64::from(h.y.0), h.surface);
                }
            }
            (0.0, 0)
        };
        // P283: the joins ask for a far edge's ground and the near edge's in turn, sample by sample,
        // 35,000 feet apart on world_ultra, and the open world's reach dropped and re-read the tiles
        // under each for every sample: 24 s of its 28 s load. Each edge is walked once here, in
        // order, so a tile is read once, and the joins read their heights from the table.
        let mut seen: HashMap<(u64, u64), (f64, u8)> = HashMap::new();
        for axis in 0..2 {
            if !wrap[axis] {
                continue;
            }
            // The edges across `axis` (0 and the extent), walked along the other axis.
            let other = 1 - axis;
            for edge in [0.0, f64::from(whole[axis])] {
                for k in 0..=whole[other] / spacing {
                    let along = f64::from(k * spacing);
                    let (x, z) = if axis == 0 { (edge, along) } else { (along, edge) };
                    seen.entry((x.to_bits(), z.to_bits())).or_insert_with(|| ground(x, z));
                }
            }
        }
        let seen = std::cell::RefCell::new(seen);
        let remembered = |x: f64, z: f64| *seen.borrow_mut().entry((x.to_bits(), z.to_bits())).or_insert_with(|| ground(x, z));
        let joins = generate::seam_joins(whole, wrap, spacing, &remembered)?;
        let first_owner = self.pieces.iter().map(|p| p.tile.first_owner).max().unwrap_or(0);
        for (mut tile, bytes) in joins {
            tile.first_owner = first_owner;
            let stem = tile.name.clone();
            self.pieces.push(WorldPiece { tile, dir: PathBuf::new(), scene: None, stem, dropped: Vec::new(), scene_bytes: None, origin: Origin::Made(bytes.into()) });
        }
        self.wrap = wrap;
        Ok(())
    }

    /// **The middle tile** (MD50): of the tiles whose own start stands inside their kept rectangle,
    /// the one whose kept rectangle's middle is nearest the middle of the world's extent, so a ride
    /// starting there is as far from a wrapped world's seams as it can be. The first in the file's
    /// order on a tie; `None` where no tile's start is kept. A pure function of the world file and
    /// the tiles' headers, so every seat of an online ride picks the same one.
    pub fn middle_tile(&self) -> Result<Option<usize>, String> {
        let world = Tiled::new(self.pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(Files { pieces: self.pieces.clone() }), Reach::DESKTOP)?;
        let extent = world.wrapped(self.wrap)?.extent().map(|v| f64::from(v.0));
        let mut best: Option<(f64, usize)> = None;
        for (i, p) in self.pieces[..self.tile_count].iter().enumerate() {
            let kept = keep_feet(p.tile.keep, p.tile.spacing);
            let Some(scene) = &p.scene else { continue };
            if !inside(kept, scene.start[0].0, scene.start[2].0) {
                continue;
            }
            let mid = [0, 1].map(|a| f64::from(p.tile.at[a].0) + f64::from(kept[a] + kept[a + 2]) / 2.0);
            let d = (mid[0] - extent[0] / 2.0).powi(2) + (mid[1] - extent[1] / 2.0).powi(2);
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, i));
            }
        }
        Ok(best.map(|(_, i)| i))
    }

    /// The log line for a world that wraps (MD29): which axes, and the seam joins that close it.
    /// `None` for one that does not.
    pub fn wrap_line(&self, tiled: &Tiled) -> Option<String> {
        if self.wrap == [false, false] {
            return None;
        }
        let axes: Vec<String> = (0..2)
            .filter(|&a| self.wrap[a])
            .map(|a| format!("{} every {:.0} feet", ["x", "z"][a], tiled.extent()[a].0))
            .collect();
        let seams = self.pieces.iter().filter(|p| p.stem.starts_with("seam_") && matches!(p.origin, Origin::Made(_))).count();
        Some(format!("world wraps: {}, closed by {seams} seam joins", axes.join(" and ")))
    }

    /// Piece `index`'s terrain file, read from its folder (MD15: the software renderer's ground).
    pub fn piece_terrain(&self, index: usize) -> Result<Vec<u8>, String> {
        Files { pieces: vec![self.pieces.get(index).ok_or("no such piece")?.clone()] }.load_terrain(0)
    }

    /// The log line the ride prints.
    pub fn line(&self, tiled: &Tiled) -> String {
        format!(
            "world: {} ({}), {} tiles and {} joins, {:.0} x {:.0} feet, lit by {}",
            self.id,
            self.name,
            self.tile_count,
            self.pieces.len() - self.tile_count,
            tiled.area.width.0,
            tiled.area.depth.0,
            self.pieces[self.home].tile.name
        )
    }

    /// The line saying where inside the world the edge test would throw (MD27): `none`, or how many
    /// holes and the first few. Only the world's outer edge should throw.
    pub fn holes_line(tiled: &Tiled) -> String {
        let holes = tiled.holes();
        if holes.is_empty() {
            return "world holes: none, so only the outer edge throws".to_string();
        }
        let first: Vec<String> =
            holes.iter().take(4).map(|[x0, z0, x1, z1]| format!("x {:.1}..{:.1} z {:.1}..{:.1}", x0.0, x1.0, z0.0, z1.0)).collect();
        format!("world holes: {} inside, thrown within 150 feet of each: {}", holes.len(), first.join("; "))
    }

    /// `--seams` (MD47): every seam walked a foot a step, a ray down an eighth of a foot to either
    /// side ([`Tiled::seam_walk`]): how many rays found no ground and the first few, and the largest
    /// step in height across a seam and where.
    pub fn seams_line(tiled: &mut Tiled) -> String {
        let w = tiled.seam_walk(F(1.0), F(0.125));
        let first: Vec<String> = w.first_gaps.iter().take(4).map(|[x, z]| format!("{:.1},{:.1}", x.0, z.0)).collect();
        let gaps = if w.gaps == 0 { "no gaps".to_string() } else { format!("{} gaps ({})", w.gaps, first.join("; ")) };
        format!(
            "world seams: {} edges, {} rays, {gaps}, {} places two pieces meet, {} of them a step over 3 inches, steepest step {:.3} ft at {:.1},{:.1} ({} | {}), {:.3} ft across the wrap",
            w.seams, w.points, w.met, w.rough, w.steepest.0, w.steepest_at[0].0, w.steepest_at[1].0, w.steepest_between[0], w.steepest_between[1], w.wrap_steepest.0
        )
    }

    /// `--seams` (MD51): the ground between each map and the next one along `x` and along `z` whose
    /// side faces it (at least [`CROSSING_FACE`] feet of it), across a seam too on a wrapped axis —
    /// how steep the ground is there and how fast its paint changes, so a crossing over the wrap can
    /// be held to the joins stitch makes between two maps. Each is read on 16 lines spread over the
    /// facing span, every [`CROSSING_STEP`] feet from one map's edge to the other's: the slope is the
    /// steepest of any line's steps, the paint the steepest change of the 16 lines' mean colour (one
    /// map's pages are mirrored on across a join, so a single line is its texture's noise), the
    /// largest of the three channels, in levels a foot. One line each.
    pub fn crossings(&self, tiled: &Tiled, paint: &game_render::terrain::WorldPaint) -> Vec<String> {
        let rects = &paint.regions.owned;
        let period = [0, 1].map(|a| if tiled.wraps()[a] { tiled.extent()[a].0 } else { 0.0 });
        let fold = |v: f32, a: usize| if period[a] > 0.0 { v.rem_euclid(period[a]) } else { v };
        // Each map's own colour: its paint's mean over a 32 x 32 grid of its ground.
        let colours: Vec<[f32; 3]> = rects
            .iter()
            .map(|r| {
                let mut m = [0.0f32; 3];
                for (i, j) in (0..32).flat_map(|i| (0..32).map(move |j| (i, j))) {
                    let c = paint.paint(r[0] + (i as f32 + 0.5) / 32.0 * (r[2] - r[0]), r[1] + (j as f32 + 0.5) / 32.0 * (r[3] - r[1]));
                    (0..3).for_each(|k| m[k] += c[k] / 1024.0);
                }
                m
            })
            .collect();
        let mut out = Vec::new();
        for axis in 0..2 {
            let other = 1 - axis;
            for (a, ra) in rects.iter().enumerate() {
                // The nearest map ahead whose side faces this one's, and whether across the seam.
                let mut next: Option<(f32, usize, bool)> = None;
                for (b, rb) in rects.iter().enumerate() {
                    let face = ra[other + 2].min(rb[other + 2]) - ra[other].max(rb[other]);
                    if b == a || face < CROSSING_FACE {
                        continue;
                    }
                    let shifts: &[f32] = if period[axis] > 0.0 { &[0.0, period[axis]] } else { &[0.0] };
                    for &shift in shifts {
                        let gap = rb[axis] + shift - ra[axis + 2];
                        if gap >= 0.0 && next.is_none_or(|n| gap < n.0) {
                            next = Some((gap, b, shift > 0.0));
                        }
                    }
                }
                let Some((gap, b, wrapped)) = next else { continue };
                let rb = rects[b];
                let (lo, hi) = (ra[other].max(rb[other]), ra[other + 2].min(rb[other + 2]));
                let steps = (gap / CROSSING_STEP).ceil().max(1.0) as usize;
                let lines: Vec<f32> = (0..16).map(|k| lo + (k as f32 + 0.5) / 16.0 * (hi - lo)).collect();
                let point = |i: usize, across: f32| {
                    let along = fold(ra[axis + 2] + gap * i as f32 / steps as f32, axis);
                    let across = fold(across, other);
                    if axis == 0 { (along, across) } else { (across, along) }
                };
                let mut slope = 0.0f32;
                let mut height_before: Vec<Option<f32>> = vec![None; lines.len()];
                // The 16 lines' mean paint, and their mean blend of the maps' own colours: before,
                // and the steepest change of each so far and where.
                let mut before: Option<([f32; 3], [f32; 3])> = None;
                let (mut paint_rate, mut blend_rate) = ((0.0f32, 0.0f32), (0.0f32, 0.0f32));
                let step = gap / steps as f32;
                for i in 0..=steps {
                    let (mut mean, mut blend) = ([0.0f32; 3], [0.0f32; 3]);
                    for (k, &across) in lines.iter().enumerate() {
                        let (x, z) = point(i, across);
                        let h = tiled.at(F(x), F(z)).map(|h| h.y.0);
                        if let (Some(h), Some(b)) = (h, height_before[k]) {
                            slope = slope.max((h - b).abs() / step);
                        }
                        height_before[k] = h;
                        let c = paint.paint(x, z);
                        let shares = paint.regions.shares(x, z);
                        for ch in 0..3 {
                            mean[ch] += c[ch] / lines.len() as f32;
                            blend[ch] += shares.iter().zip(&colours).map(|(s, m)| s * m[ch]).sum::<f32>() / lines.len() as f32;
                        }
                    }
                    if let Some((m0, b0)) = before {
                        let along = fold(ra[axis + 2] + gap * i as f32 / steps as f32, axis);
                        let rate = |p: [f32; 3], q: [f32; 3]| (0..3).map(|c| (p[c] - q[c]).abs()).fold(0.0, f32::max) / step;
                        for (r, best) in [(rate(mean, m0), &mut paint_rate), (rate(blend, b0), &mut blend_rate)] {
                            if r > best.0 {
                                *best = (r, along);
                            }
                        }
                    }
                    before = Some((mean, blend));
                }
                let name = |i: usize| &self.pieces[i].tile.name;
                let seam = if wrapped { " across the seam" } else { "" };
                let ax = ["x", "z"][axis];
                out.push(format!(
                    "world crossing: {ax} {} > {}{seam}, {gap:.0} ft of join, slope {slope:.3}, blend {:.3} a foot at {ax} {:.0}, paint {:.3} a foot at {ax} {:.0}",
                    name(a),
                    name(b),
                    blend_rate.0,
                    blend_rate.1,
                    paint_rate.0,
                    paint_rate.1
                ));
            }
        }
        out
    }
}

/// [`WorldMod::crossings`]: the least span of two maps' sides that face each other for the ground
/// between them to count as a crossing, in feet.
pub const CROSSING_FACE: f32 = 200.0;
/// [`WorldMod::crossings`]: feet a step across the ground between two maps, half a disc map's cell.
pub const CROSSING_STEP: f32 = 4.5;

/// The world's pieces on disk, as the simulation's [`Source`].
struct Files {
    pieces: Vec<WorldPiece>,
}

impl Source for Files {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        let p = self.pieces.get(index).ok_or("no such piece")?;
        let Some(scene) = &p.scene else {
            return Ok(Piece { terrain: p.terrain()?, shapes: Vec::new() });
        };
        let res = p.res()?;
        let terrain = track_mods::terrain_bytes(&res, &scene.terrain_file)?;
        // Every shape the scene places, in the tile's own coordinates; one that does not read is left
        // out, as the tile ridden alone leaves it out (`Props::load`), so the numbers are its.
        let shapes = scene
            .collision
            .iter()
            .filter_map(|at: &Placement| {
                let mesh = res.read_suffix(&at.file).and_then(|b| track::Mesh::parse(&b)).ok()?;
                Some((mesh, at.clone()))
            })
            .collect();
        Ok(Piece { terrain, shapes })
    }

    /// The scene's shapes without its terrain (MD14): the plane asks for every tile's at once.
    fn shapes(&mut self, index: usize) -> Result<Vec<(track::Mesh, Placement)>, String> {
        let p = self.pieces.get(index).ok_or("no such piece")?;
        let Some(scene) = &p.scene else { return Ok(Vec::new()) };
        let res = p.res()?;
        Ok(scene
            .collision
            .iter()
            .filter_map(|at: &Placement| {
                let mesh = res.read_suffix(&at.file).and_then(|b| track::Mesh::parse(&b)).ok()?;
                Some((mesh, at.clone()))
            })
            .collect())
    }
}

/// [`Files`] that read the pieces the ride will want on a thread of their own (MD28), so that the
/// ride's thread only takes them. A piece the ride asks for before it is read is waited for, or read
/// there and then if it was not wanted; either way it is the same piece, so the ride is the same.
struct Ahead {
    files: Files,
    /// The pieces wanted and not taken yet.
    asked: HashSet<usize>,
    /// To the reader: the pieces to read. It stops when this is dropped.
    read: std::sync::mpsc::Sender<usize>,
    /// From the reader: the pieces read. One forgotten while it was being read stays until it is
    /// wanted again (a piece at most for each the ride has been near).
    done: Arc<(Mutex<HashMap<usize, Result<Piece, String>>>, Condvar)>,
}

impl Ahead {
    fn new(files: Files) -> Ahead {
        let (read, asks) = channel::<usize>();
        let done: Arc<(Mutex<HashMap<usize, Result<Piece, String>>>, Condvar)> = Arc::default();
        let (pieces, out) = (files.pieces.clone(), done.clone());
        std::thread::spawn(move || {
            let mut files = Files { pieces };
            while let Ok(i) = asks.recv() {
                let piece = files.load(i);
                out.0.lock().expect("the pieces read").insert(i, piece);
                out.1.notify_all();
            }
        });
        Ahead { files, asked: HashSet::new(), read, done }
    }
}

impl Source for Ahead {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        let mut done = self.done.0.lock().expect("the pieces read");
        if self.asked.remove(&index) {
            loop {
                if let Some(piece) = done.remove(&index) {
                    return piece;
                }
                done = self.done.1.wait(done).expect("the pieces read");
            }
        }
        match done.remove(&index) {
            Some(piece) => piece,
            None => {
                drop(done);
                self.files.load(index)
            }
        }
    }

    fn shapes(&mut self, index: usize) -> Result<Vec<(track::Mesh, Placement)>, String> {
        self.files.shapes(index)
    }

    fn want(&mut self, index: usize) {
        if self.asked.insert(index) {
            let _ = self.read.send(index);
        }
    }

    fn ready(&mut self, index: usize) -> bool {
        self.done.0.lock().expect("the pieces read").contains_key(&index)
    }

    fn forget(&mut self, index: usize) {
        if self.asked.remove(&index) {
            self.done.0.lock().expect("the pieces read").remove(&index);
        }
    }
}

/// Move what the home tile's scene places and lights into the world, by the tile's offset.
pub fn move_look(look: &mut game_render::scene::Scene, by: [F; 2]) {
    let (dx, dz) = (by[0].0, by[1].0);
    let shift = |v: &mut game_render::mat::V3| {
        v.x += dx;
        v.z += dz;
    };
    shift(&mut look.start);
    for m in &mut look.models {
        shift(&mut m.pos);
    }
    for l in &mut look.lights {
        shift(&mut l.position);
        shift(&mut l.target);
    }
}

/// One piece's ground as the GPU draws it: its own chunks of `cells` cells, moved by its offset,
/// with every cell it does not own cut out (the simulation's [`Tiled::owner`] at the cell's middle,
/// so the drawn seams are the ridden ones). Chunks are numbered across and along within the piece:
/// the renderer stitches a piece's chunks to each other and to nothing else
/// ([`game_render::gpu::Renderer::add_ground_piece`]).
///
/// The field is the piece's own samples ([`Tile::samples`]), a tree level's sixteen to a root cell:
/// a one-level join meshed at a disc map's 256 a width unit was 256 times its own area of empty
/// cells to walk, which was most of a world's load (MD17).
pub fn piece_chunks(tile: &Tile, index: usize, bytes: &[u8], tiled: &Tiled, cells: u32) -> Result<Vec<GroundChunk>, String> {
    let terrain = Terrain::parse(bytes)?;
    let mut ground = crate::ground::Ground::new(terrain, &tile.layout);
    [ground.samples_x, ground.samples_z] = tile.samples();
    let cells = cells.max(1).next_power_of_two();
    let [ax, az] = tile.at.map(|a| a.0);
    let spacing = ground.spacing;
    // The earlier pieces' terrains, a cell wider all round: only they can own a cell before this
    // one does, so a chunk none of them reaches is cut by its own `keep` alone.
    let earlier: Vec<[f32; 4]> = tiled.tiles[..index].iter().map(|t| rect(t, t.spacing.0)).collect();
    let mut out = Vec::new();
    for mut c in ground.chunks(cells) {
        let (x0, z0) = (c.at.0 * cells, c.at.1 * cells);
        let mut missing = if c.missing.is_empty() { vec![false; (cells * cells) as usize] } else { c.missing.clone() };
        let side = cells as f32 * spacing;
        let (wx, wz) = (ax + x0 as f32 * spacing, az + z0 as f32 * spacing);
        let alone = !earlier.iter().any(|r| r[0] < wx + side && wx < r[2] && r[1] < wz + side && wz < r[3]);
        let [sx_n, sz_n] = tile.samples();
        let [k0, k1, k2, k3] = tile.keep;
        for j in 0..cells {
            for i2 in 0..cells {
                let (sx, sz) = (x0 + i2, z0 + j);
                let owned = if alone {
                    // [`Tile`]'s own test of a cell: inside `keep`, and a cell of the terrain.
                    sx >= k0 && sx < k2 && sz >= k1 && sz < k3 && sx + 1 < sx_n && sz + 1 < sz_n
                } else {
                    owns_cell(tiled, index, [ax, az], spacing, sx, sz)
                };
                if !owned {
                    missing[(j * cells + i2) as usize] = true;
                }
            }
        }
        if missing.iter().all(|&m| m) {
            continue;
        }
        for v in &mut c.vertices {
            v.pos[0] += ax;
            v.pos[2] += az;
        }
        c.centre.x += ax;
        c.centre.z += az;
        c.missing = if missing.iter().any(|&m| m) { missing } else { Vec::new() };
        out.push(c);
    }
    Ok(out)
}

/// How many of a piece's samples a far piece's cell spans each way (P286): its chunks are a
/// sixteenth of the near ones'.
pub const COARSE_BY: u32 = 4;

/// How near the riders a world's GPU ground is built at full detail (P286); the rest within
/// [`Reach::load`] is built at [`COARSE_BY`]. A piece goes fine within `load` and back to coarse
/// past `drop`. world_ultra's 15,423 chunks out to 6000 ft were 600-750 MB.
pub const FINE_REACH: Reach = Reach { load: F(2500.0), drop: F(3500.0) };

/// A piece's chunks ([`piece_chunks`]) at a `by`th of its samples each way (P286): chunks of as many
/// cells, each `by` times as wide. A corner every `by` samples, taken from a chunk that has a drawn
/// cell beside it if one does; a cell left out only when every sample cell under it is, so a cut
/// between two pieces overlaps rather than gaps. `at` and `spacing` are the piece's own, and `last`
/// its last sample each way, which the chunks' corners past the field's edge stop at. Chunks of
/// fewer cells than `by`, or not a multiple of it, are as they were.
pub fn coarse(chunks: Vec<GroundChunk>, by: u32, at: [f32; 2], spacing: f32, last: [u32; 2]) -> Vec<GroundChunk> {
    let Some(cells) = chunks.first().map(|c| c.cells) else { return chunks };
    if by <= 1 || cells < by || cells % by != 0 {
        return chunks;
    }
    let row = cells + 1;
    let index: HashMap<(u32, u32), usize> = chunks.iter().enumerate().map(|(n, c)| (c.at, n)).collect();
    let gone = |c: &GroundChunk, i: u32, j: u32| !c.missing.is_empty() && c.missing[(j * cells + i) as usize];
    // A corner of the piece's sample grid, from a chunk with a drawn cell beside it before one without.
    let corner = |gx: u32, gz: u32| -> Option<game_render::gpu::GroundVertex> {
        let mut any = None;
        for cz in [gz / cells, (gz / cells).wrapping_sub(1)] {
            for cx in [gx / cells, (gx / cells).wrapping_sub(1)] {
                let Some(&n) = index.get(&(cx, cz)) else { continue };
                let (lx, lz) = (gx - cx * cells, gz - cz * cells);
                if lx > cells || lz > cells {
                    continue;
                }
                let c = &chunks[n];
                let v = c.vertices[(lz * row + lx) as usize];
                let beside = [(lx.wrapping_sub(1), lz.wrapping_sub(1)), (lx, lz.wrapping_sub(1)), (lx.wrapping_sub(1), lz), (lx, lz)];
                if beside.iter().any(|&(i, j)| i < cells && j < cells && !gone(c, i, j)) {
                    return Some(v);
                }
                any.get_or_insert(v);
            }
        }
        any
    };
    let mut blocks: Vec<(u32, u32)> = chunks.iter().map(|c| (c.at.0 / by, c.at.1 / by)).collect();
    blocks.sort_unstable_by_key(|&(x, z)| (z, x));
    blocks.dedup();
    let mut out = Vec::with_capacity(blocks.len());
    for (bx, bz) in blocks {
        let span = by * cells;
        let mut vertices = vec![game_render::gpu::GroundVertex::default(); (row * row) as usize];
        let mut found = vec![false; vertices.len()];
        for j in 0..row {
            for i in 0..row {
                let (gx, gz) = (bx * span + i * by, bz * span + j * by);
                let k = (j * row + i) as usize;
                match corner(gx, gz) {
                    Some(v) => (vertices[k], found[k]) = (v, true),
                    None => {
                        let (x, z) = (gx.min(last[0]), gz.min(last[1]));
                        vertices[k] = game_render::gpu::GroundVertex { pos: [at[0] + x as f32 * spacing, 0.0, at[1] + z as f32 * spacing], normal: [0.0, 1.0, 0.0] };
                    }
                }
            }
        }
        // Each cell lies in one sample chunk: left out when that chunk is, or every cell of it under this one is.
        let mut missing = vec![false; (cells * cells) as usize];
        for j in 0..cells {
            for i in 0..cells {
                let (gx, gz) = (bx * span + i * by, bz * span + j * by);
                missing[(j * cells + i) as usize] = match index.get(&(gx / cells, gz / cells)) {
                    None => true,
                    Some(&n) => {
                        let (lx, lz) = (gx % cells, gz % cells);
                        (0..by).all(|dz| (0..by).all(|dx| gone(&chunks[n], lx + dx, lz + dz)))
                    }
                };
            }
        }
        if missing.iter().all(|&m| m) {
            continue;
        }
        let (mut lo, mut hi) = (v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN));
        for (v, _) in vertices.iter().zip(&found).filter(|(_, f)| **f) {
            let p = v3(v.pos[0], v.pos[1], v.pos[2]);
            lo = v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        // As a sample chunk's: a corner nothing has is only a left-out cell's, on the chunk's floor.
        for (v, _) in vertices.iter_mut().zip(&found).filter(|(_, f)| !**f) {
            v.pos[1] = lo.y;
        }
        let centre = lo.add(hi).scale(0.5);
        out.push(GroundChunk {
            vertices,
            cells,
            at: (bx, bz),
            missing: if missing.iter().any(|&m| m) { missing } else { Vec::new() },
            centre,
            radius: hi.sub(centre).len(),
        });
    }
    out
}

/// Whether piece `index` draws its cell `(sx, sz)` (its own samples, its offset `at`): the piece
/// that owns the cell's middle does, as the ride's ground asks it ([`Tiled::owner`]), and across a
/// seam between two grids that do not line up (a 4.5-foot map beside a 9-foot join, MD15) so does
/// one that owns any corner of it, a twentieth of a cell in, so the two pieces' cells overlap there
/// rather than leave a gap. Where the grids line up every point of a cell has the middle's owner, and
/// the cut is the middle's alone.
pub fn owns_cell(tiled: &Tiled, index: usize, at: [f32; 2], spacing: f32, sx: u32, sz: u32) -> bool {
    let owner = |fx: f32, fz: f32| tiled.owner(F(at[0] + (sx as f32 + fx) * spacing), F(at[1] + (sz as f32 + fz) * spacing)).map(|(o, _, _)| o);
    if owner(0.5, 0.5) == Some(index) {
        return true;
    }
    [(0.05, 0.05), (0.95, 0.05), (0.05, 0.95), (0.95, 0.95)].iter().any(|&(fx, fz)| owner(fx, fz) == Some(index))
}

/// A piece's terrain rectangle in world feet, `[x0, z0, x1, z1]`, `margin` wider all round.
fn rect(t: &Tile, margin: f32) -> [f32; 4] {
    let [sx, sz] = t.samples();
    let [ax, az] = t.at.map(|a| a.0);
    [ax - margin, az - margin, ax + sx as f32 * t.spacing.0 + margin, az + sz as f32 * t.spacing.0 + margin]
}

/// How far a world point is from a piece's terrain, the larger of the two axes' gaps: 0 over it.
fn gap(t: &Tile, x: f32, z: f32) -> f32 {
    let [x0, z0, x1, z1] = rect(t, 0.0);
    (x0 - x).max(x - x1).max(0.0).max((z0 - z).max(z - z1).max(0.0))
}

/// A world's ground on the GPU a piece at a time (MD17), as [`Tiled::stream`] keeps the
/// simulation's: the pieces within [`Reach::load`] of a rider are built before the ride, the rest on
/// a thread of their own nearest first as the riders come within reach, and a piece past
/// [`Reach::drop`] of every rider is let go. The gap between the two is the margin that keeps a
/// piece at the edge from being built again every frame.
///
/// A piece is built from its folder by [`piece_chunks`], the same chunks whenever it is built. With a
/// `fine` reach (P286) only the pieces within it are, and the rest [`coarse`]: a piece is built again
/// when it crosses `fine.load` coming in or `fine.drop` going out.
pub struct GroundStream {
    tiles: Vec<Tile>,
    reach: Reach,
    fine: Option<Reach>,
    /// Each piece's ground on the GPU, and whether at full detail.
    held: Vec<Option<bool>>,
    /// Each piece asked of the builder or being built, and whether at full detail.
    asked: Vec<Option<bool>>,
    /// The pieces whose files would not build: not asked again, as they do not change under a ride.
    failed: Vec<bool>,
    /// The pieces held whose ground may not have reached the GPU (MD97: iOS refuses GPU work from an
    /// app sent to the background), built and handed over again as they are.
    redo: Vec<bool>,
    /// What the builder has to do, nearest last (each piece at full detail or not), and whether it
    /// should stop.
    work: Arc<(Mutex<(Vec<(usize, bool)>, bool)>, Condvar)>,
    built: Receiver<(usize, bool, Result<GroundPiece, String>)>,
    /// Built pieces' parts not handed over yet, each piece's first part marked: a frame hands over
    /// `chunks_a_frame` at most.
    pending: VecDeque<(usize, GroundPiece, bool)>,
    /// The most chunks a frame hands over, [`CHUNKS_A_FRAME`] unless the shell says
    /// (`--world-chunks`).
    pub chunks_a_frame: usize,
    /// What building and dropping did since the shell last took the lines.
    log: Vec<String>,
    /// Each rider's place when its heading was last taken, and the heading (V92).
    heading: Vec<([f32; 2], [f32; 2])>,
}

/// The most ground chunks a [`GroundStream::update`] hands over, but for a piece a rider is on
/// (MD28: a national's 1444 chunks took 5-10 ms to write in one frame).
pub const CHUNKS_A_FRAME: usize = 256;

/// How long a frame spends uploading a tile dressed mid-ride's models and textures (MD28): the
/// rest wait for the next frame, and the tile is drawn once they are all up.
pub const UPLOADS_A_FRAME: std::time::Duration = std::time::Duration::from_millis(2);

/// What a [`GroundStream`] asks of the renderer: a piece's chunks to add, more of its chunks, or a
/// piece to drop.
pub enum Change {
    Add(usize, GroundPiece),
    /// More of a piece added already ([`game_render::gpu::Renderer::more_ground_piece`]).
    More(usize, GroundPiece),
    Drop(usize),
    /// A piece all of whose chunks are handed over now (V92), to be shown over its stand-in.
    Show(usize),
}

/// How far a rider goes before its heading is taken again (V92), in feet.
const HEADING_FEET: f32 = 50.0;

/// How a piece ahead of a rider is put before one as near behind it (V92): its gap counts for
/// `1 - AHEAD * cos` of the angle between the rider's heading and the way to the piece.
const AHEAD: f32 = 0.5;

/// A move between headings taken too far to have been ridden (V92), in feet.
const TELEPORT_FEET: f32 = 2000.0;

impl GroundStream {
    /// The stream, and the pieces it built before the ride: those within `reach.load` of `near`,
    /// or with `all` every one (the whole world before the first frame, as before MD17). The
    /// pieces are built on every core, all at full detail.
    pub fn start(w: &WorldMod, cells: u32, reach: Reach, near: &[[f32; 2]], all: bool) -> (GroundStream, Vec<(usize, GroundPiece)>) {
        let mut out = Vec::new();
        let stream = GroundStream::start_into(w, cells, reach, None, near, all, &mut |i, piece| out.push((i, piece)));
        (stream, out)
    }

    /// [`GroundStream::start`], each piece built before the ride handed to `take` as it is built, in
    /// the pieces' order (P283: world_ultra's 88 first pieces held at once were 830 MB), and those
    /// past `fine`'s load [`coarse`] (P286). `all` builds every piece at full detail.
    pub fn start_into(w: &WorldMod, cells: u32, reach: Reach, fine: Option<Reach>, near: &[[f32; 2]], all: bool, take: &mut dyn FnMut(usize, GroundPiece)) -> GroundStream {
        let tiles: Vec<Tile> = w.pieces.iter().map(|p| p.tile.clone()).collect();
        let fine = fine.filter(|_| !all);
        let first: Vec<(usize, bool)> = (0..tiles.len())
            .filter_map(|i| {
                let gap = nearest(&tiles[i], near);
                (all || gap <= reach.load.0).then(|| (i, fine.is_none_or(|f| gap <= f.load.0)))
            })
            .collect();
        let threads = crate::thread::available_parallelism().map_or(4, |n| n.get()).min(first.len()).max(1);
        let work = Arc::new((Mutex::new((Vec::new(), false)), Condvar::new()));
        let (send, built) = channel();
        let n = tiles.len();
        let mut stream = GroundStream {
            held: vec![None; n],
            asked: vec![None; n],
            failed: vec![false; n],
            redo: vec![false; n],
            tiles,
            reach,
            fine,
            work,
            built,
            pending: VecDeque::new(),
            chunks_a_frame: CHUNKS_A_FRAME,
            log: Vec::new(),
            heading: Vec::new(),
        };
        crate::thread::scope(|scope| {
            let (done, ready) = channel();
            for t in 0..threads {
                let (first, pieces, done) = (&first, &w.pieces, done.clone());
                scope.spawn(move || {
                    let mut b = Builder::new(pieces.clone(), reach, cells);
                    for &(i, full) in first.iter().skip(t).step_by(threads) {
                        if done.send((i, b.build(i, full))).is_err() {
                            return;
                        }
                    }
                });
            }
            drop(done);
            // Each handed over once every piece before it is: the builders work through the pieces
            // in turn, so few wait.
            let mut waiting = std::collections::BTreeMap::new();
            let mut next = 0;
            for (i, r) in ready {
                waiting.insert(i, r);
                while next < first.len()
                    && let Some(r) = waiting.remove(&first[next].0)
                {
                    let (i, full) = first[next];
                    if let Some(chunks) = stream.arrived(i, full, r) {
                        take(i, chunks);
                    }
                    next += 1;
                }
            }
        });
        let (pieces, theirs) = (w.pieces.clone(), stream.work.clone());
        std::thread::spawn(move || {
            let mut b = Builder::new(pieces, reach, cells);
            loop {
                let next = {
                    let (lock, wake) = &*theirs;
                    let mut todo = lock.lock().expect("the ground's work");
                    while todo.0.is_empty() && !todo.1 {
                        todo = wake.wait(todo).expect("the ground's work");
                    }
                    if todo.1 {
                        return;
                    }
                    todo.0.pop().expect("work")
                };
                if send.send((next.0, next.1, b.build(next.0, next.1))).is_err() {
                    return;
                }
            }
        });
        stream
    }

    /// A piece the builder finished: held, with its line, or `None` with the line why not.
    fn arrived(&mut self, i: usize, full: bool, built: Result<GroundPiece, String>) -> Option<GroundPiece> {
        let name = &self.tiles[i].name;
        match built {
            Ok(piece) => {
                let coarse = if full { "" } else { " coarse" };
                self.log.push(format!("world ground: {name} built{coarse}, {} chunks", piece.chunks.len()));
                self.held[i] = Some(full);
                self.redo[i] = false;
                Some(piece)
            }
            Err(e) => {
                self.log.push(format!("world ground: {name} not drawn: {e}"));
                self.failed[i] = true;
                self.redo[i] = false;
                None
            }
        }
    }

    /// How piece `i`, `gap` from the riders, should be held: not, coarse or at full detail.
    fn wanted(&self, i: usize, gap: f32) -> Option<bool> {
        let held = self.held[i];
        if self.failed[i] || gap > self.reach.drop.0 || (held.is_none() && gap > self.reach.load.0) {
            return None;
        }
        Some(self.fine.is_none_or(|f| gap <= if held == Some(true) { f.drop.0 } else { f.load.0 }))
    }

    /// Ask for the pieces within reach of `near` nearest first, take what the builder finished, and
    /// let go of the pieces past reach. A piece a rider is over that is not built yet is waited for,
    /// and the wait said.
    pub fn update(&mut self, near: &[[f32; 2]]) -> Vec<Change> {
        let gaps: Vec<f32> = self.tiles.iter().map(|t| nearest(t, near)).collect();
        let order = self.ahead_first(near);
        let mut out = Vec::new();
        {
            let (lock, wake) = &*self.work;
            let mut todo = lock.lock().expect("the ground's work");
            for &(i, _) in &todo.0 {
                self.asked[i] = None;
            }
            let mut want: Vec<(usize, bool)> = (0..self.tiles.len())
                .filter(|&i| self.asked[i].is_none())
                .filter_map(|i| self.wanted(i, gaps[i]).filter(|&full| self.held[i] != Some(full) || self.redo[i]).map(|full| (i, full)))
                .collect();
            want.sort_by(|a, b| order[b.0].total_cmp(&order[a.0]));
            for &(i, full) in &want {
                self.asked[i] = Some(full);
            }
            todo.0 = want;
            wake.notify_one();
        }
        let take = |s: &mut GroundStream, (i, full, r): (usize, bool, Result<GroundPiece, String>)| {
            s.asked[i] = None;
            if gaps[i] > s.reach.drop.0 {
                // Gone past reach while it was built: what the GPU has of it is let go below.
            } else if let Some(piece) = s.arrived(i, full, r) {
                // In place of the parts of it at the other detail not handed over yet.
                s.pending.retain(|p| p.0 != i);
                s.pending.extend(piece.split(s.chunks_a_frame).into_iter().enumerate().map(|(n, part)| (i, part, n == 0)));
            }
        };
        while let Ok(done) = self.built.try_recv() {
            take(self, done);
        }
        // Coarse ground underfoot is waited for too (a teleport into the far ring).
        while let Some(i) = (0..self.tiles.len()).find(|&i| gaps[i] == 0.0 && self.asked[i].is_some() && self.held[i] != Some(true)) {
            let at = Instant::now();
            let Ok(done) = self.built.recv() else { break };
            self.log.push(format!("world ground: waited {:.0} ms for {}", at.elapsed().as_secs_f32() * 1000.0, self.tiles[i].name));
            take(self, done);
        }
        // A frame's worth of parts, the piece ahead first, and all of any piece a rider is on.
        let mut pending: Vec<_> = std::mem::take(&mut self.pending).into();
        pending.sort_by(|a, b| order[a.0].total_cmp(&order[b.0]));
        let mut budget = self.chunks_a_frame;
        for (i, part, first) in pending {
            if part.chunks.len() > budget && gaps[i] > 0.0 {
                self.pending.push_back((i, part, first));
                continue;
            }
            budget = budget.saturating_sub(part.chunks.len());
            out.push(if first { Change::Add(i, part) } else { Change::More(i, part) });
        }
        // The pieces whose last part went this frame.
        let mut whole: Vec<usize> = out.iter().filter_map(|c| if let Change::Add(i, _) | Change::More(i, _) = c { Some(*i) } else { None }).collect();
        whole.sort_unstable();
        whole.dedup();
        whole.retain(|i| !self.pending.iter().any(|p| p.0 == *i));
        out.extend(whole.into_iter().map(Change::Show));
        for i in 0..self.tiles.len() {
            if self.held[i].is_some() && gaps[i] > self.reach.drop.0 {
                self.pending.retain(|p| p.0 != i);
                self.held[i] = None;
                self.redo[i] = false;
                self.log.push(format!("world ground: {} dropped", self.tiles[i].name));
                out.push(Change::Drop(i));
            }
        }
        out
    }

    /// Each piece's place in the stream's order (V92), least first: its gap to the riders, less as
    /// much as [`AHEAD`] for a piece the way a rider is heading, so the tile ahead lands first.
    fn ahead_first(&mut self, near: &[[f32; 2]]) -> Vec<f32> {
        if self.heading.len() != near.len() {
            self.heading = near.iter().map(|&p| (p, [0.0, 0.0])).collect();
        }
        for (h, p) in self.heading.iter_mut().zip(near) {
            let d = [p[0] - h.0[0], p[1] - h.0[1]];
            let l = d[0].hypot(d[1]);
            if l >= HEADING_FEET {
                // A jump of more than a ride goes in a few frames (a respawn, a teleport) has no heading.
                *h = (*p, if l < TELEPORT_FEET { [d[0] / l, d[1] / l] } else { [0.0, 0.0] });
            }
        }
        self.tiles
            .iter()
            .map(|t| {
                let [x0, z0, x1, z1] = rect(t, 0.0);
                near.iter()
                    .zip(&self.heading)
                    .map(|(p, (_, h))| {
                        let g = gap(t, p[0], p[1]);
                        let to = [(x0 + x1) * 0.5 - p[0], (z0 + z1) * 0.5 - p[1]];
                        let l = to[0].hypot(to[1]);
                        let cos = if l > 0.0 { (to[0] * h[0] + to[1] * h[1]) / l } else { 0.0 };
                        g * (1.0 - AHEAD * cos)
                    })
                    .fold(f32::MAX, f32::min)
            })
            .collect()
    }

    /// Build every piece held again and hand it over as it comes, in place of what the GPU has of it
    /// (MD97): after the iPhone app was sent away, when the uploads made as it went may have been
    /// refused. What is drawn stays until each piece's own replaces it. The number of pieces asked.
    pub fn build_again(&mut self) -> usize {
        let mut n = 0;
        for (redo, held) in self.redo.iter_mut().zip(&self.held) {
            if held.is_some() {
                *redo = true;
                n += 1;
            }
        }
        n
    }

    /// How many pieces are held now.
    pub fn held(&self) -> usize {
        self.held.iter().filter(|h| h.is_some()).count()
    }

    /// How many pieces are held [`coarse`] now (P286).
    pub fn held_coarse(&self) -> usize {
        self.held.iter().filter(|&&h| h == Some(false)).count()
    }

    /// The lines building and dropping wrote since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }
}

impl Drop for GroundStream {
    fn drop(&mut self) {
        let (lock, wake) = &*self.work;
        if let Ok(mut todo) = lock.lock() {
            todo.1 = true;
            wake.notify_one();
        }
    }
}

/// The smallest gap from a piece to any of `near`.
pub(crate) fn nearest(t: &Tile, near: &[[f32; 2]]) -> f32 {
    near.iter().map(|p| gap(t, p[0], p[1])).fold(f32::MAX, f32::min)
}

/// What builds pieces on one thread: the world's files, and its owners to cut by.
struct Builder {
    files: Files,
    tiled: Option<Tiled>,
    cells: u32,
}

impl Builder {
    fn new(pieces: Vec<WorldPiece>, reach: Reach, cells: u32) -> Builder {
        // Only its owners are asked, which read nothing.
        let tiled = Tiled::new(pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(Files { pieces: pieces.clone() }), reach).ok();
        Builder { files: Files { pieces }, tiled, cells }
    }

    /// Piece `i`'s chunks, at full detail or [`coarse`].
    fn build(&mut self, i: usize, full: bool) -> Result<GroundPiece, String> {
        let bytes = self.files.load_terrain(i)?;
        let tiled = self.tiled.as_ref().ok_or("the world does not read")?;
        let tile = &self.files.pieces[i].tile;
        let chunks = piece_chunks(tile, i, &bytes, tiled, self.cells)?;
        let chunks = if full {
            chunks
        } else {
            let [sx, sz] = tile.samples();
            coarse(chunks, COARSE_BY, tile.at.map(|a| a.0), tile.spacing.0, [sx.saturating_sub(1), sz.saturating_sub(1)])
        };
        // Its errors and hash here, off the drawing thread (MD28).
        Ok(GroundPiece::of(chunks))
    }
}

impl Files {
    fn load_terrain(&mut self, index: usize) -> Result<Vec<u8>, String> {
        self.pieces[index].terrain()
    }
}

/// The most cells a world's stand-in has (V92): 512 x 512, about 6 MB of corners on the GPU however
/// big the world, at a sixteenth of the finest piece's samples or coarser.
pub const STAND_IN_CELLS: f32 = 262_144.0;

/// A whole world's ground at its coarsest (V92), drawn wherever a piece is not loaded yet so the far
/// tiles are there from the first frame ([`game_render::gpu::Renderer::set_ground_stand_in`]): a grid
/// over every piece's rectangle, each corner the height and normal of the sample nearest it on the
/// piece that owns the place ([`Tiled::owner`]), in chunks of `cells` cells as the pieces' are. A cell
/// no piece owns any corner or the middle of is left out. Every owning piece's terrain is read once,
/// on every core.
pub fn stand_in(w: &WorldMod, tiled: &Tiled, cells: u32) -> Result<game_render::gpu::StandIn, String> {
    let cells = cells.max(1).next_power_of_two();
    let rects: Vec<[f32; 4]> = w.pieces.iter().map(|p| rect(&p.tile, 0.0)).collect();
    let lo = rects.iter().fold([f32::MAX; 2], |a, r| [a[0].min(r[0]), a[1].min(r[1])]);
    let hi = rects.iter().fold([f32::MIN; 2], |a, r| [a[0].max(r[2]), a[1].max(r[3])]);
    let [width, depth] = [hi[0] - lo[0], hi[1] - lo[1]];
    if rects.is_empty() || !(width > 0.0 && depth > 0.0) {
        return Err("a world with no ground".into());
    }
    let finest = w.pieces.iter().map(|p| p.tile.spacing.0).fold(f32::MAX, f32::min);
    // The width a whole number of cells, so a wrapped world's copies meet cell to cell.
    let least = (16.0 * finest).max((width * depth / STAND_IN_CELLS).sqrt());
    let step = width / (width / least).ceil();
    let [nx, nz] = [(width / step).round().max(1.0) as u32, (depth / step).ceil().max(1.0) as u32];
    let row = nx + 1;
    // Each corner's owner, asked a hair inside the world's far sides.
    let at = |gx: u32, gz: u32| {
        let x = (lo[0] + gx as f32 * step).min(hi[0] - 0.5);
        let z = (lo[1] + gz as f32 * step).min(hi[1] - 0.5);
        tiled.owner(F(x), F(z))
    };
    let mut asks: HashMap<usize, Vec<(usize, f32, f32)>> = HashMap::new();
    for gz in 0..=nz {
        for gx in 0..=nx {
            if let Some((i, lx, lz)) = at(gx, gz) {
                asks.entry(i).or_default().push(((gz * row + gx) as usize, lx.0, lz.0));
            }
        }
    }
    let mut owners: Vec<(usize, Vec<(usize, f32, f32)>)> = asks.into_iter().collect();
    owners.sort_by_key(|(i, _)| *i);
    let threads = crate::thread::available_parallelism().map_or(4, |n| n.get()).min(owners.len()).max(1);
    let corners: Vec<(usize, Option<GroundVertexAt>)> = crate::thread::scope(|scope| {
        let jobs: Vec<_> = (0..threads)
            .map(|t| {
                let owners = &owners;
                scope.spawn(move || {
                    let mut out = Vec::new();
                    for (i, points) in owners.iter().skip(t).step_by(threads) {
                        let piece = &w.pieces[*i];
                        let bytes = Files { pieces: vec![piece.clone()] }.load_terrain(0).ok();
                        let terrain = bytes.as_deref().and_then(|b| Terrain::parse(b).ok());
                        for &(v, lx, lz) in points {
                            out.push((v, terrain.as_ref().and_then(|t| sample_near(t, &piece.tile, lx, lz))));
                        }
                    }
                    out
                })
            })
            .collect();
        jobs.into_iter().flat_map(|j| j.join().expect("a stand-in reader")).collect()
    });
    let mut grid = vec![None; (row * (nz + 1)) as usize];
    for (v, c) in corners {
        grid[v] = c;
    }
    // A cell is drawn where a piece owns its middle or a corner; a corner no terrain gave is its
    // found neighbours' mean, so a drawn cell at the edge of the ground does not drop to nothing.
    let drawn: Vec<bool> = (0..nz * nx)
        .map(|n| {
            let (gx, gz) = (n % nx, n / nx);
            let mid = tiled.owner(F((lo[0] + (gx as f32 + 0.5) * step).min(hi[0] - 0.5)), F((lo[1] + (gz as f32 + 0.5) * step).min(hi[1] - 0.5))).is_some();
            mid || [(0, 0), (1, 0), (0, 1), (1, 1)].iter().any(|&(dx, dz)| grid[((gz + dz) * row + gx + dx) as usize].is_some())
        })
        .collect();
    for _ in 0..4 {
        let before = grid.clone();
        for gz in 0..=nz {
            for gx in 0..=nx {
                if before[(gz * row + gx) as usize].is_some() {
                    continue;
                }
                let near: Vec<GroundVertexAt> = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .filter_map(|&(dx, dz): &(i32, i32)| {
                        let (x, z) = (gx as i32 + dx, gz as i32 + dz);
                        (x >= 0 && z >= 0 && x <= nx as i32 && z <= nz as i32).then(|| before[(z as u32 * row + x as u32) as usize]).flatten()
                    })
                    .collect();
                if !near.is_empty() {
                    let k = near.len() as f32;
                    let y = near.iter().map(|c| c.0).sum::<f32>() / k;
                    let n = near.iter().fold([0.0; 3], |a, c| [a[0] + c.1[0], a[1] + c.1[1], a[2] + c.1[2]]);
                    grid[(gz * row + gx) as usize] = Some(GroundVertexAt(y, n));
                }
            }
        }
    }
    let floor = grid.iter().flatten().map(|c| c.0).fold(f32::MAX, f32::min);
    let floor = if floor == f32::MAX { 0.0 } else { floor };
    let mut chunks = Vec::new();
    for cz in 0..nz.div_ceil(cells) {
        for cx in 0..nx.div_ceil(cells) {
            let side = cells + 1;
            let mut vertices = Vec::with_capacity((side * side) as usize);
            let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
            for j in 0..side {
                for i in 0..side {
                    // Past the grid's far sides, its last corner along that row or column.
                    let (gx, gz) = ((cx * cells + i).min(nx), (cz * cells + j).min(nz));
                    let c = grid[(gz * row + gx) as usize];
                    let pos = [lo[0] + gx as f32 * step, c.map_or(floor, |c| c.0), lo[1] + gz as f32 * step];
                    let n = c.map_or([0.0, 1.0, 0.0], |c| c.1);
                    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                    let normal = if l > 0.0 { n.map(|v| v / l) } else { [0.0, 1.0, 0.0] };
                    if c.is_some() {
                        for k in 0..3 {
                            low[k] = low[k].min(pos[k]);
                            high[k] = high[k].max(pos[k]);
                        }
                    }
                    vertices.push(game_render::gpu::GroundVertex { pos, normal });
                }
            }
            let missing: Vec<bool> = (0..cells * cells)
                .map(|n| {
                    let (gx, gz) = (cx * cells + n % cells, cz * cells + n / cells);
                    gx >= nx || gz >= nz || !drawn[(gz * nx + gx) as usize]
                })
                .collect();
            if missing.iter().all(|&m| m) {
                continue;
            }
            let centre = game_render::mat::v3((low[0] + high[0]) * 0.5, (low[1] + high[1]) * 0.5, (low[2] + high[2]) * 0.5);
            let radius = ((high[0] - centre.x).powi(2) + (high[1] - centre.y).powi(2) + (high[2] - centre.z).powi(2)).sqrt();
            chunks.push(GroundChunk {
                vertices,
                cells,
                at: (cx, cz),
                missing: if missing.iter().any(|&m| m) { missing } else { Vec::new() },
                centre,
                radius,
            });
        }
    }
    Ok(game_render::gpu::StandIn {
        piece: GroundPiece::of(chunks),
        step,
        origin: lo,
        cells: [nx, nz],
        rects: rects.into_iter().enumerate().map(|(i, r)| (i as u32, r)).collect(),
    })
}

/// A stand-in corner: its height in world feet, and its normal as the terrain stores it.
#[derive(Clone, Copy)]
struct GroundVertexAt(f32, [f32; 3]);

/// The ground at the sample nearest a point in a piece's own feet (V92): from the cell it starts,
/// or failing that from a neighbour's corner, as [`crate::ground::Ground::chunks`] finds them.
fn sample_near(t: &Terrain, tile: &Tile, lx: f32, lz: f32) -> Option<GroundVertexAt> {
    let spacing = tile.spacing.0;
    let [sx_n, sz_n] = tile.samples();
    let sx = ((lx / spacing).round().max(0.0) as u32).min(sx_n.saturating_sub(1));
    let sz = ((lz / spacing).round().max(0.0) as u32).min(sz_n.saturating_sub(1));
    for (k, dx, dz) in [(0, 0, 0), (1, 1, 0), (2, 0, 1), (3, 1, 1)] {
        if sx < dx || sz < dz {
            continue;
        }
        if let Some(q) = t.cell(sx - dx, sz - dz, 1) {
            let n = q.normal[k];
            return Some(GroundVertexAt(t.scale.0 * q.height[k].0, [n[0].0, n[1].0, n[2].0]));
        }
    }
    None
}

/// What a world looks like by map (MD9): its ground's paint, and each map's light and sky.
pub struct WorldLook {
    pub paint: game_render::terrain::WorldPaint,
    pub looks: game_render::scene::Looks,
    /// Each map's sky, in the regions' order; `None` where its scene names none that reads.
    pub skies: Vec<Option<game_render::sky::Sky>>,
    /// Each map's name, for the log.
    pub names: Vec<String>,
}

impl WorldMod {
    /// The world rectangles each tile owns, in the world's order: its `keep` cells moved by its offset;
    /// and on each axis the world wraps, its side, which [`WorldMod::tiled`]'s extent is: the farthest
    /// any piece (the seam joins too) owns, by the same sums.
    pub fn regions(&self) -> game_render::terrain::Regions {
        let owned = self.pieces[..self.tile_count]
            .iter()
            .map(|p| {
                let [x0, z0, x1, z1] = p.tile.keep.map(|k| k as f32 * p.tile.spacing.0);
                let [ax, az] = p.tile.at.map(|a| a.0);
                [ax + x0, az + z0, ax + x1, az + z1]
            })
            .collect();
        let side = |a: usize| {
            let far = |p: &WorldPiece| p.tile.at[a].0 + p.tile.keep[a + 2] as f32 * p.tile.spacing.0;
            if self.wrap[a] { self.pieces.iter().map(far).fold(0.0, f32::max) } else { 0.0 }
        };
        game_render::terrain::Regions { owned, period: [side(0), side(1)] }
    }

    /// Every tile's ground paint, light and sky, read from its own folder: its terrain's pages with its
    /// own detail map (MD4), its scene's lights and fog, and the sky it names — looked for in its
    /// folder first and then in `fallback`, the ride's other archives, as the ride itself looks.
    /// A tile whose terrain has no pages (a disc map's) has pages baked from its splat and light map
    /// ([`crate::world_dress::bake_splat`], MD15), or is dirt without them; one that does not read is
    /// dirt and the home tile's light. The tiles are read on every core.
    pub fn look(&self, fallback: &[Res], dirt: [f32; 3], home: game_render::world::Lighting) -> WorldLook {
        self.look_near(fallback, dirt, home, None)
    }

    /// [`WorldMod::look`], and with `near` (P283) the world's paint held near the riders only: every
    /// map's far pages kept, and the pages of a map `near` does not keep let go as soon as it is
    /// read ([`game_render::terrain::Pages::hollow`]), its far pages made, so the load never holds
    /// more than a core's map at a time besides; `reach` is how near a map stays held
    /// ([`game_render::terrain::FarPaint::reach`]).
    pub fn look_near(&self, fallback: &[Res], dirt: [f32; 3], home: game_render::world::Lighting, near: Option<(&(dyn Fn(usize) -> bool + Sync), f32)>) -> WorldLook {
        let regions = self.regions();
        let (mut tiles, mut lights, mut skies, mut names, mut far) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
        // Each map read on a core of its own (MD15: a disc map's paint is baked, which takes a while).
        let read_one = |(i, p): (usize, &WorldPiece)| {
            let read = || -> Result<_, String> {
                let (mut pages, mut sharp, look, text, res) = map_paint(p)?;
                let sky = game_render::sky::Sky::load(std::slice::from_ref(&res), &text).or_else(|| game_render::sky::Sky::load(fallback, &text));
                let mut far = Vec::new();
                if let (Some((keep, _)), Some(pg)) = (near, &mut pages) {
                    far = pg.pages.iter().map(game_render::terrain::far_page).collect();
                    if !keep(i) {
                        hollow(pg, &mut sharp);
                    }
                }
                Ok((pages, sharp, game_render::world::Lighting::of(&look), sky, far))
            };
            read().unwrap_or_else(|e| {
                println!("world: {} looks like dirt: {e}", p.tile.name);
                (None, None, home, None, Vec::new())
            })
        };
        let maps = &self.pieces[..self.tile_count];
        let threads = crate::thread::available_parallelism().map_or(4, |n| n.get()).min(maps.len()).max(1);
        let mut read: Vec<(usize, _)> = crate::thread::scope(|scope| {
            let jobs: Vec<_> = (0..threads)
                .map(|t| {
                    let read_one = &read_one;
                    scope.spawn(move || maps.iter().enumerate().skip(t).step_by(threads).map(|(i, p)| (i, read_one((i, p)))).collect::<Vec<_>>())
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().expect("a map's look")).collect()
        });
        read.sort_by_key(|(i, _)| *i);
        for (p, (_, (pages, sharp, light, sky, far_pages))) in maps.iter().zip(read) {
            let at = p.tile.at.map(|a| a.0);
            tiles.push(game_render::terrain::PaintTile { at, pages, sharp });
            far.push(far_pages);
            lights.push(light);
            skies.push(sky);
            names.push(p.tile.name.clone());
        }
        WorldLook {
            paint: game_render::terrain::WorldPaint {
                regions: regions.clone(),
                tiles,
                dirt,
                far: game_render::terrain::FarPaint { pages: if near.is_some() { far } else { Vec::new() }, reach: near.map(|n| n.1) },
            },
            looks: game_render::scene::Looks { regions, lights },
            skies,
            names,
        }
    }
}

/// A map's pages and its sharp splat's modulate pages let go (P283, [`game_render::terrain::Pages::hollow`]).
pub fn hollow(pages: &mut game_render::terrain::Pages, sharp: &mut Option<game_render::terrain::SharpSplat>) {
    pages.hollow();
    for m in sharp.iter_mut().flat_map(|s| s.modulate.iter_mut()) {
        m.rgba = Vec::new();
    }
}

/// A world map's ground paint read from its folder ([`WorldMod::look`]): its terrain's pages with
/// its own detail map (MD4), or a disc map's baked from its splat and light (MD15) with the splat
/// kept to draw per pixel (MD35); and its scene as moved into the world, the scene's text and its
/// archive, for the light and sky. A world's paint stream reads a map's pages again with it (P283).
pub fn map_paint(
    p: &WorldPiece,
) -> Result<(Option<game_render::terrain::Pages>, Option<game_render::terrain::SharpSplat>, game_render::scene::Scene, String, Res), String> {
    let res = p.res()?;
    let (_, stem, scn) = scene_of(&res)?;
    let text = game_render::ini::text(&scn);
    let scene = p.scene.as_ref().ok_or("a join has no scene")?;
    let bytes = track_mods::terrain_bytes(&res, &scene.terrain_file)?;
    let terrain = Terrain::parse(&bytes)?;
    let [sx, sz] = p.tile.samples();
    // Sixteen samples a leaf, as `Ground::paged` counts them.
    let leaves = (sx / 16, sz / 16);
    let detail = game_render::terrain::Pages::load_detail(std::slice::from_ref(&res), &stem);
    let pages = game_render::terrain::Pages::of(&terrain, p.tile.spacing.0, leaves)
        .map(|pg| pg.with_detail(detail, game_render::terrain::detail_multiplier(&text)));
    let mut look = game_render::scene::Scene::parse(&text);
    // A disc map has no pages: its splat and baked light, baked into some (MD15), and
    // the splat kept to be drawn per pixel over them on the GPU (MD35).
    let mut sharp = None;
    let pages = pages.or_else(|| {
        let [sx, sz] = p.tile.samples();
        let extent = ((sx - 1) as f32 * p.tile.spacing.0, (sz - 1) as f32 * p.tile.spacing.0);
        let splat = game_render::splat::Splat::load(std::slice::from_ref(&res), &stem, extent)?;
        let light = game_render::world::Lighting::of(&look);
        let (pages, modulate) = crate::world_dress::bake_splat_sharp(&splat, &terrain, p.tile.spacing.0, leaves, &light)?;
        sharp = Some(game_render::terrain::SharpSplat { splat: std::sync::Arc::new(splat), modulate });
        Some(pages)
    });
    move_look(&mut look, p.tile.at);
    Ok((pages, sharp, look, text, res))
}

/// MX Unleashed's National (outdoor motocross) tracks by the names the disc gives them: the
/// STR_NAT01..STR_NAT22 strings of appstart/loc's l_ui_eng.bin, which tracks.dat's `[NatXX]
/// TextID` points at.
pub const NATIONALS: [(&str, &str); 22] = [
    ("nat01", "Seneca Butte"),
    ("nat02", "Bodie"),
    ("nat03", "Racine: Long"),
    ("nat04", "Doha"),
    ("nat05", "Blueridge"),
    ("nat06", "Summerhaven"),
    ("nat07", "Pine Bluff"),
    ("nat08", "Slack Jaw"),
    ("nat09", "Goblin Valley"),
    ("nat10", "Foxpark"),
    ("nat11", "Hillsboro"),
    ("nat12", "Black Hills"),
    ("nat13", "Rattlesnake Pass"),
    ("nat14", "Grandview"),
    ("nat15", "Cedar Mills"),
    ("nat16", "Casa De Cobre"),
    ("nat17", "Waimea"),
    ("nat18", "Manassas Park"),
    ("nat19", "Ensenada"),
    ("nat20", "Sliderock"),
    ("nat21", "Signal Butte"),
    ("nat22", "Racine: Short"),
];

/// The name the radar gives a map (MD42): a free-ride track by the town it is named for, as the
/// track pick names it (`free06` is Dallas), a national by its disc name (`nat17` is Waimea),
/// anything else as it is.
pub fn map_title(name: &str) -> String {
    let name = name.trim();
    match crate::menu::TRACKS.iter().chain(NATIONALS.iter()).find(|(archive, _)| archive.eq_ignore_ascii_case(name)) {
        Some((_, town)) => town.to_string(),
        None => name.to_string(),
    }
}

impl WorldMod {
    /// The maps the radar names (MD42): every tile's rectangle ([`WorldMod::regions`]) and its
    /// name — its own `mod.json`'s for a world mod's tile, falling back to its folder's, and a disc
    /// track's own — through [`map_title`].
    pub fn map_names(&self) -> crate::hud::MapNames {
        let regions = self.regions();
        let maps = self.pieces[..self.tile_count]
            .iter()
            .zip(regions.owned)
            .map(|(p, rect)| {
                let named = match p.origin {
                    Origin::Folder => std::fs::read_to_string(p.dir.join("mod.json"))
                        .ok()
                        .and_then(|t| Json::parse(&t).ok())
                        .and_then(|j| j.get("name").and_then(Json::str).map(str::to_string))
                        .filter(|n| !n.trim().is_empty()),
                    _ => None,
                };
                let name = named.unwrap_or_else(|| match p.origin {
                    Origin::Folder => p.tile.name.clone(),
                    _ => p.stem.clone(),
                });
                (rect, map_title(&name))
            })
            .collect();
        crate::hud::MapNames { maps }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_sim::tiles::generate;

    fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) -> u64 {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, bytes.as_ref()).unwrap();
        bytes.as_ref().len() as u64
    }

    /// Bytes this thread has read from files so far, as Linux counts them: every `read`, whoever
    /// makes it.
    fn read_so_far() -> u64 {
        let io = std::fs::read_to_string("/proc/thread-self/io").expect("Linux counts a thread's reads");
        io.lines().find_map(|l| l.strip_prefix("rchar: ")).unwrap().trim().parse().unwrap()
    }

    /// MD18: a world's find reads its `mod.json`, its tiles' scenes, and of each terrain the header
    /// and the tree's root alone — and lays out the same tiles reading that as reading every terrain
    /// whole. Two tiles (one with a palette and a texture page, as MX vs. ATV writes them) and a join.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_worlds_find_reads_each_terrains_header_and_root_alone() {
        let mods = std::env::temp_dir().join(format!("openmx-md18-{}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        let world = mods.join("tracks").join("md18_world");
        let height = |x: u32, z: u32| 40.0 + f64::from((x * 7 + z * 3) % 11);
        let page = |x: u32, y: u32| [(x % 8 * 30) as u8, (y % 8 * 30) as u8, 9];
        // (name, root cells, spacing, painted)
        let tiles = [("t0", (4u32, 3u32), 9.0f32, true), ("t1", (2, 2), 4.5, false)];
        let mut small = 0;
        let mut terrains = Vec::new();
        let mut listed = Vec::new();
        for (i, &(name, cells, spacing, painted)) in tiles.iter().enumerate() {
            let stem = name.to_uppercase();
            let dir = world.join("tiles").join(name);
            small += write(&dir, "mod.json", format!(r#"{{"standard": 1, "kind": "track", "id": "md18_{name}", "name": "MD18 {name}", "source": {{"game": "Real elevation"}}, "version": "1.0.0", "author": "test", "permission": "public-domain", "todo": []}}"#));
            let scene = [
                "[SceneInfo]".to_string(),
                format!("SceneName=MD18 {name}"),
                "DefaultStartPosition=100.00,40.00,100.00".into(),
                "DefaultStartDirection=1.00,0.00,0.00".into(),
                "[Environment]".into(),
                format!("TerrainFile={stem}.trn"),
                format!("TerrainWidth={}", cells.0),
                format!("TerrainBreadth={}", cells.1),
                format!("TerrainWidthScale={spacing:.2}"),
                format!("TerrainBreadthScale={spacing:.2}"),
                String::new(),
            ]
            .join("\r\n");
            small += write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
            let pages: Option<&dyn Fn(u32, u32) -> [u8; 3]> = if painted { Some(&page) } else { None };
            let trn = generate::terrain_painted(name, cells, spacing, height, |_, _| 1, pages);
            let path = dir.join(format!("Env/{stem}/{stem}.ps2.trn"));
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), &trn);
            terrains.push(path);
            let at = 1000 * i;
            listed.push(format!(r#"{{"tile": "{name}", "at": [{at}.0, 0.0], "keep": [0, 0, {}, {}]}}"#, cells.0 * 16 - 1, cells.1 * 16 - 1));
        }
        let join = generate::terrain("j0", (3, 1), 6.0, height, |_, _| 1);
        write(&world, "joins/j0/j0.ps2.trn", &join);
        terrains.push(world.join("joins/j0/j0.ps2.trn"));
        small += write(
            &world,
            "mod.json",
            format!(
                r#"{{"standard": 1, "kind": "world", "id": "md18_world", "name": "MD18", "source": {{"game": "Combined"}}, "version": "1.0.0", "author": "test", "permission": "public-domain", "tiles": [{}], "joins": [{{"join": "j0", "at": [600.0, 0.0]}}], "drop": [], "todo": []}}"#,
                listed.join(", ")
            ),
        );

        let before = read_so_far();
        let w = find(&mods, "md18_world").expect("the world reads");
        let read = read_so_far() - before;
        // The mod.json and scenes whole, and of each terrain its header, root node and root table.
        let most = small + terrains.len() as u64 * (TerrainHead::MOST + TerrainNode::BYTES + TerrainNode::TABLE_BYTES) as u64;
        let whole: u64 = terrains.iter().map(|p| std::fs::metadata(p).unwrap().len()).sum();
        println!("find read {read} bytes; at most {most} allowed; the terrains are {whole} bytes");
        assert!(read <= most + 512, "find read {read} bytes, more than the {most} its files' headers are");
        assert!(whole > 10 * most, "the terrains ({whole} bytes) are too small to tell a whole read from a header's");

        // The same world as a whole read of each terrain makes it.
        assert_eq!((w.pieces.len(), w.tile_count), (3, 2));
        for (p, path) in w.pieces.iter().zip(&terrains) {
            let (spacing, levels, width, breadth) = shape(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!((p.tile.spacing, p.tile.levels), (spacing, levels), "{}", p.tile.name);
            assert_eq!(shape_of_file(path), Ok((spacing, levels, width, breadth)), "{}", p.tile.name);
            if p.scene.is_none() {
                assert_eq!((p.tile.layout.width, p.tile.layout.breadth), (width, breadth), "{}", p.tile.name);
            }
        }
        assert_eq!((w.pieces[0].tile.spacing, w.pieces[1].tile.spacing, w.pieces[2].tile.spacing), (F(9.0), F(4.5), F(6.0)));
        assert_eq!((w.pieces[2].tile.layout.width, w.pieces[2].tile.layout.breadth), (3, 1));
        // The scenes are kept for the ride, as the files have them.
        for (p, (name, ..)) in w.pieces.iter().zip(&tiles) {
            let stem = name.to_uppercase();
            let on_disk = std::fs::read(p.dir.join(format!("Env/{stem}/{stem}.scn"))).unwrap();
            assert_eq!(p.scene_bytes.as_deref(), Some(&on_disk[..]));
        }
        assert!(w.pieces[2].scene_bytes.is_none());

        // A terrain cut short of its tree is refused by find; one cut inside its blocks only by the ride.
        let cut = &terrains[1];
        let bytes = std::fs::read(cut).unwrap();
        std::fs::write(cut, &bytes[..TerrainHead::MOST.min(bytes.len())]).unwrap();
        assert!(find(&mods, "md18_world").unwrap_err().contains("does not fit"));
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }

    /// MD42: a world's maps as the radar names them — each tile's own `mod.json` name (a free-ride
    /// track's by its town), its folder's where it has none — and, for a rider walked across the
    /// grid, the map they are on and the one each way, changing as they cross a tile's edge or ride
    /// a join between two.
    #[test]
    fn a_worlds_radar_names_the_map_underfoot_and_each_neighbour() {
        let mods = std::env::temp_dir().join(format!("openmx-md42-{}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        let world = mods.join("tracks").join("md42_world");
        // (folder, its mod.json's name, at): a 2x2 grid, 200 ft apart, each tile 124 ft across.
        let tiles = [("sw", Some("free06"), [0, 0]), ("se", Some("MD42 Quarry"), [200, 0]), ("nw", None, [0, 200]), ("ne", Some(" "), [200, 200])];
        let mut listed = Vec::new();
        for &(name, title, [x, z]) in &tiles {
            let stem = name.to_uppercase();
            let dir = world.join("tiles").join(name);
            if let Some(title) = title {
                write(&dir, "mod.json", format!(r#"{{"standard": 1, "kind": "track", "id": "md42_{name}", "name": "{title}"}}"#));
            }
            let scene = format!("[SceneInfo]\r\nSceneName={name}\r\nDefaultStartPosition=60.00,10.00,60.00\r\nDefaultStartDirection=1.00,0.00,0.00\r\n[Environment]\r\nTerrainFile={stem}.trn\r\nTerrainWidth=2\r\nTerrainBreadth=2\r\nTerrainWidthScale=4.00\r\nTerrainBreadthScale=4.00\r\n");
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), generate::terrain(name, (2, 2), 4.0, |_, _| 10.0, |_, _| 1));
            listed.push(format!(r#"{{"tile": "{name}", "at": [{x}.0, {z}.0], "keep": [0, 0, 31, 31]}}"#));
        }
        write(&world, "mod.json", format!(r#"{{"kind": "world", "id": "md42_world", "name": "MD42", "tiles": [{}]}}"#, listed.join(", ")));
        let w = find(&mods, "md42_world").expect("the world reads");
        let names = w.map_names();
        let titles: Vec<&str> = names.maps.iter().map(|(_, n)| n.as_str()).collect();
        assert_eq!(titles, ["Dallas", "MD42 Quarry", "nw", "ne"]);
        assert_eq!(names.maps[1].0, [200.0, 0.0, 324.0, 124.0]);

        // N E S W from the south-west tile, then across its east edge, then on to the north-east.
        assert_eq!(names.around([60.0, 60.0]), (Some("Dallas"), [Some("nw"), Some("MD42 Quarry"), None, None]));
        // Between the two south tiles, nearer the first: still on it.
        assert_eq!(names.around([150.0, 60.0]).0, Some("Dallas"));
        assert_eq!(names.around([180.0, 60.0]), (Some("MD42 Quarry"), [Some("ne"), None, None, Some("Dallas")]));
        assert_eq!(names.around([260.0, 260.0]), (Some("ne"), [None, None, Some("MD42 Quarry"), Some("nw")]));
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }

    /// MD46: a world file saying `"wrap": "x"` — MD42's 2x2 grid — names, at the tile on a wrapped
    /// edge, the map across the seam (the first tile on the far side) the way the edge faces, as the
    /// ride's radar takes it ([`crate::hud::MapNames::around_in`] with the drawn wrap); along `z`,
    /// which does not wrap, the edge's label stays blank, and so do both without the switch.
    #[test]
    fn a_wrapped_worlds_edge_names_the_map_across_the_seam() {
        let mods = std::env::temp_dir().join(format!("openmx-md46-{}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        let tiles = [("sw", [0, 0]), ("se", [200, 0]), ("nw", [0, 200]), ("ne", [200, 200])];
        let mut listed = Vec::new();
        for &(name, [x, z]) in &tiles {
            let stem = name.to_uppercase();
            for id in ["md46_round", "md46_edged"] {
                let dir = mods.join("tracks").join(id).join("tiles").join(name);
                let scene = format!("[SceneInfo]\r\nSceneName={name}\r\nDefaultStartPosition=60.00,10.00,60.00\r\nDefaultStartDirection=1.00,0.00,0.00\r\n[Environment]\r\nTerrainFile={stem}.trn\r\nTerrainWidth=2\r\nTerrainBreadth=2\r\nTerrainWidthScale=4.00\r\nTerrainBreadthScale=4.00\r\n");
                write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
                write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), generate::terrain(name, (2, 2), 4.0, |_, _| 10.0, |_, _| 1));
            }
            listed.push(format!(r#"{{"tile": "{name}", "at": [{x}.0, {z}.0], "keep": [0, 0, 31, 31]}}"#));
        }
        for (id, wrap) in [("md46_round", r#""wrap": "x", "#), ("md46_edged", "")] {
            write(&mods.join("tracks").join(id), "mod.json", format!(r#"{{"kind": "world", "id": "{id}", "name": "MD46", {wrap}"tiles": [{}]}}"#, listed.join(", ")));
        }
        let read = |id: &str| {
            let w = find(&mods, id).unwrap_or_else(|e| panic!("{id}: {e}"));
            let t = w.tiled(Reach::DESKTOP).unwrap();
            (w.map_names(), game_render::wrap::Wrap::of(t.wraps(), t.extent().map(|v| v.0)))
        };
        let (names, wrap) = read("md46_round");
        assert!(wrap.is_some_and(|w| w.on == [true, false]), "{wrap:?}");
        assert_eq!(names.maps.len(), 4, "the seam joins are no map: {:?}", names.maps);
        // The west column's west label names the east column's tile across the seam, and back.
        assert_eq!(names.around_in([60.0, 60.0], wrap), (Some("sw"), [Some("nw"), Some("se"), None, Some("se")]));
        assert_eq!(names.around_in([260.0, 60.0], wrap), (Some("se"), [Some("ne"), Some("sw"), None, Some("sw")]));
        assert_eq!(names.around_in([60.0, 260.0], wrap), (Some("nw"), [None, Some("ne"), Some("sw"), Some("ne")]));
        let (names, wrap) = read("md46_edged");
        assert_eq!(wrap, None);
        assert_eq!(names.around_in([60.0, 60.0], wrap), (Some("sw"), [Some("nw"), Some("se"), None, None]));
        assert_eq!(names.around_in([260.0, 60.0], wrap), (Some("se"), [Some("ne"), None, None, Some("sw")]));
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }

    /// MD42: the neighbour each way is the map across from the rider — of two maps east, the one
    /// whose span along `z` holds the rider's `z` — and a lone track is one map with none.
    #[test]
    fn the_neighbour_each_way_is_the_one_across_from_the_rider() {
        let maps = vec![
            ([0.0, 0.0, 100.0, 200.0], "tall".to_string()),
            ([150.0, 0.0, 250.0, 90.0], "low".to_string()),
            ([150.0, 110.0, 250.0, 200.0], "high".to_string()),
            ([400.0, 0.0, 500.0, 200.0], "far".to_string()),
        ];
        let names = crate::hud::MapNames { maps };
        assert_eq!(names.around([50.0, 40.0]).1[1], Some("low"));
        assert_eq!(names.around([50.0, 160.0]).1[1], Some("high"));
        // In the gap between the two, riding east meets neither: the map past them it does meet.
        assert_eq!(names.around([50.0, 104.0]).1[1], Some("far"));
        assert_eq!(names.around([200.0, 40.0]), (Some("low"), [Some("high"), Some("far"), None, Some("tall")]));
        // A lone track names no map (P234): it rides on no world's maps.
        assert_eq!(crate::hud::MapNames::default().around([-1.0e6, 3.0e6]), (None, [None; 4]));
        assert_eq!(map_title("free09"), "Bridgeport");
        // A national goes by its disc name, another game's track by its own.
        assert_eq!(map_title("nat17"), "Waimea");
        assert_eq!(map_title(" NAT03 "), "Racine: Long");
        assert_eq!(map_title("Glamis"), "Glamis");
    }

    /// MD29: a world file's `wrap` reads as off, one axis or both, and anything else is refused.
    #[test]
    fn the_wrap_switch_reads_its_spellings() {
        let read = |text: &str| wrap_switch(Some(&Json::parse(text).unwrap()));
        assert_eq!(wrap_switch(None), Ok([false, false]));
        for (text, want) in [("null", [false, false]), ("false", [false, false]), ("\"none\"", [false, false]), ("true", [true, true]), ("\"x\"", [true, false]), ("\"z\"", [false, true]), ("\"xz\"", [true, true]), ("\"both\"", [true, true]), ("[false, true]", [false, true])] {
            assert_eq!(read(text), Ok(want), "{text}");
        }
        for text in ["\"y\"", "3", "[true]", "{}"] {
            assert!(read(text).is_err(), "{text}");
        }
    }

    /// MD50: a wrapped 3x3 world (nine 124-foot tiles 200 feet apart, the file's spawn the corner
    /// tile `a`) starts on the middle tile `e`, at `e`'s own start; without the switch it starts on
    /// `a` as the file says; a start in world feet is kept; and where the middle tile's own start is
    /// cut out of its keep, the nearest tile whose start is kept is taken, the first on a tie.
    #[test]
    fn a_wrapped_world_starts_on_its_middle_tile() {
        let mods = std::env::temp_dir().join(format!("openmx-md50-{}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        let names = ["a", "b", "c", "d", "e", "f", "g", "h", "i"];
        let worlds = [
            ("md50_round", r#""wrap": "xz", "spawn": {"tile": "a"}, "#, false),
            ("md50_edged", r#""spawn": {"tile": "a"}, "#, false),
            ("md50_feet", r#""wrap": "xz", "spawn": {"position": [30, 10, 40], "direction": [0, 0, 1]}, "#, false),
            ("md50_cut", r#""wrap": "xz", "spawn": {"tile": "a"}, "#, true),
        ];
        for (id, head, cut) in worlds {
            let mut listed = Vec::new();
            for (n, name) in names.iter().enumerate() {
                let (x, z) = (n % 3 * 200, n / 3 * 200);
                let stem = name.to_uppercase();
                let dir = mods.join("tracks").join(id).join("tiles").join(name);
                // The middle tile's start past its keep's 124 feet, on the ground the keep leaves out.
                let start = if cut && *name == "e" { "126.00,10.00,60.00" } else { "60.00,10.00,60.00" };
                let scene = format!("[SceneInfo]\r\nSceneName={name}\r\nDefaultStartPosition={start}\r\nDefaultStartDirection=1.00,0.00,0.00\r\n[Environment]\r\nTerrainFile={stem}.trn\r\nTerrainWidth=2\r\nTerrainBreadth=2\r\nTerrainWidthScale=4.00\r\nTerrainBreadthScale=4.00\r\n");
                write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
                write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), generate::terrain(name, (2, 2), 4.0, |_, _| 10.0, |_, _| 1));
                listed.push(format!(r#"{{"tile": "{name}", "at": [{x}.0, {z}.0], "keep": [0, 0, 31, 31]}}"#));
            }
            write(&mods.join("tracks").join(id), "mod.json", format!(r#"{{"kind": "world", "id": "{id}", "name": "MD50", {head}"tiles": [{}]}}"#, listed.join(", ")));
        }
        let start = |id: &str| {
            let w = find(&mods, id).unwrap_or_else(|e| panic!("{id}: {e}"));
            let mut scene = w.pieces[w.home].scene.clone().unwrap();
            w.place_spawn(&mut scene).unwrap();
            let tile = match w.spawn {
                Spawn::Tile(i) => Some(w.pieces[i].tile.name.clone()),
                Spawn::At(..) => None,
            };
            (tile, [scene.start[0].0, scene.start[2].0], w.pieces[w.home].tile.name.clone())
        };
        assert_eq!(start("md50_round"), (Some("e".into()), [260.0, 260.0], "a".into()), "the sky stays the file's");
        assert_eq!(start("md50_edged"), (Some("a".into()), [60.0, 60.0], "a".into()));
        assert_eq!(start("md50_feet"), (None, [30.0, 40.0], "a".into()));
        // The seam strip past the far edges puts the world's middle east and north of `e`'s, so of
        // the four tiles beside `e`, `f` and `h` are nearest, and `f` comes first.
        assert_eq!(start("md50_cut"), (Some("f".into()), [460.0, 260.0], "a".into()));
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }

    /// Where each tile of `w` does not read as that map alone (MD56): its ground as the simulation
    /// asks the world ([`Tiled::at`]) against the tile's own terrain ([`game_sim::world::Ground`]) at
    /// the same point of it, on a 40 x 40 grid over its keep, and its drawn ground's corners
    /// ([`piece_chunks`]) against the same. The world's point is the tile's moved by its `at`, and the
    /// tile's is read back from it as the world does, so the two answers are the same to the bit. A
    /// corner is drawn at a sample's height, which a far-off world's float position can put a hair off
    /// the sample, so the drawn ground may be off by 0.05 feet at one corner in a thousand.
    fn tiles_unlike_alone(w: &WorldMod) -> Vec<String> {
        let tiled = w.tiled(Reach::DESKTOP).unwrap();
        let mut out = Vec::new();
        for (i, p) in w.pieces[..w.tile_count].iter().enumerate() {
            let name = &p.tile.name;
            let bytes = p.terrain().unwrap_or_else(|e| panic!("{name}: {e}"));
            let terrain = Terrain::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            let alone = game_sim::world::Ground::new(&terrain, &p.tile.layout);
            let [ax, az] = p.tile.at;
            let [k0, k1, k2, k3] = keep_feet(p.tile.keep, p.tile.spacing);
            let (mut asked, mut off, mut low, mut high) = (0, 0, f32::MAX, f32::MIN);
            for gz in 0..40 {
                for gx in 0..40 {
                    let x = F(k0 + (k2 - k0) * (gx as f32 + 0.37) / 40.0) + ax;
                    let z = F(k1 + (k3 - k1) * (gz as f32 + 0.61) / 40.0) + az;
                    let own = alone.at(x - ax, z - az).map(|h| h.y.0);
                    let world = tiled.at(x, z).map(|h| h.y.0);
                    asked += 1;
                    if world != own || own.is_none() {
                        off += 1;
                    }
                    if let Some(h) = own {
                        (low, high) = (low.min(h), high.max(h));
                    }
                }
            }
            if off > 0 {
                out.push(format!("{name}: {off} of {asked} points read otherwise in the world than alone"));
            }
            if !(high - low > 1.0) {
                out.push(format!("{name}: flat alone ({low}..{high}), so nothing told"));
            }
            let chunks = piece_chunks(&p.tile, i, &bytes, &tiled, 32).unwrap_or_else(|e| panic!("{name}: {e}"));
            let (mut corners, mut wrong) = (0usize, 0usize);
            for v in chunks.iter().flat_map(|c| &c.vertices) {
                if let Some(h) = alone.at(F(v.pos[0]) - ax, F(v.pos[2]) - az) {
                    corners += 1;
                    if (h.y.0 - v.pos[1]).abs() > 0.05 {
                        wrong += 1;
                    }
                }
            }
            if corners == 0 || wrong * 1000 > corners {
                out.push(format!("{name}: {wrong} of {corners} drawn corners off its own ground"));
            }
        }
        out
    }

    /// MD56: every tile of a world reads the same heights as that map alone, at its `at` — the
    /// simulation's ground and the drawn ground's corners — on a generated world of three maps of
    /// their own hills at 9, 4.5 and Nat_OR's 4.499999 feet a sample, each cut to a keep, and a join.
    #[test]
    fn every_tile_of_a_world_reads_its_heights_as_alone() {
        let mods = std::env::temp_dir().join(format!("openmx-md56-{}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        let world = mods.join("tracks").join("md56_world");
        // (name, root cells, spacing, at): hills of their own on each, so a tile read at the wrong
        // place or from another's file reads otherwise.
        let tiles = [("t0", (3u32, 2u32), 9.0f32, [0, 0]), ("t1", (4, 4), 4.5, [600, 0]), ("t2", (4, 3), 4.499999, [0, 700])];
        let mut listed = Vec::new();
        for (n, &(name, cells, spacing, [x, z])) in tiles.iter().enumerate() {
            let stem = name.to_uppercase();
            let dir = world.join("tiles").join(name);
            let scene = format!("[SceneInfo]\r\nSceneName={name}\r\nDefaultStartPosition=60.00,10.00,60.00\r\nDefaultStartDirection=1.00,0.00,0.00\r\n[Environment]\r\nTerrainFile={stem}.trn\r\nTerrainWidth={}\r\nTerrainBreadth={}\r\nTerrainWidthScale={spacing}\r\nTerrainBreadthScale={spacing}\r\n", cells.0, cells.1);
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
            let k = 1.0 + n as f64;
            let height = |sx: u32, sz: u32| 200.0 + 60.0 * (f64::from(sx) * 0.11 * k).sin() * (f64::from(sz) * 0.07 + k).cos() + f64::from((sx * 13 + sz * 7) % 5);
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), generate::terrain(name, cells, spacing, height, |_, _| 1));
            listed.push(format!(r#"{{"tile": "{name}", "at": [{x}.0, {z}.0], "keep": [3, 2, {}, {}]}}"#, cells.0 * 16 - 4, cells.1 * 16 - 3));
        }
        write(&world, "joins/j0/j0.ps2.trn", generate::terrain("j0", (4, 4), 9.0, |_, _| 150.0, |_, _| 1));
        write(
            &world,
            "mod.json",
            format!(r#"{{"kind": "world", "id": "md56_world", "name": "MD56", "tiles": [{}], "joins": [{{"join": "j0", "at": [300.0, 300.0]}}]}}"#, listed.join(", ")),
        );
        let w = find(&mods, "md56_world").expect("the world reads");
        assert_eq!(w.tile_count, 3);
        assert_eq!(tiles_unlike_alone(&w), Vec::<String>::new());
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }

    /// A generated world of two tiles of their own hills and a join, with `source` its mod.json's
    /// (MD97).
    fn two_tile_world(mods: &Path, id: &str, source: &str) {
        let world = mods.join("tracks").join(id);
        let mut listed = Vec::new();
        for (n, (name, [x, z])) in [("t0", [0, 0]), ("t1", [600, 0])].into_iter().enumerate() {
            let stem = name.to_uppercase();
            let dir = world.join("tiles").join(name);
            let scene = format!("[SceneInfo]\r\nSceneName={name}\r\nDefaultStartPosition=60.00,10.00,60.00\r\nDefaultStartDirection=1.00,0.00,0.00\r\n[Environment]\r\nTerrainFile={stem}.trn\r\nTerrainWidth=3\r\nTerrainBreadth=3\r\nTerrainWidthScale=9\r\nTerrainBreadthScale=9\r\n");
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
            let k = 1.0 + n as f64;
            let height = |sx: u32, sz: u32| 200.0 + 40.0 * (f64::from(sx) * 0.1 * k).sin() + f64::from((sx * 13 + sz * 7) % 5);
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), generate::terrain(name, (3, 3), 9.0, height, |_, _| 1));
            listed.push(format!(r#"{{"tile": "{name}", "at": [{x}.0, {z}.0], "keep": [0, 0, 47, 47]}}"#));
        }
        write(&world, "joins/j0/j0.ps2.trn", generate::terrain("j0", (1, 3), 9.0, |_, _| 150.0, |_, _| 1));
        write(
            &world,
            "mod.json",
            format!(r#"{{"kind": "world", "id": "{id}", "name": "MD97", "source": {source}, "tiles": [{}], "joins": [{{"join": "j0", "at": [440.0, 0.0]}}]}}"#, listed.join(", ")),
        );
    }

    /// MD97: a world one of whose tiles' folders is not there is refused, and says which game's
    /// files it needs — not ridden with a hole where the tile was.
    #[test]
    fn a_world_whose_tile_files_are_missing_is_refused_with_the_game_it_needs() {
        let mods = std::env::temp_dir().join(format!("openmx-md97-missing-{}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        two_tile_world(&mods, "md97_world", r#"{"game": "Combined", "games": ["MX vs. ATV Unleashed"]}"#);
        assert_eq!(find(&mods, "md97_world").expect("whole, it reads").tile_count, 2);
        std::fs::remove_dir_all(mods.join("tracks/md97_world/tiles/t1")).unwrap();
        let e = find(&mods, "md97_world").err().expect("a world missing a tile is refused");
        assert_eq!(e, "md97_world: t1 is not installed: the world needs MX vs. ATV Unleashed's files");
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }

    /// MD97: after the iPhone app was sent away, every piece the stream held is built and handed
    /// over again — each as it was, at its detail — and nothing past them; the pieces then rest.
    #[test]
    fn the_ground_held_is_built_again_after_the_app_was_away() {
        let mods = std::env::temp_dir().join(format!("openmx-md97-again-{}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        two_tile_world(&mods, "md97_again", r#"{"game": "Real elevation"}"#);
        let w = find(&mods, "md97_again").unwrap();
        let corners = |made: &GroundPiece| -> Vec<[u32; 3]> { made.chunks.iter().flat_map(|c| c.vertices.iter().map(|v| v.pos.map(f32::to_bits))).collect() };
        let reach = Reach { load: F(5000.0), drop: F(6000.0) };
        let near = [[60.0f32, 60.0]];
        let mut first = std::collections::BTreeMap::new();
        let mut s = GroundStream::start_into(&w, 16, reach, None, &near, false, &mut |i, made| {
            first.insert(i, corners(&made));
        });
        assert_eq!(first.len(), w.pieces.len(), "every piece is in reach");
        assert!(s.update(&near).is_empty(), "nothing more before the app goes away");
        assert_eq!(s.build_again(), w.pieces.len());
        let mut again = std::collections::BTreeMap::new();
        let mut shown = std::collections::BTreeSet::new();
        let until = Instant::now() + std::time::Duration::from_secs(60);
        while shown.len() < w.pieces.len() && Instant::now() < until {
            for change in s.update(&near) {
                match change {
                    Change::Add(i, made) => assert!(again.insert(i, corners(&made)).is_none(), "piece {i} handed over twice"),
                    Change::More(i, _) => panic!("piece {i} in more than one part"),
                    Change::Drop(i) => panic!("piece {i} dropped"),
                    Change::Show(i) => _ = shown.insert(i),
                }
            }
            std::thread::yield_now();
        }
        assert_eq!(again, first, "every piece again, the same ground");
        assert_eq!(s.held(), w.pieces.len());
        for _ in 0..3 {
            assert!(s.update(&near).is_empty(), "built again once, not every frame");
        }
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }

    /// MD97 on the installed world_mxvatv: everywhere the ride has ground — every 40 feet over
    /// the world, wherever a piece owns the point ([`Tiled::owner`]) — that piece's drawn chunks
    /// cover it, and every tile's corners are not all at one height. A join all of whose ground
    /// the tiles own draws nothing, and that is no hole.
    #[test]
    #[ignore = "needs the stitched worlds in $OPENMX_WORLDS/tracks"]
    fn every_tile_of_the_installed_world_mxvatv_has_ground() {
        let mods = PathBuf::from(std::env::var_os("OPENMX_WORLDS").expect("OPENMX_WORLDS names a mods folder"));
        let w = find(&mods, "world_mxvatv").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(w.tile_count, 27);
        let reach = Reach { load: F(1500.0), drop: F(2000.0) };
        let tiled = Tiled::new(w.pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(Files { pieces: w.pieces.clone() }), reach).unwrap();
        let owners = Tiled::new(w.pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(Files { pieces: w.pieces.clone() }), reach).unwrap();
        let mut b = Builder { files: Files { pieces: w.pieces.clone() }, tiled: Some(tiled), cells: 32 };
        let mut wrong = Vec::new();
        // Each piece's chunks' rectangles, [x0, z0, x1, z1].
        let mut drawn: Vec<Vec<[f32; 4]>> = Vec::new();
        for (i, p) in w.pieces.iter().enumerate() {
            let piece = b.build(i, true).unwrap_or_else(|e| panic!("{}: {e}", p.tile.name));
            let ys: Vec<f32> = piece.chunks.iter().flat_map(|c| c.vertices.iter().map(|v| v.pos[1])).collect();
            let (lo, hi) = ys.iter().fold((f32::MAX, f32::MIN), |(l, h), &y| (l.min(y), h.max(y)));
            if i < w.tile_count && !(hi - lo > 1.0) {
                wrong.push(format!("{}: {} chunks, heights {lo}..{hi}", p.tile.name, piece.chunks.len()));
            }
            drawn.push(
                piece
                    .chunks
                    .iter()
                    .map(|c| c.vertices.iter().fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |r, v| [r[0].min(v.pos[0]), r[1].min(v.pos[2]), r[2].max(v.pos[0]), r[3].max(v.pos[2])]))
                    .collect(),
            );
        }
        let all = w.pieces.iter().map(|p| rect(&p.tile, 0.0)).fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |r, t| [r[0].min(t[0]), r[1].min(t[1]), r[2].max(t[2]), r[3].max(t[3])]);
        let (mut owned, mut holes) = (0usize, Vec::new());
        let mut z = all[1];
        while z < all[3] {
            let mut x = all[0];
            while x < all[2] {
                if let Some((o, _, _)) = owners.owner(F(x), F(z)) {
                    owned += 1;
                    if !drawn[o].iter().any(|r| r[0] <= x && x <= r[2] && r[1] <= z && z <= r[3]) {
                        holes.push(format!("{} at {x:.0},{z:.0}", w.pieces[o].tile.name));
                    }
                }
                x += 40.0;
            }
            z += 40.0;
        }
        assert!(owned > 10_000, "{owned} points with ground");
        if !holes.is_empty() {
            wrong.push(format!("{} of {owned} points with ground undrawn: {:?}", holes.len(), &holes[..holes.len().min(20)]));
        }
        assert_eq!(wrong, Vec::<String>::new());
    }

    /// MD56 on the installed worlds: every tile of each reads the same heights as that map alone.
    /// The user's flat "maps" were the joins between them (see [`crate::hud::OFF_MAP`]).
    #[test]
    #[ignore = "needs the stitched worlds in $OPENMX_WORLDS/tracks"]
    fn every_installed_worlds_tiles_read_as_alone() {
        let mods = PathBuf::from(std::env::var_os("OPENMX_WORLDS").expect("OPENMX_WORLDS names a mods folder"));
        let mut wrong = Vec::new();
        for id in ["world_mxu", "world_mxu_free", "world_mxvatv", "world_untamed", "world_ultra"] {
            let w = find(&mods, id).unwrap_or_else(|e| panic!("{id}: {e}"));
            wrong.extend(tiles_unlike_alone(&w).into_iter().map(|e| format!("{id} {e}")));
        }
        assert_eq!(wrong, Vec::<String>::new());
    }
}

/// V94: the stream's order puts the piece a rider heads for before one as near behind or beside it,
/// once the rider has ridden [`HEADING_FEET`], and forgets the heading over a teleport.
#[cfg(test)]
mod heading_order {
    use super::*;

    /// A 5x5 world of 1000 ft pieces, and a stream over it that builds nothing.
    fn stream() -> GroundStream {
        let layout = TerrainLayout { width: 1, breadth: 1, ..TerrainLayout::default() };
        let tiles: Vec<Tile> = (0..25)
            .map(|i| Tile { name: format!("p{}{}", i % 5, i / 5), at: [F((i % 5 * 1000) as f32), F((i / 5 * 1000) as f32)], layout, spacing: F(62.5), levels: 1, keep: [0, 0, 16, 16], first_owner: 0 })
            .collect();
        let reach = Reach { load: F(1500.0), drop: F(2000.0) };
        GroundStream {
            held: vec![None; tiles.len()],
            asked: vec![None; tiles.len()],
            failed: vec![false; tiles.len()],
            redo: vec![false; tiles.len()],
            tiles,
            reach,
            fine: None,
            work: Arc::new((Mutex::new((Vec::new(), false)), Condvar::new())),
            built: channel().1,
            pending: VecDeque::new(),
            chunks_a_frame: CHUNKS_A_FRAME,
            log: Vec::new(),
            heading: Vec::new(),
        }
    }

    /// The pieces in reach of `at` in the order the builder takes them (update's `want`, popped).
    fn queue(s: &mut GroundStream, at: [f32; 2]) -> Vec<String> {
        let order = s.ahead_first(&[at]);
        let mut want: Vec<usize> = (0..s.tiles.len()).filter(|&i| nearest(&s.tiles[i], &[at]) <= s.reach.load.0).collect();
        want.sort_by(|a, b| order[*b].total_cmp(&order[*a]));
        want.iter().rev().map(|&i| s.tiles[i].name.clone()).collect()
    }

    #[test]
    fn the_piece_ahead_is_built_before_the_one_behind() {
        let mut s = stream();
        // Standing on the middle piece: no heading yet, so ahead and behind tie on their gap.
        let order = s.ahead_first(&[[2500.0, 2500.0]]);
        let at = |s: &GroundStream, name: &str| s.tiles.iter().position(|t| t.name == name).unwrap();
        assert_eq!(order[at(&s, "p32")], order[at(&s, "p12")], "no heading before the rider moves");
        // Ridden 100 ft east: east ahead of west, north and south between.
        let order = s.ahead_first(&[[2600.0, 2500.0]]);
        let (e, w, n, so) = (order[at(&s, "p32")], order[at(&s, "p12")], order[at(&s, "p23")], order[at(&s, "p21")]);
        assert!(e < n && n == so && so < w, "east {e} north {n} south {so} west {w}");
        // In the builder's queue, so is the piece two ahead (gap 1400, counted 700) before the one
        // behind (gap 600, counted 900).
        let q = queue(&mut s, [2600.0, 2500.0]);
        let pos = |name: &str| q.iter().position(|n| n == name).unwrap();
        assert_eq!(q[0], "p22", "the piece underfoot first: {q:?}");
        assert!(pos("p32") < pos("p23") && pos("p23") < pos("p12"), "{q:?}");
        assert!(pos("p42") < pos("p12"), "the piece two ahead before the one behind: {q:?}");
        // Turned north and ridden 100 ft: north first now.
        let order = s.ahead_first(&[[2600.0, 2600.0]]);
        assert!(order[at(&s, "p23")] < order[at(&s, "p32")], "north before east after the turn");
        // A teleport has no heading: the gaps alone again.
        let order = s.ahead_first(&[[600.0, 600.0]]);
        assert_eq!(order[at(&s, "p10")], order[at(&s, "p01")], "no heading over a teleport");
    }
}

/// P286: the far ring's ground, a [`COARSE_BY`]th of a piece's samples each way, and when a piece
/// goes from one to the other.
#[cfg(test)]
mod far_ring {
    use super::*;
    use game_render::gpu::GroundVertex;

    const CELLS: u32 = 8;
    const SPACING: f32 = 4.5;
    const AT: [f32; 2] = [1000.0, -500.0];

    fn height(gx: u32, gz: u32) -> f32 {
        (gx * 7 + gz * 13) as f32 * 0.25
    }

    /// A piece of `side` x `side` chunks of [`CELLS`] cells, as [`piece_chunks`] gives them, less the
    /// chunks `absent`, and each chunk's cells `gone` says left out.
    fn piece(side: u32, absent: &[(u32, u32)], gone: impl Fn(u32, u32) -> bool) -> Vec<GroundChunk> {
        let row = CELLS + 1;
        let mut out = Vec::new();
        for cz in 0..side {
            for cx in 0..side {
                if absent.contains(&(cx, cz)) {
                    continue;
                }
                let vertices = (0..row * row)
                    .map(|k| {
                        let (gx, gz) = (cx * CELLS + k % row, cz * CELLS + k / row);
                        GroundVertex { pos: [AT[0] + gx as f32 * SPACING, height(gx, gz), AT[1] + gz as f32 * SPACING], normal: [0.0, 1.0, 0.0] }
                    })
                    .collect();
                let missing: Vec<bool> = (0..CELLS * CELLS).map(|k| gone(cx * CELLS + k % CELLS, cz * CELLS + k / CELLS)).collect();
                let missing = if missing.contains(&true) { missing } else { Vec::new() };
                out.push(GroundChunk { vertices, cells: CELLS, at: (cx, cz), missing, centre: v3(0.0, 0.0, 0.0), radius: 1.0 });
            }
        }
        out
    }

    #[test]
    fn a_far_piece_has_every_fourth_sample_in_a_sixteenth_of_the_chunks() {
        let last = 8 * CELLS;
        let far = coarse(piece(8, &[], |_, _| false), COARSE_BY, AT, SPACING, [last, last]);
        assert_eq!(far.len(), 4, "8 x 8 chunks to 2 x 2");
        for c in &far {
            assert_eq!((c.cells, c.vertices.len()), (CELLS, ((CELLS + 1) * (CELLS + 1)) as usize));
            assert!(c.missing.is_empty(), "{:?}", c.at);
            for (k, v) in c.vertices.iter().enumerate() {
                let (i, j) = (k as u32 % (CELLS + 1), k as u32 / (CELLS + 1));
                let (gx, gz) = ((c.at.0 * CELLS + i) * COARSE_BY, (c.at.1 * CELLS + j) * COARSE_BY);
                assert_eq!(v.pos, [AT[0] + gx as f32 * SPACING, height(gx, gz), AT[1] + gz as f32 * SPACING], "{:?} corner {i},{j}", c.at);
            }
        }
        // Chunks of fewer cells than it takes are left as they were.
        let small: Vec<GroundChunk> = piece(2, &[], |_, _| false).into_iter().map(|mut c| {
            c.cells = 2;
            c
        }).collect();
        assert_eq!(coarse(small, COARSE_BY, AT, SPACING, [last, last]).len(), 4);
    }

    #[test]
    fn a_far_cell_is_left_out_only_when_every_cell_under_it_is() {
        let last = 8 * CELLS;
        // Chunk (0, 0) gone; in chunk (1, 0) every cell left out but sample cell (11, 2); the cells
        // past sample row 40 owned by another piece.
        let gone = |gx: u32, gz: u32| (gx >= CELLS && gx < 2 * CELLS && gz < CELLS && (gx, gz) != (11, 2)) || gz >= 40;
        let far = coarse(piece(8, &[(0, 0)], gone), COARSE_BY, AT, SPACING, [last, last]);
        let first = far.iter().find(|c| c.at == (0, 0)).expect("the first far chunk");
        let missing = |i: u32, j: u32| first.missing[(j * CELLS + i) as usize];
        // Far cells (0..2, 0..2) are over chunk (0, 0); (2..4, 0..2) over (1, 0), of which only
        // (2, 0) has sample cell (11, 2) under it.
        assert!(missing(0, 0) && missing(1, 1) && missing(3, 0) && missing(3, 1) && missing(2, 1));
        assert!(!missing(2, 0), "the one sample cell keeps its far cell");
        assert!(!missing(4, 0) && !missing(0, 2));
        // Its corners are the samples', from the chunk that draws them.
        let v = first.vertices[(0 * (CELLS + 1) + 3) as usize];
        assert_eq!(v.pos[1], height(12, 0));
        // Sample row 40 is far row 10, the second far chunk's row 2: left out from there on, the
        // cut's own row kept.
        let second = far.iter().find(|c| c.at == (0, 1)).expect("the second far row");
        assert!(!second.missing[(1 * CELLS) as usize] && second.missing[(2 * CELLS) as usize]);
    }

    #[test]
    fn a_piece_goes_fine_coming_in_and_coarse_going_out() {
        let tile = Tile { name: "p".into(), at: [F(0.0), F(0.0)], layout: TerrainLayout::default(), spacing: F(62.5), levels: 1, keep: [0, 0, 16, 16], first_owner: 0 };
        let mut s = GroundStream {
            held: vec![None],
            asked: vec![None],
            failed: vec![false],
            redo: vec![false],
            tiles: vec![tile],
            reach: Reach { load: F(6000.0), drop: F(9000.0) },
            fine: Some(Reach { load: F(2500.0), drop: F(3500.0) }),
            work: Arc::new((Mutex::new((Vec::new(), false)), Condvar::new())),
            built: channel().1,
            pending: VecDeque::new(),
            chunks_a_frame: CHUNKS_A_FRAME,
            log: Vec::new(),
            heading: Vec::new(),
        };
        // Not held: past load nothing, in the far ring coarse, in the near one fine.
        assert_eq!([7000.0, 3000.0, 2000.0].map(|g| s.wanted(0, g)), [None, Some(false), Some(true)]);
        // Held coarse: fine only within the near ring's load, and kept out to the drop.
        s.held[0] = Some(false);
        assert_eq!([8000.0, 3000.0, 2500.0, 9500.0].map(|g| s.wanted(0, g)), [Some(false), Some(false), Some(true), None]);
        // Held fine: coarse again only past the near ring's drop.
        s.held[0] = Some(true);
        assert_eq!([3500.0, 3600.0].map(|g| s.wanted(0, g)), [Some(true), Some(false)]);
        // Without a near ring, everything in reach at full detail; a piece that would not build, not at all.
        s.fine = None;
        assert_eq!(s.wanted(0, 8000.0), Some(true));
        s.failed[0] = true;
        assert_eq!(s.wanted(0, 0.0), None);
    }
}
