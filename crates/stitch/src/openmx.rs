//! A built world as openmx loads it (C11), and the ground it then answers.
//!
//! openmx reads a world mod in its shell, `crates/openmx/src/worlds.rs` (`find`, and its `Files`
//! source), which is a binary and cannot be linked. [`load`] repeats `find`'s steps on the same
//! readers (game-render's `Res::folder`, game-sim's `Scene::parse` and `Terrain::parse`) to the same
//! game-sim [`Tile`]s, and [`World::tiled`] hands them to game-sim's own [`Tiled`], which is what
//! the ride asks for its ground. What is checked here is then that `Tiled`'s answer, not a second
//! reader's:
//!
//! - [`tile_ground`]: under every tile, the world's ground is the imported map's own, to the bit,
//!   through both of the port's queries (`Tiled::at` and the collision search's `contact_search::ground`).
//! - [`join_ground`]: under every join, the world's ground is the join's file alone, to the bit, and at
//!   its samples the height stitch solved, to the join's height quantum.
//!
//! Each also reports how far a world point's move into the piece's own coordinates rounded (the
//! ultra map runs past 116,000 ft, where a float holds 1/128 ft).

use game_render::disc::Res;
use game_sim::contact_search;
use game_sim::tiles::{Piece as SimPiece, Reach, Source, Tile, Tiled};
use game_sim::track::{Scene, Terrain, TerrainLayout, TREE_ARITY};
use game_sim::world::Ground;
use crate::json::Json;
use ps2_float::{F, Vf};
use std::path::{Path, PathBuf};

/// One tile or join of a world, as openmx's `WorldPiece` has it.
#[derive(Clone, Debug)]
pub struct Piece {
    pub tile: Tile,
    pub dir: PathBuf,
    /// A tile's scene file name in its folder (found as openmx's `scene_of` finds it); `None` for a join.
    pub scene: Option<String>,
    /// The scene's terrain file name, or the join's name.
    pub terrain: String,
}

/// A world mod read as openmx reads it.
pub struct World {
    pub id: String,
    pub dir: PathBuf,
    /// Tiles, then joins, in the file's order.
    pub pieces: Vec<Piece>,
    pub tile_count: usize,
}

/// The one `.scn` of a tile folder, as openmx's `scene_of`.
fn scene_of(res: &Res) -> Result<(String, Vec<u8>), String> {
    let scenes: Vec<&str> = res.entries().iter().map(|e| e.name.as_str()).filter(|n| n.to_ascii_lowercase().ends_with(".scn")).collect();
    let [scn] = scenes.as_slice() else { return Err(format!("{} .scn files, and a tile has one", scenes.len())) };
    Ok((scn.to_string(), res.read(scn)?))
}

/// openmx's `track_mods::terrain_bytes`: the scene's terrain by suffix, or its `.ps2.` form.
pub fn terrain_bytes(res: &Res, named: &str) -> Result<Vec<u8>, String> {
    if let Ok(bytes) = res.read_suffix(named) {
        return Ok(bytes);
    }
    let tagged = match named.rsplit_once('.') {
        Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
        None => return Err(format!("no file ending in {named}")),
    };
    res.read_suffix(&tagged).map_err(|_| format!("no file ending in {named} or {tagged}"))
}

/// openmx's `shape`: a terrain's spacing, levels and the root cells its tree has.
fn shape(bytes: &[u8]) -> Result<(F, u32, u32, u32), String> {
    let t = Terrain::parse(bytes)?;
    let root = t.root().ok_or("no tree")?;
    let (mut w, mut b) = (0, 0);
    for cz in 0..TREE_ARITY {
        for cx in 0..TREE_ARITY {
            if t.child(&root, cx, cz).is_some() {
                w = w.max(cx + 1);
                b = b.max(cz + 1);
            }
        }
    }
    Ok((t.scale, t.levels, w.max(1), b.max(1)))
}

/// A join's terrain file, as openmx's `join_terrain`.
fn join_terrain(folder: &Path, join: &str) -> Result<Vec<u8>, String> {
    std::fs::read(folder.join(format!("{join}.ps2.trn")))
        .or_else(|_| std::fs::read(folder.join(format!("{join}.trn"))))
        .map_err(|e| format!("{}: {e}", folder.join(format!("{join}.ps2.trn")).display()))
}

/// A track mod folder's scene and terrain, read as openmx reads a mod track (or a world's tile).
pub fn track(dir: &Path) -> Result<(Scene, Vec<u8>), String> {
    let res = Res::folder(dir)?;
    let (_, bytes) = scene_of(&res)?;
    let scene = Scene::parse(&bytes)?;
    let terrain = terrain_bytes(&res, &scene.terrain_file)?;
    Ok((scene, terrain))
}

/// openmx's `pair`: two numbers, as f64 off their text.
fn pair(v: Option<&Json>, what: &str) -> Result<[f64; 2], String> {
    match v.and_then(Json::arr) {
        Some([a, b]) => match (a.f64(), b.f64()) {
            (Some(a), Some(b)) => Ok([a, b]),
            _ => Err(format!("{what} is not two numbers")),
        },
        _ => Err(format!("{what} is not two numbers")),
    }
}

/// A world mod folder, read as openmx's `worlds::find` reads `mods/tracks/<id>`. The `drop`ped
/// sections only change a tile's collision count (its shapes' owner numbers), never its ground, so
/// they are not taken out here and every tile's `first_owner` is left 0.
pub fn load(dir: &Path) -> Result<World, String> {
    let text = std::fs::read_to_string(dir.join("mod.json")).map_err(|e| format!("{}: {e}", dir.display()))?;
    let json = Json::parse(&text)?;
    if json.get("kind").and_then(Json::str) != Some("world") {
        return Err(format!("{}: not a world", dir.display()));
    }
    let id = json.get("id").and_then(Json::str).ok_or("no id")?.to_string();
    let list = |key: &str| json.get(key).and_then(Json::arr).map(<[Json]>::to_vec).unwrap_or_default();
    let mut pieces = Vec::new();
    for t in list("tiles") {
        let name = t.get("tile").and_then(Json::str).ok_or("a tile with no name")?.to_string();
        let at = pair(t.get("at"), &format!("{name}'s at"))?;
        let keep: Vec<u32> = t.get("keep").and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::f64).map(|n| n as u32).collect();
        let keep: [u32; 4] = keep.try_into().map_err(|_| format!("{name}'s keep is not four numbers"))?;
        let folder = dir.join("tiles").join(&name);
        let res = Res::folder(&folder).map_err(|e| format!("{name}: {e}"))?;
        let (scn, bytes) = scene_of(&res).map_err(|e| format!("{name}: {e}"))?;
        let scene = Scene::parse(&bytes).map_err(|e| format!("{name}: {e}"))?;
        let terrain = terrain_bytes(&res, &scene.terrain_file).map_err(|e| format!("{name}: {e}"))?;
        let (spacing, levels, _, _) = shape(&terrain).map_err(|e| format!("{name}: {e}"))?;
        let tile = Tile { name, at: at.map(|a| F(a as f32)), layout: scene.terrain.clone(), spacing, levels, keep, first_owner: 0 };
        pieces.push(Piece { tile, dir: folder, scene: Some(scn), terrain: scene.terrain_file.clone() });
    }
    let tile_count = pieces.len();
    for j in list("joins") {
        let name = j.get("join").and_then(Json::str).ok_or("a join with no name")?.to_string();
        let at = pair(j.get("at"), &format!("{name}'s at"))?;
        let folder = dir.join("joins").join(&name);
        let terrain = join_terrain(&folder, &name)?;
        let (spacing, levels, width, breadth) = shape(&terrain).map_err(|e| format!("{name}: {e}"))?;
        let layout = TerrainLayout { width, breadth, width_scale: spacing, breadth_scale: spacing, ..TerrainLayout::default() };
        let mut tile = Tile { name: name.clone(), at: at.map(|a| F(a as f32)), layout, spacing, levels, keep: [0; 4], first_owner: 0 };
        tile.keep = tile.every_cell();
        pieces.push(Piece { tile, dir: folder, scene: None, terrain: name });
    }
    Ok(World { id, dir: dir.to_path_buf(), pieces, tile_count })
}

/// The pieces' terrains off their folders, as openmx's `Files` source (without the shapes: the
/// ground queries here never ask for them).
struct Files(Vec<Piece>);

impl Source for Files {
    fn load(&mut self, i: usize) -> Result<SimPiece, String> {
        let p = self.0.get(i).ok_or("no such piece")?;
        let terrain = match &p.scene {
            None => join_terrain(&p.dir, &p.terrain)?,
            Some(_) => terrain_bytes(&Res::folder(&p.dir)?, &p.terrain)?,
        };
        Ok(SimPiece { terrain, shapes: Vec::new() })
    }
}

impl World {
    /// game-sim's world over the pieces, as `WorldMod::tiled` makes it.
    pub fn tiled(&self) -> Result<Tiled, String> {
        Tiled::new(self.pieces.iter().map(|p| p.tile.clone()).collect(), Box::new(Files(self.pieces.clone())), Reach::DESKTOP)
    }

    /// The piece named `name`.
    pub fn index(&self, name: &str) -> Option<usize> {
        self.pieces.iter().position(|p| p.tile.name == name)
    }

    /// A piece's terrain file, read alone.
    pub fn terrain_of(&self, i: usize) -> Result<Vec<u8>, String> {
        Files(vec![self.pieces[i].clone()]).load(0).map(|p| p.terrain)
    }
}

/// What one piece's comparison came to.
#[derive(Clone, Debug, Default)]
pub struct Compared {
    /// Points asked whose owner was this piece.
    pub points: usize,
    /// Points asked that another piece owned (a join under a tile's keep or an earlier join).
    pub elsewhere: usize,
    /// Points where `Tiled` answered differently from the piece alone (either query, any bit), with
    /// the first few written out.
    pub differ: usize,
    pub first: Vec<String>,
    /// Join samples compared with stitch's field, and the largest difference past the quantum.
    pub field_points: usize,
    pub field_worst: f64,
    pub field_over: usize,
    /// The largest distance, in feet, between the point meant in the piece's coordinates and where
    /// the world's float put it (world − at).
    pub rounding: f32,
}

impl Compared {
    pub fn add(&mut self, o: &Compared) {
        self.points += o.points;
        self.elsewhere += o.elsewhere;
        self.differ += o.differ;
        self.field_points += o.field_points;
        self.field_worst = self.field_worst.max(o.field_worst);
        self.field_over += o.field_over;
        self.rounding = self.rounding.max(o.rounding);
        for f in &o.first {
            if self.first.len() < 8 {
                self.first.push(f.clone());
            }
        }
    }
}

/// Ask the world at the point `local` of piece `i` means, and the piece alone (`alone`, in its own
/// coordinates) at the point the world's float arithmetic moved it to, both ways; `field` is stitch's
/// height there, for a join sample.
fn ask(tiled: &Tiled, i: usize, alone: &Ground, local: [F; 2], field: Option<f64>, quantum: f64, out: &mut Compared) {
    let at = tiled.tiles[i].at;
    let (x, z) = (at[0] + local[0], at[1] + local[1]);
    let Some((owner, lx, lz)) = tiled.owner(x, z) else {
        out.elsewhere += 1;
        return;
    };
    if owner != i {
        out.elsewhere += 1;
        return;
    }
    out.points += 1;
    out.rounding = out.rounding.max((lx - local[0]).0.abs()).max((lz - local[1]).0.abs());
    let mut wrong = |what: String| {
        out.differ += 1;
        if out.first.len() < 8 {
            out.first.push(format!("{} at ({}, {}): {what}", tiled.tiles[i].name, x.0, z.0));
        }
    };
    let (got, want) = (tiled.at(x, z), alone.at(lx, lz));
    let bits = |h: &game_sim::world::Hit| (h.y.0.to_bits(), h.surface, h.normal.0.map(f32::to_bits));
    match (&got, &want) {
        (Some(g), Some(w)) if bits(g) == bits(w) => {}
        _ => wrong(format!("Tiled::at {:?}, alone {:?}", got.map(|h| h.y.0), want.map(|h| h.y.0))),
    }
    let g = contact_search::ground(&tiled.field(), Vf::new(x.0, 1.0e5, z.0, 1.0));
    let a = contact_search::ground(&alone.field(), Vf::new(lx.0, 1.0e5, lz.0, 1.0));
    if g.point.y().to_bits() != a.point.y().to_bits() || g.normal.0.map(f32::to_bits) != a.normal.0.map(f32::to_bits) || g.surfaces != a.surfaces {
        wrong(format!("contact_search::ground {} against {}", g.point.y(), a.point.y()));
    }
    if let (Some(h), Some(g)) = (field, got) {
        out.field_points += 1;
        let d = (f64::from(g.y.0) - h.max(0.0)).abs();
        let tol = quantum + 1e-4 * h.abs().max(1.0);
        if d > tol {
            out.field_over += 1;
            out.field_worst = out.field_worst.max(d - quantum);
            if out.first.len() < 8 {
                out.first.push(format!("{} sample at ({}, {}): {} against stitch's {h:.4}", tiled.tiles[i].name, x.0, z.0, g.y.0));
            }
        }
    }
}

/// The points of a piece's own cells `[x0, z0, x1, z1]` to ask at: `n × n` spread over them, off the
/// samples, and a line just inside each of the four edges, where one piece's ground hands over to the
/// next. In the piece's own feet.
fn points(keep: [u32; 4], spacing: F, n: u32) -> Vec<[F; 2]> {
    let [x0, z0, x1, z1] = keep.map(|k| k as f32 * spacing.0);
    let (w, d) = (x1 - x0, z1 - z0);
    let mut out = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let fx = (i as f32 + 0.5 + 0.21 * ((i * 7 + j * 3) % 5) as f32 - 0.42) / n as f32;
            let fz = (j as f32 + 0.5 + 0.19 * ((i * 3 + j * 11) % 5) as f32 - 0.38) / n as f32;
            out.push([F(x0 + fx * w), F(z0 + fz * d)]);
        }
    }
    let inset = (0.05 * spacing.0).max(0.01);
    for k in 0..4 * n {
        let f = (k as f32 + 0.37) / (4 * n) as f32;
        out.push([F(x0 + inset), F(z0 + f * d)]);
        out.push([F(x1 - inset), F(z0 + f * d)]);
        out.push([F(x0 + f * w), F(z0 + inset)]);
        out.push([F(x0 + f * w), F(z1 - inset)]);
    }
    out
}

/// Tile `i` of the world against the imported map in `imported` (its own track mod folder, as openmx
/// rides it alone): the world's ground at `n × n` points of the tile's keep and along its four edges.
pub fn tile_ground(world: &World, tiled: &Tiled, i: usize, imported: &Path, n: u32) -> Result<Compared, String> {
    let (scene, bytes) = track(imported)?;
    let terrain = Terrain::parse(&bytes)?;
    let alone = Ground::new(&terrain, &scene.terrain);
    let p = &world.pieces[i].tile;
    let mut out = Compared::default();
    for local in points(p.keep, p.spacing, n) {
        ask(tiled, i, &alone, local, None, 0.0, &mut out);
    }
    Ok(out)
}

/// Join `i` of the world against its own file alone, at `n × n` points and along its edges, and at
/// every `stride`th sample against `field` (stitch's height at a world point, in f64 feet), where
/// `field` is given.
pub fn join_ground(world: &World, tiled: &Tiled, i: usize, n: u32, stride: u32, field: Option<&dyn Fn(f64, f64) -> f64>) -> Result<Compared, String> {
    let bytes = world.terrain_of(i)?;
    let terrain = Terrain::parse(&bytes)?;
    let hs = terrain.root().map_or(F::ZERO, |r| r.height_scale);
    let quantum = f64::from((terrain.scale * hs).0);
    let p = &world.pieces[i].tile;
    let alone = Ground::new(&terrain, &p.layout);
    let mut out = Compared::default();
    for local in points(p.keep, p.spacing, n) {
        ask(tiled, i, &alone, local, None, quantum, &mut out);
    }
    if let Some(field) = field {
        let [x0, z0, x1, z1] = p.keep;
        for kz in (z0..z1).step_by(stride.max(1) as usize) {
            for kx in (x0..x1).step_by(stride.max(1) as usize) {
                let local = [F::from(kx as i32) * p.spacing, F::from(kz as i32) * p.spacing];
                let (wx, wz) = (f64::from(p.at[0].0) + f64::from(local[0].0), f64::from(p.at[1].0) + f64::from(local[1].0));
                ask(tiled, i, &alone, local, Some(field(wx, wz)), quantum, &mut out);
            }
        }
    }
    Ok(out)
}

/// The world's rectangle on a grid `step` feet apart: how many points no piece owns.
pub fn holes(tiled: &Tiled, step: f32) -> (usize, usize, Vec<[f32; 2]>) {
    let (w, d) = (tiled.area.width.0, tiled.area.depth.0);
    let (mut asked, mut none, mut first) = (0, 0, Vec::new());
    let mut z = step / 2.0;
    while z < d {
        let mut x = step / 2.0;
        while x < w {
            asked += 1;
            if tiled.owner(F(x), F(z)).is_none() {
                none += 1;
                if first.len() < 8 {
                    first.push([x, z]);
                }
            }
            x += step;
        }
        z += step;
    }
    (asked, none, first)
}

/// A whole world's comparison: every tile against its imported map, every join against its file and
/// (where `field` is given) stitch's heights, on `threads` threads, each with its own [`Tiled`] that
/// drops a piece once it is done with it, so the ultra map's 2 GB are never all held.
pub struct Summary {
    pub tiles: Compared,
    pub joins: Compared,
    /// Per tile, its own result, in tile order.
    pub per_tile: Vec<(String, Compared)>,
    /// The world's rectangle every 64 ft: points asked, and how many no piece owns.
    pub asked: usize,
    pub holes: usize,
    pub first_holes: Vec<[f32; 2]>,
    pub width: f32,
    pub depth: f32,
}

pub fn check_world(
    world: &World,
    imported: &(dyn Fn(&str) -> PathBuf + Sync),
    field: Option<&(dyn Fn(f64, f64) -> f64 + Sync)>,
    n: u32,
    stride: u32,
    threads: usize,
) -> Result<Summary, String> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, Result<Compared, String>)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| {
                let mut tiled = match world.tiled() {
                    Ok(t) => t,
                    Err(e) => {
                        results.lock().expect("lock").push((usize::MAX, Err(e)));
                        return;
                    }
                };
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= world.pieces.len() {
                        break;
                    }
                    let r = if i < world.tile_count {
                        tile_ground(world, &tiled, i, &imported(&world.pieces[i].tile.name), n)
                    } else {
                        join_ground(world, &tiled, i, n, stride, field.map(|f| f as &dyn Fn(f64, f64) -> f64))
                    };
                    tiled.stream(&[]);
                    results.lock().expect("lock").push((i, r));
                }
            });
        }
    });
    let mut results = results.into_inner().expect("lock");
    results.sort_by_key(|r| r.0);
    let (mut tiles, mut joins, mut per_tile) = (Compared::default(), Compared::default(), Vec::new());
    for (i, r) in results {
        let c = r.map_err(|e| if i == usize::MAX { e } else { format!("{}: {e}", world.pieces[i].tile.name) })?;
        if i < world.tile_count {
            tiles.add(&c);
            per_tile.push((world.pieces[i].tile.name.clone(), c));
        } else {
            joins.add(&c);
        }
    }
    let tiled = world.tiled()?;
    let (asked, holes, first_holes) = holes(&tiled, 64.0);
    Ok(Summary { tiles, joins, per_tile, asked, holes, first_holes, width: tiled.area.width.0, depth: tiled.area.depth.0 })
}

/// Where openmx would leave ground to no one inside the world: points a quarter foot outside each
/// side of every piece's owned rectangle, every `step` feet along it, that are inside the world's
/// rectangle and owned by nothing. A gap between two pieces always runs along one of these lines, so
/// this finds every strip a dense grid would, at a fraction of the questions. Returns the points asked,
/// the ones with no owner, and the first few with the piece beside them.
pub fn strips(world: &World, tiled: &Tiled, step: f32) -> (usize, usize, Vec<String>) {
    let (w, d) = (tiled.area.width.0, tiled.area.depth.0);
    let (mut asked, mut none, mut first) = (0, 0, Vec::new());
    for p in &world.pieces {
        let t = &p.tile;
        let [x0, z0, x1, z1] = t.keep.map(|k| k as f32 * t.spacing.0);
        let (ax, az) = (t.at[0].0, t.at[1].0);
        let (x0, z0, x1, z1) = (ax + x0, az + z0, ax + x1, az + z1);
        let mut line = |from: [f32; 2], to: [f32; 2]| {
            let len = (to[0] - from[0]).abs().max((to[1] - from[1]).abs());
            let n = (len / step).ceil().max(1.0) as u32;
            for k in 0..=n {
                let f = k as f32 / n as f32;
                let (x, z) = (from[0] + f * (to[0] - from[0]), from[1] + f * (to[1] - from[1]));
                if !(x > 0.0 && z > 0.0 && x < w && z < d) {
                    continue;
                }
                asked += 1;
                if tiled.owner(F(x), F(z)).is_none() {
                    none += 1;
                    if first.len() < 8 {
                        first.push(format!("({x}, {z}) beside {}", t.name));
                    }
                }
            }
        };
        let e = 0.25;
        line([x0 - e, z0], [x0 - e, z1]);
        line([x1 + e, z0], [x1 + e, z1]);
        line([x0, z0 - e], [x1, z0 - e]);
        line([x0, z1 + e], [x1, z1 + e]);
    }
    (asked, none, first)
}
