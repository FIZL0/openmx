//! The first custom track (P71): every free-ride map on the disc spliced into one ground, ridden as a
//! world (MD8).
//!
//! **What the maps share.** The five free-ride maps are one frame filled five ways: each is 768 × 768
//! samples at 9 feet, and each has the same ring of mountains round it, the same bytes on every map —
//! a ridge about 85 samples in from each side (peaking at 240 to 260 feet), and outside it a plateau
//! at 190 feet out to the map's edge. The plateau is the shootoff ground: the original throws a rider
//! who comes within 150 feet of the map's edge (S59, `game_sim::off_map`).
//!
//! **How they are laid.** In a row, west to east in the track pick's order ([`MAPS`]), every map at its
//! own size and its own spacing, so each one's ground is its own to the bit (`game_sim::tiles`). Where
//! two meet, each is cut at its ridge line: the western map keeps its ground up to its east ridge's
//! peak, the eastern one from its west ridge's peak, and the plateaus between are gone. The ridges stay
//! as the divider between the two maps, one mountain with a map on each side, as the user asked of a
//! world of these maps (MD27: "cut off right at the peak of the mountains"). The outer side of the
//! row's first and last map and the whole north and south of every map keep their plateau, so the
//! world's outer edge is each map's own edge with the original's shootoff on it.
//!
//! **The joins.** A map's east ridge and its west ridge are not the same shape, so the two peak lines
//! do not meet height for height. Between them goes a strip of join ground one leaf wide (15 cells,
//! 135 feet), made here and never read from anywhere: at its west edge it is the western map's
//! peak-line samples, at its east edge the eastern map's, and in between each row runs straight from
//! one to the other. Its surfaces are the nearer map's. The join is the tiles' own kind of terrain
//! file ([`game_sim::tiles::generate`]), so the simulation asks it the way it asks every piece. A
//! one-level terrain is at most 16 root cells a side, so each seam is four joins along `z`.
//!
//! **What else is joined.** Each map's props, collision shapes, plants and path cars stand where its
//! scene puts them, moved by its offset ([`crate::world_dress`]); what stood on a cut plateau is left
//! out, as a world mod's are (MD27). Each map's fixed sound emitters are moved the same way
//! ([`crate::audio::Sound::set_map_emitters`]), and the ones on cut ground dropped. Each map's ground
//! is painted from its own splat and light map, and its sky and light are its own under the World sky
//! setting's Per map (MD9); the ride is lit and dressed from the first map, free06.

use crate::worlds::{self, Origin, Spawn, WorldMod, WorldPiece};
use game_render::disc::Disc;
use game_sim::tiles::{generate, Tile};
use game_sim::track::{self, Terrain, TerrainHead, TerrainLayout};
use ps2_float::F;
use std::path::PathBuf;
use std::sync::Arc;

/// The track's id: `--track`'s word, and the name its saved markers are kept under.
pub const ID: &str = crate::menu::SPLICED.0;

/// What it is called.
pub const NAME: &str = crate::menu::SPLICED.1;

/// The maps, west to east.
pub const MAPS: [&str; 5] = ["free06", "free07", "free09", "free10", "free11"];

/// Leaves across one join: one, 16 samples, 15 cells.
const JOIN_LEAVES: u32 = 1;

/// The most root cells a made terrain has along a side ([`generate::terrain`]).
const MOST_LEAVES: u32 = 16;

/// One map, read far enough to lay it out.
struct Map {
    member: String,
    second: Option<String>,
    stem: String,
    scene: track::Scene,
    scene_bytes: Arc<[u8]>,
    terrain: Vec<u8>,
}

/// A map's sample heights and surfaces, in world feet: the terrain's own samples, each scaled as the
/// ground query scales them.
struct Samples {
    n: u32,
    spacing: f32,
    height: Vec<f32>,
    surface: Vec<u8>,
}

impl Samples {
    fn read(map: &Map) -> Result<Samples, String> {
        let terrain = Terrain::parse(&map.terrain)?;
        let n = map.scene.terrain.width.max(1) * game_sim::world::SAMPLES_PER_WIDTH;
        if map.scene.terrain.breadth.max(1) * game_sim::world::SAMPLES_PER_WIDTH != n {
            return Err(format!("{}: not square", map.stem));
        }
        let spacing = terrain.scale;
        let mut height = vec![0.0; (n * n) as usize];
        let mut surface = vec![0; (n * n) as usize];
        for z in 0..n {
            for x in 0..n {
                // A cell holds its sample and the three past it; the last row and column are the
                // far corners of the cells before them.
                let (cx, cz) = (x.min(n - 2), z.min(n - 2));
                let q = terrain.cell(cx, cz, 1).ok_or_else(|| format!("{}: no cell at {cx}, {cz}", map.stem))?;
                let corner = (x - cx) as usize + 2 * (z - cz) as usize;
                height[(z * n + x) as usize] = (spacing * q.height[corner]).0;
                surface[(z * n + x) as usize] = q.surface[corner];
            }
        }
        Ok(Samples { n, spacing: spacing.0, height, surface })
    }

    fn at(&self, x: u32, z: u32) -> f32 {
        self.height[(z.min(self.n - 1) * self.n + x.min(self.n - 1)) as usize]
    }

    fn surface(&self, x: u32, z: u32) -> u8 {
        self.surface[(z.min(self.n - 1) * self.n + x.min(self.n - 1)) as usize]
    }

    /// The column of a ridge: of the columns `from..to`, the one highest on average over the middle
    /// half of the map along `z`, where the ridge runs straight (its ends turn into the north and
    /// south ridges).
    fn ridge(&self, from: u32, to: u32) -> u32 {
        let rows = self.n / 4..self.n * 3 / 4;
        let mean = |x: u32| rows.clone().map(|z| f64::from(self.at(x, z))).sum::<f64>();
        (from..to).max_by(|a, b| mean(*a).total_cmp(&mean(*b))).unwrap_or(from)
    }
}

fn read_map(disc: &mut Disc, name: &str) -> Result<Map, String> {
    let ending = |suffix: String| {
        disc.entries()
            .iter()
            .filter(|e| e.name.to_ascii_lowercase().ends_with(&suffix))
            .min_by_key(|e| e.name.len())
            .map(|e| e.name.clone())
    };
    let member = ending(format!("\\{name}.res")).ok_or_else(|| format!("the disc has no {name}.res"))?;
    let second = ending(format!("\\{name}_2.res"));
    let res = disc.res(&member)?;
    let (scn, stem, bytes) = worlds::scene_of(&res).map_err(|e| format!("{name}: {e}"))?;
    let scene = track::Scene::parse(&bytes).map_err(|e| format!("{scn}: {e}"))?;
    let terrain = crate::track_mods::terrain_bytes(&res, &scene.terrain_file).map_err(|e| format!("{name}: {e}"))?;
    Ok(Map { member, second, stem, scene, scene_bytes: bytes.into(), terrain })
}

/// Where a made join's sample `(sx, sz)` takes its height from: the western map's peak line at its
/// west edge, the eastern map's at its east edge, a straight run between.
fn join_height(west: &Samples, wx: u32, east: &Samples, ex: u32, z: u32, sx: u32, last: u32) -> f64 {
    let t = f64::from(sx.min(last)) / f64::from(last);
    let (a, b) = (f64::from(west.at(wx, z)), f64::from(east.at(ex, z)));
    a + (b - a) * t
}

/// The world, read off `disc`: [`MAPS`] in a row, cut at their ridges and joined across them.
pub fn world(disc: &mut Disc) -> Result<WorldMod, String> {
    let maps: Vec<Map> = MAPS.iter().map(|m| read_map(disc, m)).collect::<Result<_, _>>()?;
    let samples: Vec<Samples> = maps.iter().map(Samples::read).collect::<Result<_, _>>()?;
    let (n, spacing) = (samples[0].n, samples[0].spacing);
    if let Some(s) = samples.iter().find(|s| s.n != n || s.spacing != spacing) {
        return Err(format!("the free-ride maps are not one size: {} x {} feet and {} x {}", n, spacing, s.n, s.spacing));
    }
    // Each map's west and east ridge lines, by its own ground.
    let ridges: Vec<(u32, u32)> = samples.iter().map(|s| (s.ridge(0, n / 4), s.ridge(n * 3 / 4, n - 1))).collect();
    let foot = |samples: u32| samples as f32 * spacing;
    let join_samples = JOIN_LEAVES * 16;
    // The cells every tile owns along `z`: all a terrain has, `n - 1`.
    let cells = n - 1;
    let mut pieces = Vec::new();
    let mut joins = Vec::new();
    let mut first_owner = 0i32;
    let mut at_x = 0.0f32;
    for (i, map) in maps.iter().enumerate() {
        let west = if i == 0 { 0 } else { ridges[i].0 };
        let east = if i + 1 == maps.len() { cells } else { ridges[i].1 };
        let keep = [west, 0, east, cells];
        if i > 0 {
            // This map's west ridge sits on the last sample of the join before it.
            at_x -= foot(west);
        }
        let mut scene = map.scene.clone();
        let kept = worlds::keep_feet(keep, F(spacing));
        scene.collision.retain(|p| worlds::inside(kept, p.position[0].0, p.position[2].0));
        scene.models.retain(|p| worlds::inside(kept, p.position[0].0, p.position[2].0));
        let levels = TerrainHead::parse(&map.terrain).map_err(|e| format!("{}: {e}", map.stem))?.levels;
        let tile = Tile { name: map.stem.clone(), at: [F(at_x), F::ZERO], layout: scene.terrain, spacing: F(spacing), levels, keep, first_owner };
        first_owner = first_owner.saturating_add(scene.collision.len() as i32);
        pieces.push(WorldPiece {
            tile,
            dir: PathBuf::from(&map.member),
            scene: Some(scene),
            stem: map.stem.clone(),
            dropped: Vec::new(),
            scene_bytes: Some(map.scene_bytes.clone()),
            origin: Origin::Disc { track: map.member.clone(), second: map.second.clone() },
        });
        if i + 1 < maps.len() {
            // The join starts on this map's east ridge.
            let join_x = at_x + foot(east);
            let (wx, ex, next) = (east, ridges[i + 1].0, &samples[i + 1]);
            let mut z = 0;
            let mut k = 0;
            while z < cells {
                let leaves = if cells - z >= MOST_LEAVES * 16 - 1 { MOST_LEAVES } else { (cells - z).div_ceil(16).max(1) };
                let owns = leaves * 16 - 1;
                // The last one ends on the maps' last cell, over the one before it where it has to.
                let start = if z + owns > cells { cells - owns } else { z };
                let name = format!("{}_{}_{k}", map.stem, maps[i + 1].stem);
                let west_map = &samples[i];
                let bytes = generate::terrain(
                    &name,
                    (JOIN_LEAVES, leaves),
                    spacing,
                    |sx, sz| join_height(west_map, wx, next, ex, start + sz, sx, join_samples - 1),
                    |sx, sz| if sx * 2 < join_samples - 1 { west_map.surface(wx, start + sz) } else { next.surface(ex, start + sz) },
                );
                let layout = TerrainLayout { width: JOIN_LEAVES, breadth: leaves, width_scale: F(spacing), breadth_scale: F(spacing), ..TerrainLayout::default() };
                let mut t = Tile { name: name.clone(), at: [F(join_x), F(foot(start))], layout, spacing: F(spacing), levels: 1, keep: [0; 4], first_owner: 0 };
                t.keep = t.every_cell();
                joins.push(WorldPiece { tile: t, dir: PathBuf::new(), scene: None, stem: name, dropped: Vec::new(), scene_bytes: None, origin: Origin::Made(bytes.into()) });
                z = start + owns;
                k += 1;
            }
            at_x = join_x + foot(join_samples - 1);
        }
    }
    let tile_count = pieces.len();
    for mut j in joins {
        j.tile.first_owner = first_owner;
        pieces.push(j);
    }
    Ok(WorldMod { id: ID.to_string(), name: NAME.to_string(), dir: PathBuf::new(), pieces, tile_count, home: 0, spawn: Spawn::Tile(0), wrap: [false, false] })
}

/// The log's lines for the layout: where each map is and the ground it keeps, west to east, in
/// world feet, and each seam's join strip.
pub fn layout(w: &WorldMod) -> Vec<String> {
    let regions = w.regions();
    let mut out: Vec<String> = w.pieces[..w.tile_count]
        .iter()
        .zip(regions.owned)
        .map(|(p, [x0, z0, x1, z1])| format!("spliced: {} at {:.0} keeps x {x0:.0}..{x1:.0} z {z0:.0}..{z1:.0}", p.tile.name, p.tile.at[0].0))
        .collect();
    for p in &w.pieces[w.tile_count..] {
        let [sx, sz] = p.tile.samples();
        let (x, z) = (p.tile.at[0].0, p.tile.at[1].0);
        let (dx, dz) = ((sx - 1) as f32 * p.tile.spacing.0, (sz - 1) as f32 * p.tile.spacing.0);
        out.push(format!("spliced: join {} x {x:.0}..{:.0} z {z:.0}..{:.0}", p.tile.name, x + dx, z + dz));
    }
    out
}
