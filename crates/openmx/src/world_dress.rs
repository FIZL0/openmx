//! A world's tiles dressed (MD15): every tile's props, ecosystem and path cars, each read from the
//! tile's own folder and moved by the tile's offset, loaded and let go by distance as the riders go.
//!
//! The ground streams the same way ([`crate::worlds::GroundStream`]); this is what stands on it.
//! A tile within [`Reach::load`] of a rider is built on a thread of its own, nearest first: its
//! scene's props (the world's `drop`ped sections left out), its ecosystem stood on its own terrain,
//! and the models of the cars its `.toy` drives, all into an [`Assets`] of its own. The shell takes
//! that in under the tile's name ([`Assets::take_in`]), so two tiles' `Model1` are two models, and
//! lets it go again ([`Assets::let_go`]) once the tile is past [`Reach::drop`] of every rider.
//!
//! Every tile is dressed this way, the home tile too: in a world nothing is the home tile's alone
//! but its light and sky.

use crate::ground;
use crate::worlds::{self, WorldMod, WorldPiece};
use game_render::disc::Res;
use game_render::eco::{Grove, Lod};
use game_render::scene::{self as view, Placement};
use game_render::toy::{self, Traffic};
use game_render::world::{Assets, Taken};
use game_render::ini;
use game_sim::tiles::{Reach, Tile, Tiled};
use std::collections::HashMap;
use game_sim::track;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Condvar, Mutex};

/// One tile's dressing as its builder made it: everything in world feet, every model name the
/// tile's own ([`prefix`]).
pub struct Built {
    pub index: usize,
    pub assets: Assets,
    pub props: Vec<Placement>,
    pub grove: Option<Grove>,
    /// The cars' models, by their names in the tile's own folder.
    pub cars: usize,
    /// What did not come out, for the log.
    pub notes: Vec<String>,
}

/// A tile's dressing as the ride holds it.
pub struct Dressed {
    pub index: usize,
    pub prefix: String,
    /// Its props, moved and named for the ride's [`Assets`].
    pub props: Vec<Placement>,
    pub grove: Option<Grove>,
    /// Its `.toy`'s cars, driving in the tile's own feet ([`Dressed::shift`] places them).
    pub traffic: Traffic,
    /// The tile's offset, `x` and `z`.
    pub shift: [f32; 2],
    /// What the ride's assets took in for it.
    pub taken: Taken,
}

impl Dressed {
    /// Every car's model, by its name in the ride's assets, and where it is in the world, `elapsed`
    /// seconds of the scene's clock in, on `ground` asked in world feet.
    pub fn cars(&self, elapsed: f64, ground: &mut dyn FnMut(game_render::mat::V3, [f32; 2]) -> (game_render::mat::V3, game_render::mat::V3)) -> Vec<(String, game_render::mat::M4)> {
        let shift = self.shift;
        let mut placed = self.traffic.placed(elapsed, &mut |p| ground(p, shift));
        for (name, at) in &mut placed {
            *name = format!("{}{name}", self.prefix);
            at.0[3][0] += shift[0];
            at.0[3][2] += shift[1];
        }
        placed
    }
}

/// The name a tile's models are taken in under: `<tile>::<model>`.
pub fn prefix(tile: &Tile) -> String {
    format!("{}::", tile.name)
}

/// Build tile `index`'s dressing from its folder.
pub fn build(piece: &WorldPiece, index: usize, lod: Lod) -> Result<Built, String> {
    let Some(scene) = &piece.scene else { return Err("a join has nothing on it".into()) };
    let res = Res::folder(&piece.dir)?;
    let archives = [res];
    // The scene as the world's find read it (MD18), else off the folder.
    let bytes = match &piece.scene_bytes {
        Some(kept) => kept.to_vec(),
        None => worlds::scene_of(&archives[0])?.2,
    };
    let text = worlds::without_sections(&ini::text(&bytes), &piece.dropped);
    let mut look = view::Scene::parse(&text);
    let mut assets = Assets::default();
    let mut notes = Vec::new();
    let mut names: Vec<String> = look.models.iter().map(|p| p.model.clone()).collect();
    names.sort();
    names.dedup();
    let missing = names.iter().filter(|n| !assets.load_placed(&archives, n)).count();
    if missing > 0 {
        notes.push(format!("{missing} of {} models missing", names.len()));
    }
    // Its trees and bushes, stood on its own terrain in its own feet, as the tile ridden alone has them.
    let terrain = crate::track_mods::terrain_bytes(&archives[0], &scene.terrain_file);
    let grove = match terrain.as_deref().map_err(|e| e.clone()).and_then(track::Terrain::parse) {
        Ok(terrain) => match Grove::load(&archives, &look, &scene.terrain, &terrain, &mut assets) {
            Ok(g) => g,
            Err(e) => {
                notes.push(format!("ecosystem: {e}"));
                None
            }
        },
        Err(e) => {
            notes.push(format!("ecosystem: {e}"));
            None
        }
    };
    // The cars its `.toy` drives: their models now, their clips on the ride's thread, which stands
    // them on the ground.
    let mut cars = 0;
    if let Ok(toy_text) = archives[0].read_suffix(&format!("{}.toy", piece.stem)) {
        for car in toy::cars(&ini::text(&toy_text)).iter().filter(|c| !c.has_physics) {
            cars += assets.load_placed(&archives, car.stem()) as usize;
        }
    }
    let [dx, dz] = piece.tile.at.map(|a| a.0);
    let prefix = prefix(&piece.tile);
    let props: Vec<Placement> = std::mem::take(&mut look.models)
        .into_iter()
        .filter(|p| assets.models.contains_key(&p.model))
        .map(|mut p| {
            p.pos.x += dx;
            p.pos.z += dz;
            p.model = format!("{prefix}{}", p.model);
            p
        })
        .collect();
    let grove = grove.map(|mut g| {
        for plant in &mut g.ecosystem.plants {
            plant.pos.x += dx;
            plant.pos.z += dz;
        }
        for name in g.models.iter_mut().chain(&mut g.billboards).chain(&mut g.matched).chain(&mut g.cut) {
            *name = format!("{prefix}{name}");
        }
        g.lod = lod;
        g
    });
    Ok(Built { index, assets, props, grove, cars, notes })
}

#[derive(Clone, Copy, PartialEq)]
enum Held {
    Out,
    Asked,
    In,
}

/// What a [`DressStream`] asks of the ride: a tile to take in, or one to let go.
pub enum Change {
    Add(Built),
    Drop(usize),
}

/// The world's tiles dressed by distance, as [`crate::worlds::GroundStream`] streams the ground.
pub struct DressStream {
    tiles: Vec<Tile>,
    reach: Reach,
    held: Vec<Held>,
    work: Arc<(Mutex<(Vec<usize>, bool)>, Condvar)>,
    built: Receiver<(usize, Result<Built, String>)>,
    log: Vec<String>,
}

impl DressStream {
    /// The stream, and the tiles it dressed before the ride: those within `reach.load` of `near`,
    /// built on every core.
    pub fn start(w: &WorldMod, reach: Reach, near: &[[f32; 2]], lod: Lod) -> (DressStream, Vec<Built>) {
        let pieces: Vec<WorldPiece> = w.pieces[..w.tile_count].to_vec();
        let tiles: Vec<Tile> = pieces.iter().map(|p| p.tile.clone()).collect();
        let first: Vec<usize> = (0..tiles.len()).filter(|&i| worlds::nearest(&tiles[i], near) <= reach.load.0).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).min(first.len()).max(1);
        let mut ready: Vec<(usize, Result<Built, String>)> = std::thread::scope(|scope| {
            let jobs: Vec<_> = (0..threads)
                .map(|t| {
                    let (first, pieces) = (&first, &pieces);
                    scope.spawn(move || first.iter().skip(t).step_by(threads).map(|&i| (i, build(&pieces[i], i, lod))).collect::<Vec<_>>())
                })
                .collect();
            jobs.into_iter().flat_map(|j| j.join().expect("a tile's dresser")).collect()
        });
        ready.sort_by_key(|(i, _)| *i);
        let work = Arc::new((Mutex::new((Vec::new(), false)), Condvar::new()));
        let (send, built) = channel();
        let theirs = work.clone();
        std::thread::spawn(move || {
            loop {
                let next = {
                    let (lock, wake) = &*theirs;
                    let mut todo = lock.lock().expect("the dressing's work");
                    while todo.0.is_empty() && !todo.1 {
                        todo = wake.wait(todo).expect("the dressing's work");
                    }
                    if todo.1 {
                        return;
                    }
                    todo.0.pop().expect("work")
                };
                if send.send((next, build(&pieces[next], next, lod))).is_err() {
                    return;
                }
            }
        });
        let mut stream = DressStream { held: vec![Held::Out; tiles.len()], tiles, reach, work, built, log: Vec::new() };
        let out = ready.into_iter().filter_map(|(i, r)| stream.arrived(i, r)).collect();
        (stream, out)
    }

    fn arrived(&mut self, i: usize, built: Result<Built, String>) -> Option<Built> {
        let name = &self.tiles[i].name;
        self.held[i] = Held::In;
        match built {
            Ok(b) => {
                let plants = b.grove.as_ref().map_or(0, |g| g.ecosystem.plants.len());
                let notes = if b.notes.is_empty() { String::new() } else { format!(" ({})", b.notes.join("; ")) };
                self.log.push(format!(
                    "world props: {name} built, {} props, {plants} plants, {} car models{notes}",
                    b.props.len(),
                    b.cars
                ));
                Some(b)
            }
            Err(e) => {
                self.log.push(format!("world props: {name} not drawn: {e}"));
                None
            }
        }
    }

    /// Ask for the tiles within reach of `near` nearest first, take what the builder finished, and
    /// let go of the tiles past reach. Nothing waits: a tile's props come in a frame or two late
    /// rather than the ride stopping for them.
    pub fn update(&mut self, near: &[[f32; 2]]) -> Vec<Change> {
        let gaps: Vec<f32> = self.tiles.iter().map(|t| worlds::nearest(t, near)).collect();
        let mut out = Vec::new();
        {
            let (lock, wake) = &*self.work;
            let mut todo = lock.lock().expect("the dressing's work");
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
        while let Ok((i, r)) = self.built.try_recv() {
            if gaps[i] > self.reach.drop.0 {
                self.held[i] = Held::Out;
            } else if let Some(b) = self.arrived(i, r) {
                out.push(Change::Add(b));
            }
        }
        for i in 0..self.tiles.len() {
            if self.held[i] == Held::In && gaps[i] > self.reach.drop.0 {
                self.held[i] = Held::Out;
                self.log.push(format!("world props: {} dropped", self.tiles[i].name));
                out.push(Change::Drop(i));
            }
        }
        out
    }

    /// The lines building and dropping wrote since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.log)
    }
}

impl Drop for DressStream {
    fn drop(&mut self) {
        let (lock, wake) = &*self.work;
        if let Ok(mut todo) = lock.lock() {
            todo.1 = true;
            wake.notify_one();
        }
    }
}

/// Take a built tile into the ride: its models and textures into `assets` under its name, and its
/// cars onto their clips, stood on `ground` (asked in world feet).
pub fn take(
    b: Built,
    w: &WorldMod,
    assets: &mut Assets,
    ground: &mut dyn FnMut(game_render::mat::V3, [f32; 2]) -> (game_render::mat::V3, game_render::mat::V3),
) -> Dressed {
    let piece = &w.pieces[b.index];
    let prefix = prefix(&piece.tile);
    let shift = piece.tile.at.map(|a| a.0);
    let taken = assets.take_in(b.assets, &prefix);
    let traffic = match Res::folder(&piece.dir) {
        Ok(res) => Traffic::load(&[res], &piece.stem, &mut |p| ground(p, shift)),
        Err(_) => Traffic::default(),
    };
    Dressed { index: b.index, prefix, props: b.props, grove: b.grove, traffic, shift, taken }
}

/// A world's ground as the software renderer draws it (MD15): every piece within [`ground::REACH`]
/// of `around`, each cell by the piece that owns it as the ride's ground does ([`Tiled::owner`]),
/// painted by the world's paint (MD9) or dirt. A piece's terrain is read once and kept in `files`
/// while it is near. The triangles drawn.
#[allow(clippy::too_many_arguments)]
pub fn draw_ground(
    frame: &mut game_render::raster::Frame,
    camera: &game_render::world::Camera,
    light: &game_render::world::Lighting,
    around: game_render::mat::V3,
    w: &WorldMod,
    tiled: &Tiled,
    files: &mut HashMap<usize, Vec<u8>>,
    paint: Option<&game_render::terrain::WorldPaint>,
) -> usize {
    let here = [[around.x, around.z]];
    let near: Vec<usize> = (0..w.pieces.len()).filter(|&i| worlds::nearest(&w.pieces[i].tile, &here) <= ground::REACH).collect();
    files.retain(|i, _| worlds::nearest(&w.pieces[*i].tile, &here) <= 4.0 * ground::REACH);
    let mut batch = game_render::raster::Batch::new(frame.width, frame.height);
    let mut drawn = 0;
    for &i in &near {
        if !files.contains_key(&i) {
            match w.piece_terrain(i) {
                Ok(bytes) => files.insert(i, bytes),
                Err(_) => continue,
            };
        }
        let Some(terrain) = files.get(&i).and_then(|b| track::Terrain::parse(b).ok()) else { continue };
        let tile = &w.pieces[i].tile;
        let mut g = ground::Ground::new(terrain, &tile.layout);
        [g.samples_x, g.samples_z] = tile.samples();
        let at = tile.at.map(|a| a.0);
        let spacing = g.spacing;
        let owns = |sx: u32, sz: u32| worlds::owns_cell(tiled, i, at, spacing, sx, sz);
        drawn += g.queue_piece(&mut batch, camera, (frame.width, frame.height), light, around, g.radius(), at, &owns);
    }
    let dirt = ground::DIRT;
    match paint {
        Some(p) => ground::draw_world(frame, &batch, light, &|x, z| p.paint(x, z)),
        None => ground::draw_world(frame, &batch, light, &|_, _| dirt),
    }
    drawn
}

/// How many pixels a side a baked page is ([`bake_splat`]): 64 samples to 128 pixels, four and a
/// half feet a pixel on a disc map.
pub const BAKED_PAGE: usize = 128;

/// A disc map's ground paint as terrain pages (MD15), so a world paints it as it paints a mod map's
/// own (MD9): its splat ([`Splat::paint`]) under its baked light map where it has one, both read in
/// the map's own feet, a page per four by four leaves as [`Pages::page_at`] reads them. Where the
/// light map reaches, the light is in the paint, so the paint is divided by the sun on the stored
/// normal that the world's ground multiplies it by again; where it does not, the paint goes in bare.
/// `None` for a terrain with no leaves.
pub fn bake_splat(
    splat: &game_render::splat::Splat,
    terrain: &track::Terrain,
    spacing: f32,
    (across, along): (u32, u32),
    light: &game_render::world::Lighting,
) -> Option<game_render::terrain::Pages> {
    use game_render::pst::Texture;
    const LEAF: u32 = 16;
    const PAGE_LEAVES: u32 = 4;
    const PAGE_SAMPLES: u32 = LEAF * PAGE_LEAVES;
    // `Pages::page_at`'s border and inner share of a page.
    const BORDER: f32 = 1.0 / 32.0;
    const INNER: f32 = 15.0 / 16.0;
    let has_leaf = |lx: u32, lz: u32| terrain.cell(lx * LEAF + LEAF / 2, lz * LEAF + LEAF / 2, 1).is_some();
    let (bx_n, bz_n) = (across.div_ceil(PAGE_LEAVES), along.div_ceil(PAGE_LEAVES));
    let mut page_of = vec![None; (across * along) as usize];
    let mut blocks = Vec::new();
    for bz in 0..bz_n {
        for bx in 0..bx_n {
            let leaves: Vec<(u32, u32)> = (0..PAGE_LEAVES * PAGE_LEAVES)
                .map(|k| (bx * PAGE_LEAVES + k % PAGE_LEAVES, bz * PAGE_LEAVES + k / PAGE_LEAVES))
                .filter(|&(lx, lz)| lx < across && lz < along && has_leaf(lx, lz))
                .collect();
            if leaves.is_empty() {
                continue;
            }
            let page = u16::try_from(blocks.len()).ok()?;
            for (lx, lz) in leaves {
                page_of[(lz * across + lx) as usize] = Some(page);
            }
            blocks.push((bx, bz));
        }
    }
    if blocks.is_empty() {
        return None;
    }
    let bake = |&(bx, bz): &(u32, u32)| -> Texture {
        let n = BAKED_PAGE;
        let (sx0, sz0) = (bx * PAGE_SAMPLES, bz * PAGE_SAMPLES);
        // The sun on each sample's stored normal, a row of the page's samples and one more.
        let side = (PAGE_SAMPLES + 1) as usize;
        let sunlit: Vec<[f32; 3]> = (0..side * side)
            .map(|k| {
                let (sx, sz) = (sx0 + (k % side) as u32, sz0 + (k / side) as u32);
                let q = terrain.cell(sx, sz, 1).or_else(|| terrain.cell(sx.saturating_sub(1), sz.saturating_sub(1), 1));
                let lambert = q.map_or(1.0, |q| {
                    let n = q.normal[0];
                    let normal = game_render::mat::v3(n[0].0, n[1].0, n[2].0).norm();
                    (-normal.dot(light.sun)).max(0.0)
                });
                [0, 1, 2].map(|c| (light.ambient[c] + light.sun_color[c] * lambert).max(0.2))
            })
            .collect();
        let mut rgba = Vec::with_capacity(n * n * 4);
        for py in 0..n {
            for px in 0..n {
                let on = |p: usize| ((((p as f32 + 0.5) / n as f32) - BORDER) / INNER * PAGE_SAMPLES as f32).clamp(0.0, PAGE_SAMPLES as f32);
                let (lx, lz) = (on(px), on(py));
                let (x, z) = ((sx0 as f32 + lx) * spacing, (sz0 as f32 + lz) * spacing);
                let c = splat.paint(x, z);
                let c = match splat.light(x, z) {
                    Some(l) => {
                        let s = sunlit[(lz as usize).min(side - 1) * side + (lx as usize).min(side - 1)];
                        [0, 1, 2].map(|k| (c[k] as f32 * l / s[k]).clamp(0.0, 255.0) as u8)
                    }
                    None => c,
                };
                rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        Texture { width: n, height: n, rgba }
    };
    let pages = blocks.iter().map(bake).collect();
    Some(game_render::terrain::Pages { pages, across, along, page_of, spacing, detail: None, detail_repeat: spacing * 8.0 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ps2_float::F;
    use game_sim::tiles::generate;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    const SPACING: f32 = 9.0;

    fn write(root: &Path, rel: &str, bytes: impl AsRef<[u8]>) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    /// A world of three tiles in a row along `x`, 720 feet apart, 64 samples (567 feet) a side,
    /// each a track mod of its own with a scene and a terrain and nothing standing on it.
    fn row_world(test: &str) -> PathBuf {
        let mods = std::env::temp_dir().join(format!("openmx-md15-{}-{test}", std::process::id())).join("mods");
        let _ = std::fs::remove_dir_all(&mods);
        let world = mods.join("tracks").join("md15_row");
        let mut tiles = Vec::new();
        for col in 0..3u32 {
            let (name, at) = (format!("t{col}"), 720 * col);
            let stem = name.to_uppercase();
            let dir = world.join("tiles").join(&name);
            write(&dir, "mod.json", format!(r#"{{"standard": 1, "kind": "track", "id": "md15_{name}", "name": "MD15 {name}", "source": {{"game": "Real elevation"}}, "version": "1.0.0", "author": "test", "permission": "public-domain", "todo": []}}"#));
            let scene = [
                "[SceneInfo]".to_string(),
                format!("SceneName=MD15 {name}"),
                "DefaultStartPosition=200.00,40.00,200.00".into(),
                "DefaultStartDirection=1.00,0.00,0.00".into(),
                "[Environment]".into(),
                format!("TerrainFile={stem}.trn"),
                "TerrainWidth=4".into(),
                "TerrainBreadth=4".into(),
                format!("TerrainWidthScale={SPACING:.2}"),
                format!("TerrainBreadthScale={SPACING:.2}"),
                "TerrainBorderWidth=0.00".into(),
                "MinDivisionSize=96.00".into(),
                String::new(),
            ]
            .join("\r\n");
            write(&dir, &format!("Env/{stem}/{stem}.scn"), scene);
            let ox = f64::from(at);
            let terrain = generate::terrain(&name, (4, 4), SPACING, |x, z| 40.0 + 1.5 * ((ox + f64::from(x) * 9.0) / 140.0).sin() * (f64::from(z) * 9.0 / 90.0).cos(), |_, _| 1);
            write(&dir, &format!("Env/{stem}/{stem}.ps2.trn"), terrain);
            tiles.push(format!(r#"{{"tile": "{name}", "at": [{at}.0, 0.0], "keep": [0, 0, 63, 63]}}"#));
        }
        write(
            &world,
            "mod.json",
            format!(
                r#"{{"standard": 1, "kind": "world", "id": "md15_row", "name": "MD15 row", "source": {{"game": "Combined", "games": ["Real elevation"]}}, "version": "1.0.0", "author": "test", "permission": "public-domain", "tiles": [{}], "joins": [], "drop": [], "sky": "t0", "spawn": {{"tile": "t0"}}, "todo": []}}"#,
                tiles.join(", ")
            ),
        );
        mods
    }

    /// Update until `done` says the changes so far are enough, or ten seconds pass.
    fn until<C>(mut update: impl FnMut() -> Vec<C>, done: impl Fn(&[C]) -> bool) -> Vec<C> {
        let start = Instant::now();
        let mut all = Vec::new();
        while !done(&all) && start.elapsed() < Duration::from_secs(10) {
            all.extend(update());
            std::thread::sleep(Duration::from_millis(5));
        }
        all
    }

    #[test]
    fn a_far_tile_is_dropped_and_built_again_when_the_rider_comes_back() {
        let mods = row_world("back");
        let w = worlds::find(&mods, "md15_row").unwrap();
        let reach = Reach { load: F(200.0), drop: F(300.0) };
        let (home, far) = ([[200.0f32, 200.0]], [[1600.0f32, 200.0]]);

        // The ground: t0 is built before the ride, let go out at the far end, and built again, the
        // same chunks, back at the start.
        let (mut ground, first) = worlds::GroundStream::start(&w, 16, reach, &home, false);
        let t0: Vec<_> = first.iter().filter(|(i, _)| *i == 0).map(|(_, c)| c.iter().map(|c| (c.at, c.vertices.len())).collect::<Vec<_>>()).collect();
        assert_eq!(first.iter().map(|(i, _)| *i).collect::<Vec<_>>(), vec![0], "only t0 is in reach of its start");
        let away = until(|| ground.update(&far), |c| c.iter().any(|c| matches!(c, worlds::Change::Drop(0))));
        assert!(away.iter().any(|c| matches!(c, worlds::Change::Drop(0))), "t0 was not dropped at the far end");
        let back = until(|| ground.update(&home), |c| c.iter().any(|c| matches!(c, worlds::Change::Add(0, _))));
        let again: Vec<_> = back.iter().filter_map(|c| match c {
            worlds::Change::Add(0, chunks) => Some(chunks.iter().map(|c| (c.at, c.vertices.len())).collect::<Vec<_>>()),
            _ => None,
        }).collect();
        assert_eq!(again, t0, "t0's ground built again, as it was built first");
        let log = ground.take_log().join("\n");
        assert!(log.contains("t0 dropped") && log.matches("t0 built").count() >= 2, "{log}");

        // Its dressing the same way, with nothing waiting for it.
        let (mut dress, first) = DressStream::start(&w, reach, &home, Lod::default());
        assert_eq!(first.iter().map(|b| b.index).collect::<Vec<_>>(), vec![0]);
        let away = until(|| dress.update(&far), |c| c.iter().any(|c| matches!(c, Change::Drop(0))) && c.iter().any(|c| matches!(c, Change::Add(b) if b.index == 2)));
        assert!(away.iter().any(|c| matches!(c, Change::Drop(0))), "t0's dressing was not dropped at the far end");
        assert!(away.iter().any(|c| matches!(c, Change::Add(b) if b.index == 2)), "t2, under the rider, was not dressed");
        let back = until(|| dress.update(&home), |c| c.iter().any(|c| matches!(c, Change::Add(b) if b.index == 0)));
        assert!(back.iter().any(|c| matches!(c, Change::Add(b) if b.index == 0)), "t0's dressing was not built again");
        assert!(back.iter().any(|c| matches!(c, Change::Drop(2))), "t2 was not dropped back at the start");
        let log = dress.take_log().join("\n");
        assert!(log.contains("world props: t0 dropped") && log.matches("world props: t0 built").count() == 2, "{log}");
        let _ = std::fs::remove_dir_all(mods.parent().unwrap());
    }
}
