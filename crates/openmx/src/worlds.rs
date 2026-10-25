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
use game_render::gpu::GroundChunk;
use ps2_float::F;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

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
/// whole file: what [`shape_of_file`] reads from its header and root alone.
#[cfg(test)]
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
    for t in list("tiles")? {
        let tile = t.get("tile").and_then(Json::str).ok_or_else(|| format!("{id}: a tile with no name"))?.to_string();
        let at = pair(t.get("at"), &format!("{id}: {tile}'s at"))?;
        let keep = match t.get("keep") {
            Some(Json::Arr(a)) if a.len() == 4 => {
                let n: Vec<u32> = a.iter().filter_map(|n| n.num().and_then(|s| s.parse::<f64>().ok())).map(|n| n as u32).collect();
                <[u32; 4]>::try_from(n).map_err(|_| format!("{id}: {tile}'s keep is not four numbers"))?
            }
            _ => return Err(format!("{id}: {tile}'s keep is not four numbers")),
        };
        let folder = dir.join("tiles").join(&tile);
        let res = Res::folder(&folder).map_err(|e| format!("{id}: {tile}: {e}"))?;
        let (scn, stem, bytes) = scene_of(&res).map_err(|e| format!("{id}: {tile}: {e}"))?;
        let sections: Vec<String> = drop
            .iter()
            .filter(|d| d.get("tile").and_then(Json::str) == Some(tile.as_str()))
            .filter_map(|d| d.get("section").and_then(Json::str).map(str::to_string))
            .collect();
        let text = String::from_utf8_lossy(&bytes);
        let mut scene = track::Scene::parse(without_sections(&text, &sections).as_bytes()).map_err(|e| format!("{id}: {tile}: {scn}: {e}"))?;
        let terrain = track_mods::terrain_path(&res, &folder, &scene.terrain_file).map_err(|e| format!("{id}: {tile}: {e}"))?;
        let (spacing, levels, _, _) = shape_of_file(&terrain).map_err(|e| format!("{id}: {tile}: {e}"))?;
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
        pieces.push(WorldPiece { tile: t, dir: folder, scene: Some(scene), stem, dropped: sections, scene_bytes: Some(bytes.into()) });
    }
    let tile_count = pieces.len();
    if tile_count == 0 {
        return Err(format!("{id}: a world with no tiles"));
    }
    for j in list("joins")? {
        let join = j.get("join").and_then(Json::str).ok_or_else(|| format!("{id}: a join with no name"))?.to_string();
        let at = pair(j.get("at"), &format!("{id}: {join}'s at"))?;
        let folder = dir.join("joins").join(&join);
        let (spacing, levels, width, breadth) = shape_of_file(&join_terrain_path(&folder, &join)).map_err(|e| format!("{id}: {join}: {e}"))?;
        let layout = TerrainLayout { width, breadth, width_scale: spacing, breadth_scale: spacing, ..TerrainLayout::default() };
        let mut t = Tile { name: join.clone(), at: at.map(|a| F(a as f32)), layout, spacing, levels, keep: [0; 4], first_owner };
        t.keep = t.every_cell();
        pieces.push(WorldPiece { tile: t, dir: folder, scene: None, stem: join, dropped: Vec::new(), scene_bytes: None });
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
    Ok(WorldMod { id: id.to_string(), name, dir, pieces, tile_count, home, spawn })
}

/// Where a join's terrain file is: `<join>.ps2.trn`, or `<join>.trn` where that is not there.
fn join_terrain_path(folder: &Path, join: &str) -> PathBuf {
    let tagged = folder.join(format!("{join}.ps2.trn"));
    if tagged.is_file() { tagged } else { folder.join(format!("{join}.trn")) }
}

/// A tile's `keep` in its own feet, `[x0, z0, x1, z1]`, by the scene's sample spacing.
fn keep_feet(keep: [u32; 4], spacing: F) -> [f32; 4] {
    keep.map(|k| k as f32 * spacing.0)
}

/// Whether a point in a tile's own feet stands inside its kept rectangle.
fn inside(kept: [f32; 4], x: f32, z: f32) -> bool {
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

    /// The simulation's world, reading each piece from its folder when it is first needed.
    pub fn tiled(&self, reach: Reach) -> Result<Tiled, String> {
        let files = Files { pieces: self.pieces.clone() };
        Tiled::new(self.pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(files), reach)
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
}

/// The world's pieces on disk, as the simulation's [`Source`].
struct Files {
    pieces: Vec<WorldPiece>,
}

impl Source for Files {
    fn load(&mut self, index: usize) -> Result<Piece, String> {
        let p = self.pieces.get(index).ok_or("no such piece")?;
        let Some(scene) = &p.scene else {
            return Ok(Piece { terrain: join_terrain(&p.dir, &p.stem)?, shapes: Vec::new() });
        };
        let res = Res::folder(&p.dir)?;
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
        let res = Res::folder(&p.dir)?;
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
/// A piece is built from its folder by [`piece_chunks`], the same chunks whenever it is built.
pub struct GroundStream {
    tiles: Vec<Tile>,
    reach: Reach,
    held: Vec<Held>,
    /// What the builder has to do, nearest last, and whether it should stop.
    work: Arc<(Mutex<(Vec<usize>, bool)>, Condvar)>,
    built: Receiver<(usize, Result<Vec<GroundChunk>, String>)>,
    /// What building and dropping did since the shell last took the lines.
    log: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Held {
    Out,
    /// Asked of the builder, or being built.
    Asked,
    In,
}

/// What a [`GroundStream`] asks of the renderer: a piece's chunks to add, or a piece to drop.
pub enum Change {
    Add(usize, Vec<GroundChunk>),
    Drop(usize),
}

impl GroundStream {
    /// The stream, and the pieces it built before the ride: those within `reach.load` of `near`,
    /// or with `all` every one (the whole world before the first frame, as before MD17). The
    /// pieces are built on every core.
    pub fn start(w: &WorldMod, cells: u32, reach: Reach, near: &[[f32; 2]], all: bool) -> (GroundStream, Vec<(usize, Vec<GroundChunk>)>) {
        let tiles: Vec<Tile> = w.pieces.iter().map(|p| p.tile.clone()).collect();
        let first: Vec<usize> = (0..tiles.len()).filter(|&i| all || nearest(&tiles[i], near) <= reach.load.0).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(first.len()).max(1);
        let mut ready: Vec<(usize, Result<Vec<GroundChunk>, String>)> = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..threads)
                .map(|t| {
                    let (first, pieces) = (&first, &w.pieces);
                    scope.spawn(move || {
                        let mut b = Builder::new(pieces.clone(), reach, cells);
                        first.iter().skip(t).step_by(threads).map(|&i| (i, b.build(i))).collect::<Vec<_>>()
                    })
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().expect("a ground builder")).collect()
        });
        ready.sort_by_key(|(i, _)| *i);
        let work = Arc::new((Mutex::new((Vec::new(), false)), Condvar::new()));
        let (send, built) = channel();
        let (pieces, theirs) = (w.pieces.clone(), work.clone());
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
                if send.send((next, b.build(next))).is_err() {
                    return;
                }
            }
        });
        let mut stream = GroundStream { held: vec![Held::Out; tiles.len()], tiles, reach, work, built, log: Vec::new() };
        let mut out = Vec::new();
        for (i, r) in ready {
            if let Some(chunks) = stream.arrived(i, r) {
                out.push((i, chunks));
            }
        }
        (stream, out)
    }

    /// A piece the builder finished: held, with its line, or `None` with the line why not.
    fn arrived(&mut self, i: usize, built: Result<Vec<GroundChunk>, String>) -> Option<Vec<GroundChunk>> {
        let name = &self.tiles[i].name;
        match built {
            Ok(chunks) => {
                self.log.push(format!("world ground: {name} built, {} chunks", chunks.len()));
                self.held[i] = Held::In;
                Some(chunks)
            }
            Err(e) => {
                // Not asked again: its files do not change under a ride.
                self.log.push(format!("world ground: {name} not drawn: {e}"));
                self.held[i] = Held::In;
                None
            }
        }
    }

    /// Ask for the pieces within reach of `near` nearest first, take what the builder finished, and
    /// let go of the pieces past reach. A piece a rider is over that is not built yet is waited for,
    /// and the wait said.
    pub fn update(&mut self, near: &[[f32; 2]]) -> Vec<Change> {
        let gaps: Vec<f32> = self.tiles.iter().map(|t| nearest(t, near)).collect();
        let mut out = Vec::new();
        {
            let (lock, wake) = &*self.work;
            let mut todo = lock.lock().expect("the ground's work");
            for &i in &todo.0 {
                self.held[i] = Held::Out;
            }
            let mut want: Vec<usize> = (0..self.tiles.len()).filter(|&i| self.held[i] == Held::Out && gaps[i] <= self.reach.load.0).collect();
            want.sort_by(|a, b| gaps[*b].total_cmp(&gaps[*a]));
            for &i in &want {
                self.held[i] = Held::Asked;
            }
            todo.0 = want;
            wake.notify_one();
        }
        let mut take = |s: &mut GroundStream, (i, r): (usize, Result<Vec<GroundChunk>, String>)| {
            if gaps[i] > s.reach.drop.0 {
                s.held[i] = Held::Out;
            } else if let Some(chunks) = s.arrived(i, r) {
                out.push(Change::Add(i, chunks));
            }
        };
        while let Ok(done) = self.built.try_recv() {
            take(self, done);
        }
        while let Some(i) = (0..self.tiles.len()).find(|&i| gaps[i] == 0.0 && self.held[i] == Held::Asked) {
            let at = Instant::now();
            let Ok(done) = self.built.recv() else { break };
            self.log.push(format!("world ground: waited {:.0} ms for {}", at.elapsed().as_secs_f32() * 1000.0, self.tiles[i].name));
            take(self, done);
        }
        for i in 0..self.tiles.len() {
            if self.held[i] == Held::In && gaps[i] > self.reach.drop.0 {
                self.held[i] = Held::Out;
                self.log.push(format!("world ground: {} dropped", self.tiles[i].name));
                out.push(Change::Drop(i));
            }
        }
        out
    }

    /// How many pieces are held now.
    pub fn held(&self) -> usize {
        self.held.iter().filter(|&&h| h == Held::In).count()
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

    fn build(&mut self, i: usize) -> Result<Vec<GroundChunk>, String> {
        let bytes = self.files.load_terrain(i)?;
        let tiled = self.tiled.as_ref().ok_or("the world does not read")?;
        piece_chunks(&self.files.pieces[i].tile, i, &bytes, tiled, self.cells)
    }
}

impl Files {
    fn load_terrain(&mut self, index: usize) -> Result<Vec<u8>, String> {
        let p = &self.pieces[index];
        match &p.scene {
            None => join_terrain(&p.dir, &p.stem),
            Some(scene) => track_mods::terrain_bytes(&Res::folder(&p.dir)?, &scene.terrain_file),
        }
    }
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
    /// The world rectangles each tile owns, in the world's order: its `keep` cells moved by its offset.
    pub fn regions(&self) -> game_render::terrain::Regions {
        let owned = self.pieces[..self.tile_count]
            .iter()
            .map(|p| {
                let [x0, z0, x1, z1] = p.tile.keep.map(|k| k as f32 * p.tile.spacing.0);
                let [ax, az] = p.tile.at.map(|a| a.0);
                [ax + x0, az + z0, ax + x1, az + z1]
            })
            .collect();
        game_render::terrain::Regions { owned }
    }

    /// Every tile's ground paint, light and sky, read from its own folder: its terrain's pages with its
    /// own detail map (MD4), its scene's lights and fog, and the sky it names — looked for in its
    /// folder first and then in `fallback`, the ride's other archives, as the ride itself looks.
    /// A tile whose terrain has no pages (a disc map's) has pages baked from its splat and light map
    /// ([`crate::world_dress::bake_splat`], MD15), or is dirt without them; one that does not read is
    /// dirt and the home tile's light. The tiles are read on every core.
    pub fn look(&self, fallback: &[Res], dirt: [f32; 3], home: game_render::world::Lighting) -> WorldLook {
        let regions = self.regions();
        let (mut tiles, mut lights, mut skies, mut names) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        // Each map read on a core of its own (MD15: a disc map's paint is baked, which takes a while).
        let read_one = |p: &WorldPiece| {
            let read = || -> Result<_, String> {
                let res = Res::folder(&p.dir)?;
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
                // A disc map has no pages: its splat and baked light, baked into some (MD15).
                let pages = pages.or_else(|| {
                    let [sx, sz] = p.tile.samples();
                    let extent = ((sx - 1) as f32 * p.tile.spacing.0, (sz - 1) as f32 * p.tile.spacing.0);
                    let splat = game_render::splat::Splat::load(std::slice::from_ref(&res), &stem, extent)?;
                    let light = game_render::world::Lighting::of(&look);
                    crate::world_dress::bake_splat(&splat, &terrain, p.tile.spacing.0, leaves, &light)
                });
                move_look(&mut look, p.tile.at);
                let sky = game_render::sky::Sky::load(std::slice::from_ref(&res), &text).or_else(|| game_render::sky::Sky::load(fallback, &text));
                Ok((pages, game_render::world::Lighting::of(&look), sky))
            };
            read().unwrap_or_else(|e| {
                println!("world: {} looks like dirt: {e}", p.tile.name);
                (None, home, None)
            })
        };
        let maps = &self.pieces[..self.tile_count];
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(maps.len()).max(1);
        let mut read: Vec<(usize, _)> = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..threads)
                .map(|t| {
                    let read_one = &read_one;
                    scope.spawn(move || maps.iter().enumerate().skip(t).step_by(threads).map(|(i, p)| (i, read_one(p))).collect::<Vec<_>>())
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().expect("a map's look")).collect()
        });
        read.sort_by_key(|(i, _)| *i);
        for (p, (_, (pages, light, sky))) in maps.iter().zip(read) {
            let at = p.tile.at.map(|a| a.0);
            tiles.push(game_render::terrain::PaintTile { at, pages });
            lights.push(light);
            skies.push(sky);
            names.push(p.tile.name.clone());
        }
        WorldLook {
            paint: game_render::terrain::WorldPaint { regions: regions.clone(), tiles, dirt },
            looks: game_render::scene::Looks { regions, lights },
            skies,
            names,
        }
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
}
