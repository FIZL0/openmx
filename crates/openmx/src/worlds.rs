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
use game_sim::track::{self, Placement, Terrain, TerrainLayout};
use ps2_float::F;
use std::path::{Path, PathBuf};

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
fn without_sections(text: &str, drop: &[String]) -> String {
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
fn scene_of(res: &Res) -> Result<(String, String, Vec<u8>), String> {
    let scenes: Vec<&str> = res.entries().iter().map(|e| e.name.as_str()).filter(|n| n.to_ascii_lowercase().ends_with(".scn")).collect();
    let [scn] = scenes.as_slice() else { return Err(format!("{} .scn files, and a tile has one", scenes.len())) };
    let stem = scn.rsplit(['\\', '/']).next().unwrap_or(scn);
    let stem = stem[..stem.len() - 4].to_string();
    Ok((scn.to_string(), stem, res.read(scn)?))
}

/// A terrain's spacing, tree levels and the root cells its tree has, across and along.
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

/// One world mod by its id: its `mod.json`, every tile's scene and every terrain's header read and
/// checked, nothing else loaded.
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
        let (_, stem, bytes) = scene_of(&res).map_err(|e| format!("{id}: {tile}: {e}"))?;
        let sections: Vec<String> = drop
            .iter()
            .filter(|d| d.get("tile").and_then(Json::str) == Some(tile.as_str()))
            .filter_map(|d| d.get("section").and_then(Json::str).map(str::to_string))
            .collect();
        let text = String::from_utf8_lossy(&bytes);
        let scene = track::Scene::parse(without_sections(&text, &sections).as_bytes()).map_err(|e| format!("{id}: {tile}: {e}"))?;
        let terrain = track_mods::terrain_bytes(&res, &scene.terrain_file).map_err(|e| format!("{id}: {tile}: {e}"))?;
        let (spacing, levels, _, _) = shape(&terrain).map_err(|e| format!("{id}: {tile}: {e}"))?;
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
        pieces.push(WorldPiece { tile: t, dir: folder, scene: Some(scene), stem });
    }
    let tile_count = pieces.len();
    if tile_count == 0 {
        return Err(format!("{id}: a world with no tiles"));
    }
    for j in list("joins")? {
        let join = j.get("join").and_then(Json::str).ok_or_else(|| format!("{id}: a join with no name"))?.to_string();
        let at = pair(j.get("at"), &format!("{id}: {join}'s at"))?;
        let folder = dir.join("joins").join(&join);
        let terrain = join_terrain(&folder, &join).map_err(|e| format!("{id}: {join}: {e}"))?;
        let (spacing, levels, width, breadth) = shape(&terrain).map_err(|e| format!("{id}: {join}: {e}"))?;
        let layout = TerrainLayout { width, breadth, width_scale: spacing, breadth_scale: spacing, ..TerrainLayout::default() };
        let mut t = Tile { name: join.clone(), at: at.map(|a| F(a as f32)), layout, spacing, levels, keep: [0; 4], first_owner };
        t.keep = t.every_cell();
        pieces.push(WorldPiece { tile: t, dir: folder, scene: None, stem: join });
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

/// Every piece's ground as the GPU draws it: each piece's own chunks of `cells` cells, moved by its
/// offset, with every cell it does not own cut out (the simulation's [`Tiled::owner`] at the
/// cell's middle, so the drawn seams are the ridden ones). Each piece's chunks are numbered in a
/// column band of their own, so the GPU stitches a piece's chunks to each other and to nothing else.
pub fn chunks(w: &WorldMod, tiled: &Tiled, cells: u32) -> Vec<game_render::gpu::GroundChunk> {
    let cells = cells.max(1).next_power_of_two();
    let mut out = Vec::new();
    let mut column = 0u32;
    let mut files = Files { pieces: w.pieces.clone() };
    for (i, p) in w.pieces.iter().enumerate() {
        let bytes = match files.load_terrain(i) {
            Ok(b) => b,
            Err(e) => {
                println!("world: {} not drawn: {e}", p.tile.name);
                continue;
            }
        };
        let Ok(terrain) = Terrain::parse(&bytes) else { continue };
        let ground = crate::ground::Ground::new(terrain, &p.tile.layout);
        let [ax, az] = p.tile.at.map(|a| a.0);
        let spacing = ground.spacing;
        let mut widest = 0;
        for mut c in ground.chunks(cells) {
            let (x0, z0) = (c.at.0 * cells, c.at.1 * cells);
            let mut missing = if c.missing.is_empty() { vec![false; (cells * cells) as usize] } else { c.missing.clone() };
            for j in 0..cells {
                for i2 in 0..cells {
                    let (sx, sz) = (x0 + i2, z0 + j);
                    let (x, z) = (ax + (sx as f32 + 0.5) * spacing, az + (sz as f32 + 0.5) * spacing);
                    if tiled.owner(F(x), F(z)).map(|(o, _, _)| o) != Some(i) {
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
            widest = widest.max(c.at.0 + 1);
            c.at.0 += column;
            out.push(c);
        }
        column += widest + 1;
    }
    out
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
