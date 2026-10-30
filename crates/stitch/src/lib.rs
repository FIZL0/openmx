//! The map joiner: several track mods laid out as one world mod (standard §3), none of them resampled.
//!
//! Each map is a **tile**: its track mod folder copied whole, placed at an offset `at` that is a whole
//! number of its leaves. Its edge berm band is found from its terrain and its scene ([`Tile::find_keep`]),
//! and only the interior `keep` is used. The gaps between the interiors are new ground, the **joins**:
//! PLN terrains written with T2's writer on one lattice for the whole world ([`build`]), whose heights
//! are a blend solved in closed form over the gap:
//!
//! - Every tile is extended past each edge of its interior by its own ground's height and slope at that
//!   edge, the slope fading out over a taper ([`Source::extend`]).
//! - The extensions are blended with modified Shepard weights of the distance to each interior,
//!   `((R − d′)₊ / (R d′))²`, `d′` the distance less one lattice step. A weight's gradient is zero
//!   where it reaches 1, so the blend meets every interior with the interior's own height and slope.
//! - The first lattice ring outside an interior is pinned to the extension itself, so the first join
//!   cell has the tile's last cell's slope exactly (up to the join's height quantum).
//!
//! The same weights are the join's look (`<join>.blend`, MD9): 255 for a tile at its edge, fading to the
//! others across the gap, never a hard edge. The surface byte is the nearest tile's at its edge.
//!
//! [`check`] reads a world back with openmx's readers and proves the seams, the slopes and the interiors.

pub mod build;
pub mod check;
pub mod jsonw;
pub mod openmx;
pub mod json;

use game_sim::track::{ActivityModel, Scene, Terrain, switch};
use crate::json::Json;
use ps2_float::F;
use std::path::{Path, PathBuf};

/// The game's off-map margin: within this of the edge a rider is thrown back (game-sim
/// `off_map::MARGIN`: 150 on both the bike's and the biplane's test). A tile's own edge
/// is no edge in a world, so at least this much of each side is cut.
pub const SHOOTOFF: f64 = 150.0;
/// How far past the shootoff band the berm is looked for, at most.
pub const MAX_BERM: f64 = 400.0;
/// The berm ends where the edge profile stops falling inward faster than this (feet per foot).
pub const BERM_FALL: f64 = 0.02;
/// The profile is compared over this many samples.
const BERM_WINDOW: usize = 4;
/// How far past the berm a ridge is looked for, at most (M23): MX Unleashed's freeride maps ring their
/// ground with a mountain ridge whose peak stands about 640 ft inside the berm.
pub const RIDGE_REACH: f64 = 1200.0;
/// A ridge stands this much over the berm's height, and falls this much again inward of its peak.
pub const RIDGE_RISE: f64 = 10.0;
/// A ridge is cut only behind a plateau: the profile within [`RIDGE_FLAT`] of the berm's height for at
/// least this far inward of it (MX Unleashed's freeride maps: 477 ft of flat 189.9 ft shootoff ground on
/// every side). No other map of the three discs that a world joins has one; without it the rule would cut
/// into a track wherever its ground climbs inward of the edge.
pub const RIDGE_PLATEAU: f64 = 2.0 * SHOOTOFF;
/// How far from the berm's height the plateau may stray, feet.
pub const RIDGE_FLAT: f64 = 1.0;
/// Ground steeper than this across a cut line, feet per foot (72°: no bike climbs it), is a rock face,
/// and no seam runs through one ([`Tile::find_cut`], openmx MD53). The join's first cell takes the tile's
/// last cell's slope, so openmx's `--seams`, rays an eighth of a foot either side of the seam, reads a
/// quarter of it as a step: this is its 0.75 ft. nat17's corner mountain is 10 to 30 a foot through every
/// line across it; the berm cuts of nat10, nat20 and nat21 were 3.05 to 3.72 and move in 1 to 6 samples;
/// no map of MX vs. ATV's or Untamed's worlds moves.
pub const ROCK: f64 = 3.0;

/// The stitcher's own version, bumped whenever a build of the same tiles writes other joins (P232):
/// the shell's cache of the disc's worlds (`crates/openmx/src/disc_worlds.rs`) is keyed by it, so a
/// changed stitcher builds them again. Not [`build::AUTHOR`], which mod.json carries as it was.
pub const VERSION: u32 = 1;

/// One track mod folder read for joining.
pub struct Tile {
    /// The folder's name, which is the mod's id and the tile's name in the world.
    pub id: String,
    pub dir: PathBuf,
    pub meta: Json,
    /// The scene's path in the folder, `/`-separated.
    pub scene_file: String,
    pub scene_text: String,
    pub scene: Scene,
    pub terrain_file: String,
    /// The terrain as `Terrain::parse` takes it (through trnwrite's MD1 view when it has pages).
    terrain: Vec<u8>,
    /// Feet a sample (the terrain's own scale).
    pub spacing: f64,
    pub scale: F,
    /// The last sample on each axis: the field is samples `0..=n`, its edge `n × spacing`.
    pub n: (u32, u32),
    /// The ground that is there, `[x0, z0, x1, z1]` in samples, half-open in cells: the largest
    /// rectangle of whole leaves (16 samples) inside `0..n` whose cells all exist ([`whole_cells`]).
    /// The whole field on most maps; less where the tree has holes (MX vs. ATV's nat04 has no leaves
    /// past x = 896 of its 1024, C8). The interior is cut inside it, so no missing cell is ever read.
    pub ground: [u32; 4],
}

/// The largest rectangle of whole leaves whose cells all exist, `[x0, z0, x1, z1]` in samples.
///
/// A leaf is there when the cells at its first and last corner are (the cells of one leaf come from one
/// block). The rectangle is the largest by area (the first found of equals, rows in z and then x), by
/// the histogram method over the leaf grid.
pub fn whole_cells(t: &Terrain, n: (u32, u32)) -> [u32; 4] {
    const LEAF: u32 = 16;
    let (w, h) = ((n.0 / LEAF) as usize, (n.1 / LEAF) as usize);
    let there = |i: usize, j: usize| {
        let (x, z) = (i as u32 * LEAF, j as u32 * LEAF);
        t.cell(x, z, 1).is_some() && t.cell(x + LEAF - 1, z + LEAF - 1, 1).is_some()
    };
    let mut run = vec![0usize; w];
    let (mut best, mut area) = ([0u32; 4], 0usize);
    for j in 0..h {
        for (i, r) in run.iter_mut().enumerate() {
            *r = if there(i, j) { *r + 1 } else { 0 };
        }
        // Each column's run up to this row, widened left and right while the runs are as tall.
        for i in 0..w {
            let tall = run[i];
            if tall == 0 {
                continue;
            }
            let lo = (0..i).rev().take_while(|&k| run[k] >= tall).last().unwrap_or(i);
            let hi = (i + 1..w).take_while(|&k| run[k] >= tall).last().unwrap_or(i);
            let a = tall * (hi + 1 - lo);
            if a > area {
                area = a;
                best = [lo, j + 1 - tall, hi + 1, j + 1].map(|v| v as u32 * LEAF);
            }
        }
    }
    best
}

/// A wrapped world's `spawn` tile (openmx MD50): of the tiles whose own start (`[x, y, z]` in their
/// own feet) stands inside their kept rectangle, the one whose kept rectangle's middle is nearest the
/// middle of the world's `extent`, so the ride starts as far from the seams as it can. Each tile is
/// `(at, keep, spacing, start)`; the first on a tie, `None` where no start is kept. openmx picks
/// the same on load (its `WorldMod::middle_tile`, over the extent with its seam joins).
pub fn middle_tile(tiles: &[([f64; 2], [u32; 4], f64, [f64; 3])], extent: [f64; 2]) -> Option<usize> {
    let mut best: Option<(f64, usize)> = None;
    for (i, &(at, keep, spacing, start)) in tiles.iter().enumerate() {
        let kept = keep.map(|k| f64::from(k) * spacing);
        if !(start[0] >= kept[0] && start[2] >= kept[1] && start[0] < kept[2] && start[2] < kept[3]) {
            continue;
        }
        let mid = [at[0] + (kept[0] + kept[2]) / 2.0, at[1] + (kept[1] + kept[3]) / 2.0];
        let d = (mid[0] - extent[0] / 2.0).powi(2) + (mid[1] - extent[1] / 2.0).powi(2);
        if best.is_none_or(|(b, _)| d < b) {
            best = Some((d, i));
        }
    }
    best.map(|(_, i)| i)
}

/// Every file under a folder, relative and `/`-separated, in sorted order.
pub fn files(dir: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, at: &Path, out: &mut Vec<String>) -> Result<(), String> {
        let rd = std::fs::read_dir(at).map_err(|e| format!("{}: {e}", at.display()))?;
        for e in rd {
            let e = e.map_err(|e| format!("{}: {e}", at.display()))?;
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out)?;
            } else {
                let rel = p.strip_prefix(root).expect("under the root");
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out)?;
    out.sort();
    Ok(out)
}

/// The files a name finds as the port finds them: the end of the path, compared in lowercase (§2).
fn lookup<'a>(files: &'a [String], name: &str) -> Vec<&'a String> {
    let want = name.replace('\\', "/").to_ascii_lowercase();
    files
        .iter()
        .filter(|f| {
            let f = f.to_ascii_lowercase();
            f == want || f.ends_with(&format!("/{want}"))
        })
        .collect()
}

impl Tile {
    /// Read a track mod folder: its mod.json, its one scene, the terrain the scene names.
    pub fn load(dir: &Path) -> Result<Tile, String> {
        let id = dir
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .ok_or_else(|| format!("{}: not a folder name", dir.display()))?;
        let text = std::fs::read_to_string(dir.join("mod.json")).map_err(|e| format!("{id}/mod.json: {e}"))?;
        let meta = Json::parse(&text).map_err(|e| format!("{id}/mod.json: {e}"))?;
        if meta.get("kind").and_then(Json::str) != Some("track") {
            return Err(format!("{id}: not a track mod (a world's tile is a track, §3)"));
        }
        let all = files(dir)?;
        let scenes: Vec<&String> = all.iter().filter(|f| f.to_ascii_lowercase().ends_with(".scn")).collect();
        let [scene_file] = scenes.as_slice() else {
            return Err(format!("{id}: {} .scn files, a track has one", scenes.len()));
        };
        let scene_bytes = std::fs::read(dir.join(scene_file)).map_err(|e| format!("{id}/{scene_file}: {e}"))?;
        let scene = Scene::parse(&scene_bytes).map_err(|e| format!("{id}/{scene_file}: {e}"))?;
        // As the shell looks it up: the name as written, then with the platform tag (§2).
        let named = scene.terrain_file.clone();
        let tagged = match named.rsplit_once('.') {
            Some((stem, ext)) => format!("{stem}.ps2.{ext}"),
            None => format!("{named}.ps2"),
        };
        let terrain_file = [named.as_str(), tagged.as_str()]
            .iter()
            .find_map(|n| match lookup(&all, n).as_slice() {
                [one] => Some((*one).clone()),
                _ => None,
            })
            .ok_or_else(|| format!("{id}: the terrain {named} is not in the folder (or is there twice)"))?;
        let raw = std::fs::read(dir.join(&terrain_file)).map_err(|e| format!("{id}/{terrain_file}: {e}"))?;
        let mut t = Tile::from_parts(&id, meta, scene_file, scene_bytes, &terrain_file, &raw)?;
        t.dir = dir.to_path_buf();
        Ok(t)
    }

    /// A tile from its files' bytes, wherever they were read (P232: a disc map's archives, as the
    /// shell's `worlds::Origin::Disc` reads them, with no folder): `meta` as its mod.json would say,
    /// the scene's path and bytes, and the terrain's path and its bytes as the file has them. The
    /// tile's [`Tile::dir`] is empty; [`Tile::load`] sets it.
    pub fn from_parts(id: &str, meta: Json, scene_file: &str, scene_bytes: Vec<u8>, terrain_file: &str, raw: &[u8]) -> Result<Tile, String> {
        let scene = Scene::parse(&scene_bytes).map_err(|e| format!("{id}/{scene_file}: {e}"))?;
        let scene_text = String::from_utf8(scene_bytes).map_err(|_| format!("{id}/{scene_file}: not UTF-8"))?;
        let terrain = trnwrite::openmx_view(raw).map_err(|e| format!("{id}/{terrain_file}: {e}"))?.into_owned();
        let t = Terrain::parse(&terrain).map_err(|e| format!("{id}/{terrain_file}: {e}"))?;
        let root = t.root().ok_or_else(|| format!("{id}/{terrain_file}: no root node"))?;
        let (mut cx, mut cz) = (0, 0);
        for z in 0..16 {
            for x in 0..16 {
                if t.child(&root, x, z).is_some() {
                    cx = cx.max(x + 1);
                    cz = cz.max(z + 1);
                }
            }
        }
        if t.levels == 0 || cx == 0 {
            return Err(format!("{id}/{terrain_file}: an empty tree"));
        }
        let per = 16u32.pow(t.levels);
        let n = (cx * per, cz * per);
        let ground = whole_cells(&t, n);
        if ground[2] <= ground[0] || ground[3] <= ground[1] {
            return Err(format!("{id}/{terrain_file}: no leaf whose cells are all there"));
        }
        let (scale, spacing) = (t.scale, f64::from(t.scale.0));
        drop(t);
        Ok(Tile {
            id: id.to_owned(),
            dir: PathBuf::new(),
            meta,
            scene_file: scene_file.to_owned(),
            scene_text,
            scene,
            terrain_file: terrain_file.to_owned(),
            terrain,
            spacing,
            scale,
            n,
            ground,
        })
    }

    pub fn terrain(&self) -> Terrain<'_> {
        Terrain::parse(&self.terrain).expect("parsed when the tile was loaded")
    }

    /// The `source.game` of the tile's mod.json.
    pub fn game(&self) -> String {
        self.meta.get("source").and_then(|s| s.get("game")).and_then(Json::str).unwrap_or("").to_owned()
    }

    /// One corner of a cell as world feet: the terrain's local height times its scale, in the game's
    /// single floats (game-sim `world::Ground::at` at a corner).
    fn corner(&self, t: &Terrain, cx: u32, cz: u32, dx: u32, dz: u32) -> Option<f64> {
        let q = t.cell(cx, cz, 1)?;
        Some(f64::from((self.scale * q.height[(dx + 2 * dz) as usize]).0))
    }

    /// A sample's height from whichever cell holds it (the ground's samples, for the berm profile).
    fn sample(&self, t: &Terrain, x: u32, z: u32) -> f64 {
        let g = self.ground;
        let (cx, cz) = (x.clamp(g[0], g[2] - 1), z.clamp(g[1], g[3] - 1));
        self.corner(t, cx, cz, x - cx, z - cz).unwrap_or(0.0)
    }

    /// The ground at a point inside (or on the edge of) `keep`, in the tile's sample units: the cell is
    /// the one inside `keep` (so the far edges read the last kept cell, not the cut one), blended as
    /// game-sim's `Ground::at` blends it. Height in world feet, and the nearest corner's surface.
    pub fn ground_in(&self, t: &Terrain, keep: [u32; 4], fx: f64, fz: f64) -> Option<(f64, u8)> {
        let cx = (fx.floor().max(0.0) as u32).clamp(keep[0], keep[2] - 1);
        let cz = (fz.floor().max(0.0) as u32).clamp(keep[1], keep[3] - 1);
        let q = t.cell(cx, cz, 1)?;
        let (tx, tz) = (F((fx - f64::from(cx)) as f32), F((fz - f64::from(cz)) as f32));
        let w = [(F::ONE - tx) * (F::ONE - tz), tx * (F::ONE - tz), (F::ONE - tx) * tz, tx * tz];
        let mut y = F::ZERO;
        for i in 0..4 {
            y = y + w[i] * q.height[i];
        }
        let nearest = usize::from(tx > F(0.5)) + 2 * usize::from(tz > F(0.5));
        Some((f64::from((self.scale * y).0), q.surface[nearest]))
    }

    /// The interior left after the edge berm is cut, `[x0, z0, x1, z1]` in samples, half-open.
    ///
    /// Each side loses at least the shootoff band (150 ft, or the scene's `TerrainBorderWidth` if
    /// wider), then as long as the side's profile (the median height at each distance from the edge)
    /// keeps falling inward faster than [`BERM_FALL`]: that is the berm's inner slope. At most
    /// [`MAX_BERM`] feet past the shootoff band, and at most a quarter of the map.
    ///
    /// Past the berm, a side whose profile then climbs to a ridge ([`Tile::ridge`]) is cut at the
    /// ridge's peak instead (M23): the ground between is the map's own ring of mountains and the flat
    /// shootoff plateau outside them, which in a world would leave every join a plateau with a ridge
    /// either side. Cut at the peaks, two maps meet ridge to ridge.
    ///
    /// The sides are the [`Tile::ground`]'s: where cells are missing at an edge, the side is the hole's
    /// edge, and the shootoff band is only what is left of it past the hole (the game has no shootoff
    /// there, only no ground). The berm is looked for from the hole's edge all the same.
    pub fn find_keep(&self) -> [u32; 4] {
        self.find_cut().0
    }

    /// [`Tile::find_keep`]'s interior, and which sides (x0, z0, x1, z1) it cut at a ridge.
    ///
    /// A side whose cut would run through a rock face is then cut farther in, past it ([`Tile::rock_cut`]).
    pub fn find_cut(&self) -> ([u32; 4], [bool; 4]) {
        let g = self.ground;
        let (mut band, ridged) = self.berm_cut();
        let rock = self.past_rock(&self.terrain(), band);
        (0..4).for_each(|side| band[side] += rock[side]);
        ([g[0] + band[0], g[1] + band[1], g[2] - band[2], g[3] - band[3]], ridged)
    }

    /// How many samples farther in than its berm (or ridge) [`Tile::find_cut`] cuts each side, past a
    /// rock face: 0 on every side of most maps.
    pub fn rock_cut(&self) -> [u32; 4] {
        self.past_rock(&self.terrain(), self.berm_cut().0)
    }

    /// Each side's band in samples from the [`Tile::ground`]'s side, berm or ridge, and which it cut at a
    /// ridge.
    fn berm_cut(&self) -> ([u32; 4], [bool; 4]) {
        let t = self.terrain();
        let s = self.spacing;
        let g = self.ground;
        let border = f64::from(self.scene.terrain.border_width.0).max(0.0);
        let m0 = (SHOOTOFF.max(border) / s).ceil() as usize;
        let hole = [g[0], g[1], self.n.0 - g[2], self.n.1 - g[3]];
        let mut band = [0u32; 4];
        let mut ridged = [false; 4];
        for (side, b) in band.iter_mut().enumerate() {
            // side 0: x = d, 1: z = d, 2: x = n − d, 3: z = n − d (n and 0 the ground's sides)
            let across = if side % 2 == 0 { g[2] - g[0] } else { g[3] - g[1] };
            let dmax = (m0 + (MAX_BERM / s).ceil() as usize).min(across as usize / 4);
            let reach = (dmax + (RIDGE_REACH / s).ceil() as usize).min(across as usize / 4);
            let profile = self.profile(&t, side, reach.max(dmax + BERM_WINDOW), m0 as u32);
            let mut d = m0.saturating_sub(hole[side] as usize).min(dmax);
            while d < dmax && profile[d] - profile[d + BERM_WINDOW] > BERM_FALL * BERM_WINDOW as f64 * s {
                d += 1;
            }
            let ridge = Tile::ridge(&profile[..=reach.max(d)], d, s);
            ridged[side] = ridge.is_some();
            *b = ridge.unwrap_or(d) as u32;
        }
        (band, ridged)
    }

    /// The samples each side of `band` is cut farther in so that no seam runs through a rock face
    /// (openmx MD53): while some side's cut line has ground steeper than [`ROCK`] across it (its last
    /// cell, anywhere along the side's kept span), the side whose nearest line inward that is not steep
    /// costs the least ground is moved to that line, until none is steep or no steep side can move
    /// (within a quarter of the map, as the berm is). MX Unleashed's nat17 stands a mountain about 900 ft
    /// square in its x1/z1 corner, 10 to 30 ft a foot through every line across it, and its seam there
    /// was a 4 ft step between rays a quarter of a foot apart.
    fn past_rock(&self, t: &Terrain, band: [u32; 4]) -> [u32; 4] {
        let g = self.ground;
        let mut b = band;
        let side_len = |side: usize| if side % 2 == 0 { g[2] - g[0] } else { g[3] - g[1] };
        // The steepest slope across side `side`'s line `d` samples in, over the other sides' kept span.
        let steep = |b: &[u32; 4], side: usize, d: u32| -> f64 {
            let (lo, hi) = if side % 2 == 0 { (g[1] + b[1], g[3] - b[3]) } else { (g[0] + b[0], g[2] - b[2]) };
            let mut most = 0.0f64;
            for a in lo..=hi {
                let (p, q) = match side {
                    0 => (self.sample(t, g[0] + d, a), self.sample(t, g[0] + d + 1, a)),
                    1 => (self.sample(t, a, g[1] + d), self.sample(t, a, g[1] + d + 1)),
                    2 => (self.sample(t, g[2] - d, a), self.sample(t, g[2] - d - 1, a)),
                    _ => (self.sample(t, a, g[3] - d), self.sample(t, a, g[3] - d - 1)),
                };
                most = most.max((p - q).abs() / self.spacing);
            }
            most
        };
        loop {
            let mut best: Option<(u64, usize, u32)> = None;
            for side in 0..4 {
                if steep(&b, side, b[side]) <= ROCK {
                    continue;
                }
                let span = u64::from(side_len(1 - side % 2) - b[(side + 1) % 4] - b[(side + 3) % 4]);
                let cap = side_len(side % 2) / 4;
                if let Some(d) = (b[side] + 1..=cap).find(|&d| steep(&b, side, d) <= ROCK) {
                    let cost = u64::from(d - b[side]) * span;
                    if best.is_none_or(|(c, _, _)| cost < c) {
                        best = Some((cost, side, d));
                    }
                }
            }
            let Some((_, side, d)) = best else { break };
            b[side] = d;
        }
        (0..4).map(|side| b[side] - band[side]).collect::<Vec<_>>().try_into().expect("four sides")
    }

    /// The peak of a ridge in a side's `profile` (samples `spacing` feet apart) past the berm at `berm`,
    /// if there is one: behind a [`RIDGE_PLATEAU`] of flat ground, the highest point within the profile
    /// (the caller's [`RIDGE_REACH`]) that stands [`RIDGE_RISE`] over the berm, and that the profile falls
    /// [`RIDGE_RISE`] below again inward of it (so ground that keeps rising inward is not cut into). The
    /// first of equal heights.
    pub fn ridge(profile: &[f64], berm: usize, spacing: f64) -> Option<usize> {
        let base = *profile.get(berm)?;
        let flat = profile[berm..].iter().take_while(|h| (*h - base).abs() <= RIDGE_FLAT).count();
        if (flat as f64) * spacing < RIDGE_PLATEAU {
            return None;
        }
        let (peak, top) = profile.iter().enumerate().skip(berm).fold((berm, base), |a, (d, &h)| if h > a.1 { (d, h) } else { a });
        if top - base <= RIDGE_RISE {
            return None;
        }
        profile[peak + 1..].iter().any(|&h| h < top - RIDGE_RISE).then_some(peak)
    }

    /// One side's edge profile: for each distance `d` in samples from the side of the [`Tile::ground`]
    /// (0: x = x0 + d, 1: z = z0 + d, 2: x = x1 − d, 3: z = z1 − d), the median height along it, leaving
    /// `skip` samples at either end.
    pub fn profile(&self, t: &Terrain, side: usize, last: usize, skip: u32) -> Vec<f64> {
        let [x0, z0, x1, z1] = self.ground;
        let (len, across) = if side % 2 == 0 { (z1 - z0, x1 - x0) } else { (x1 - x0, z1 - z0) };
        let lo = skip.min(len / 2);
        let hi = len.saturating_sub(skip).max(lo + 1);
        (0..=last)
            .map(|d| {
                let d = (d as u32).min(across);
                let mut v: Vec<f64> = (lo..hi)
                    .map(|a| match side {
                        0 => self.sample(t, x0 + d, z0 + a),
                        1 => self.sample(t, x0 + a, z0 + d),
                        2 => self.sample(t, x1 - d, z0 + a),
                        _ => self.sample(t, x0 + a, z1 - d),
                    })
                    .collect();
                v.sort_by(f64::total_cmp);
                v[v.len() / 2]
            })
            .collect()
    }

    /// Every placement the scene's readers take, with its section name and position, in file order.
    /// The rule is `track::Scene::parse`'s: a numbered `[Model<n>]` or `[Object<n>]` with a file, less
    /// what a free ride leaves out (openmx MD13): an activity model that hides, and each collision object
    /// whose name holds an activity model's with `HasCOL` on, unless a start zone's puts it back.
    pub fn placements(&self) -> Result<Vec<(String, [f64; 3])>, String> {
        // (name, kind, file, Type, Hide, HasCOL) of each numbered section.
        type Section = (String, usize, String, Option<String>, Option<String>, Option<String>);
        let mut sections: Vec<Section> = Vec::new();
        for raw in self.scene_text.lines() {
            let line = raw.trim();
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                let low = name.to_ascii_lowercase();
                let numbered = |p: &str| {
                    low.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.bytes().all(|b| b.is_ascii_digit()))
                };
                let kind = if numbered("model") {
                    0
                } else if numbered("object") {
                    1
                } else {
                    2
                };
                sections.push((name.to_owned(), kind, String::new(), None, None, None));
                continue;
            }
            if let (Some(c), Some((k, v))) = (sections.last_mut(), line.split_once('=')) {
                let v = v.trim().to_owned();
                match k.trim().to_ascii_lowercase().as_str() {
                    "col" | "slt" => c.2 = v,
                    "type" => c.3 = Some(v),
                    "hide" => c.4 = Some(v),
                    "hascol" => c.5 = Some(v),
                    _ => {}
                }
            }
        }
        sections.retain(|c| c.1 < 2 && !c.2.is_empty());
        let squeezed = |n: &str| n.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
        let (mut off, mut on) = (Vec::new(), Vec::new());
        for (_, kind, file, ty, _, has_col) in &sections {
            let Some(class) = (*kind == 0).then(|| ActivityModel::of(ty.as_deref())).flatten() else { continue };
            if has_col.as_deref().is_none_or(switch) && !squeezed(file).is_empty() {
                if class == ActivityModel::StartZone { &mut on } else { &mut off }.push(squeezed(file));
            }
        }
        let named = |names: &[String], file: &str| names.iter().any(|n| squeezed(file).contains(n.as_str()));
        let mut names: [Vec<String>; 2] = [Vec::new(), Vec::new()];
        for (name, kind, file, ty, hide, _) in sections {
            let kept = if kind == 0 {
                !ActivityModel::free_ride_hides(ty.as_deref(), hide.as_deref())
            } else {
                !named(&off, &file) || named(&on, &file)
            };
            if kept {
                names[kind].push(name);
            }
        }
        let lists = [&self.scene.models, &self.scene.collision];
        let mut out = Vec::new();
        for kind in 0..2 {
            if names[kind].len() != lists[kind].len() {
                return Err(format!(
                    "{}/{}: {} sections against Scene::parse's {} placements",
                    self.id,
                    self.scene_file,
                    names[kind].len(),
                    lists[kind].len()
                ));
            }
            for (n, p) in names[kind].iter().zip(lists[kind]) {
                out.push((n.clone(), p.position.map(|c| f64::from(c.0))));
            }
        }
        Ok(out)
    }

    /// Where a rider starts on this tile alone: mod.json's `spawn` (§2b) or the scene's.
    pub fn spawn(&self) -> [f64; 3] {
        let from_json = self.meta.get("spawn").and_then(|s| s.get("position")).and_then(Json::arr).and_then(|a| {
            let v: Vec<f64> = a.iter().filter_map(Json::f64).collect();
            (v.len() == 3).then(|| [v[0], v[1], v[2]])
        });
        from_json.unwrap_or(self.scene.start.map(|c| f64::from(c.0)))
    }

    /// The ground along the four edges of `keep`, one entry per sample (see [`Ring`]).
    pub fn ring(&self, keep: [u32; 4]) -> Ring {
        let t = self.terrain();
        let s = self.spacing;
        let [x0, z0, x1, z1] = keep;
        let at = |x: u32, z: u32| -> Edge {
            let (cx, cz) = (x.clamp(x0, x1 - 1), z.clamp(z0, z1 - 1));
            let (dx, dz) = (x - cx, z - cz);
            let c = |a, b| self.corner(&t, cx, cz, a, b).unwrap_or(0.0);
            let surf = t.cell(cx, cz, 1).map_or(0, |q| q.surface[(dx + 2 * dz) as usize]);
            Edge { h: c(dx, dz), gx: (c(1, dz) - c(0, dz)) / s, gz: (c(dx, 1) - c(dx, 0)) / s, surface: surf }
        };
        let edges = [
            (z0..=z1).map(|z| at(x0, z)).collect::<Vec<Edge>>(),
            (x0..=x1).map(|x| at(x, z0)).collect(),
            (z0..=z1).map(|z| at(x1, z)).collect(),
            (x0..=x1).map(|x| at(x, z1)).collect(),
        ];
        Ring::new(edges)
    }
}

/// The ground at one sample on an interior's edge: height in feet, the slope of the kept cell it
/// belongs to in x and z (feet per foot), and the surface byte (id) there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub h: f64,
    pub gx: f64,
    pub gz: f64,
    pub surface: u8,
}

/// The four edges of an interior: 0 x = x0 and 2 x = x1 (along z), 1 z = z0 and 3 z = z1 (along x).
#[derive(Clone, Debug)]
pub struct Ring {
    pub edges: [Vec<Edge>; 4],
    /// Per edge, the integral of (height, x slope, z slope) from its start to each sample.
    prefix: [Vec<[f64; 3]>; 4],
}

impl Ring {
    /// A ring from its four edges' samples (a tile's, [`Tile::ring`], or a whole world's outer edge, C5).
    pub fn new(edges: [Vec<Edge>; 4]) -> Ring {
        let prefix = edges.clone().map(|e| {
            let mut acc = [0.0; 3];
            let mut out = vec![acc];
            for w in e.windows(2) {
                let (a, b) = ([w[0].h, w[0].gx, w[0].gz], [w[1].h, w[1].gx, w[1].gz]);
                acc = [0, 1, 2].map(|k| acc[k] + (a[k] + b[k]) / 2.0);
                out.push(acc);
            }
            out
        });
        Ring { edges, prefix }
    }

    /// An edge at a fractional sample along it, the height and slopes blended linearly (which is what the
    /// cell's own blend gives on its edge), the surface the nearest sample's.
    fn along(&self, edge: usize, a: f64) -> Edge {
        let e = &self.edges[edge];
        let a = a.clamp(0.0, (e.len() - 1) as f64);
        let i = (a.floor() as usize).min(e.len() - 2);
        let t = a - i as f64;
        let (p, q) = (e[i], e[i + 1]);
        Edge {
            h: p.h + (q.h - p.h) * t,
            gx: p.gx + (q.gx - p.gx) * t,
            gz: p.gz + (q.gz - p.gz) * t,
            surface: if t > 0.5 { q.surface } else { p.surface },
        }
    }

    /// The integral of (height, x slope, z slope) along an edge from its start to `a` samples, the edge
    /// held at its end values past either end. Exact for the linear pieces.
    fn integral(&self, edge: usize, a: f64) -> [f64; 3] {
        let e = &self.edges[edge];
        let p = &self.prefix[edge];
        let v = |x: &Edge| [x.h, x.gx, x.gz];
        let last = (e.len() - 1) as f64;
        if a <= 0.0 {
            return v(&e[0]).map(|c| c * a);
        }
        if a >= last {
            let end = v(&e[e.len() - 1]);
            return [0, 1, 2].map(|i| p[e.len() - 1][i] + end[i] * (a - last));
        }
        let i = a.floor() as usize;
        let at = self.along(edge, a);
        let (lo, mid) = (v(&e[i]), [at.h, at.gx, at.gz]);
        [0, 1, 2].map(|k| p[i][k] + (lo[k] + mid[k]) / 2.0 * (a - i as f64))
    }

    /// (height, x slope, z slope) averaged over `a ± sigma` samples along an edge.
    pub fn blurred(&self, edge: usize, a: f64, sigma: f64) -> [f64; 3] {
        if sigma < 1e-9 {
            let e = self.along(edge, a);
            return [e.h, e.gx, e.gz];
        }
        let (hi, lo) = (self.integral(edge, a + sigma), self.integral(edge, a - sigma));
        [0, 1, 2].map(|k| (hi[k] - lo[k]) / (2.0 * sigma))
    }

    pub fn mean(&self) -> f64 {
        let all: Vec<f64> = self.edges.iter().flatten().map(|e| e.h).collect();
        all.iter().sum::<f64>() / all.len() as f64
    }

    pub fn edge_mean(&self, edge: usize) -> f64 {
        self.edges[edge].iter().map(|e| e.h).sum::<f64>() / self.edges[edge].len() as f64
    }
}

/// A placed interior as the blend sees it: its rectangle in world feet and its edge ground.
#[derive(Clone, Debug)]
pub struct Source {
    pub tile: usize,
    /// `[x0, z0, x1, z1]` in world feet, closed.
    pub rect: [f64; 4],
    pub at: [f64; 2],
    pub spacing: f64,
    pub keep: [u32; 4],
    pub ring: Ring,
    /// Per edge (0 x = x0, 1 z = z0, 2 x = x1, 3 z = z1), how far out the join ground counts as near
    /// the seam, feet: past this the rideable gate holds it to `max_slope` (C9, [`Source::edge_gates`]).
    pub gates: [f64; 4],
}

/// What one tile's extension says at a point outside its interior.
#[derive(Clone, Copy, Debug)]
pub struct Reach {
    /// Distance to the interior, feet.
    pub d: f64,
    /// The extended height.
    pub h: f64,
    pub surface: u8,
}

impl Source {
    /// Distance from a world point to the closed interior.
    pub fn distance(&self, x: f64, z: f64) -> f64 {
        let dx = (self.rect[0] - x).max(x - self.rect[2]).max(0.0);
        let dz = (self.rect[1] - z).max(z - self.rect[3]).max(0.0);
        dx.hypot(dz)
    }

    pub fn contains(&self, x: f64, z: f64) -> bool {
        const E: f64 = 1e-6;
        x >= self.rect[0] - E && x <= self.rect[2] + E && z >= self.rect[1] - E && z <= self.rect[3] + E
    }

    /// The distance [`Source::extend`] finds, by the same arithmetic to the bit (for the pruning, which
    /// must decide on exactly the distance the weight is taken from).
    pub fn extend_distance(&self, x: f64, z: f64) -> f64 {
        let s = self.spacing;
        let [x0, z0, x1, z1] = self.keep.map(f64::from);
        let fx = (x - self.at[0]) / s;
        let fz = (z - self.at[1]) / s;
        let (qx, qz) = (fx.clamp(x0, x1), fz.clamp(z0, z1));
        ((fx - qx) * s).hypot((fz - qz) * s)
    }

    /// The edges a point outside the interior is past: (x side 0 or 2, z side 1 or 3), each if any.
    fn sides(&self, x: f64, z: f64) -> (Option<usize>, Option<usize>) {
        let [x0, z0, x1, z1] = self.rect;
        let v = if x < x0 { Some(0) } else if x > x1 { Some(2) } else { None };
        let h = if z < z0 { Some(1) } else if z > z1 { Some(3) } else { None };
        (v, h)
    }

    /// The seam gate at a point outside the interior: the gate of the edge it is past, the larger of the
    /// two past a corner.
    pub fn gate_at(&self, x: f64, z: f64) -> f64 {
        match self.sides(x, z) {
            (Some(v), Some(h)) => self.gates[v].max(self.gates[h]),
            (Some(e), None) | (None, Some(e)) => self.gates[e],
            (None, None) => self.gates.iter().copied().fold(0.0, f64::max),
        }
    }

    /// How far past each edge this tile's own extension stays steeper than `limit` (C9): the distance
    /// from the interior past which every lattice cell of the extension alone is at most `limit`, and at
    /// least `base`. A steep edge's relief (a hill the cut runs through) can only be carried down at a
    /// slope the ground can take over a distance the relief sets, however wide the gap: the blur along
    /// the edge brings its along-edge slope to about relief / σ, and σ grows no faster than the distance.
    ///
    /// Measured on lattice cells (`step` a side) straight out from every lattice point of each edge, and
    /// on the corner squares (counted for both their edges), out to `base` plus twice the edge's
    /// relief (its heights' range plus the steepest slope across it carried over `step + fade`) over
    /// `limit`.
    pub fn edge_gates(&self, step: f64, fade: f64, limit: f64, base: f64) -> [f64; 4] {
        let [x0, z0, x1, z1] = self.rect;
        let h = |x: f64, z: f64| self.extend(x, z, step, fade).h;
        let steep = |x: f64, z: f64| {
            let c = [h(x, z), h(x + step, z), h(x, z + step), h(x + step, z + step)];
            let gx = (c[1] - c[0] + c[3] - c[2]) / (2.0 * step);
            let gz = (c[2] - c[0] + c[3] - c[1]) / (2.0 * step);
            gx.hypot(gz) > limit
        };
        let reach: [f64; 4] = std::array::from_fn(|e| {
            let edge = &self.ring.edges[e];
            let (lo, hi) = edge.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.h), b.max(p.h)));
            let across = edge.iter().map(|p| p.gx.abs().max(p.gz.abs())).fold(0.0, f64::max);
            let relief = hi - lo + across * (step + fade);
            (base + 2.0 * relief / limit).min(base.max(30_000.0))
        });
        let n = |len: f64| (len / step).ceil() as usize;
        // Eight jobs on threads: straight out from each edge (the cell whose nearest corner is `k`
        // steps out), and each corner square out to the larger of its two edges' reaches. Each gives
        // the farthest steep cell's distance plus a step, per edge.
        let job = |j: usize| -> [f64; 4] {
            let mut g = [0.0f64; 4];
            if j < 4 {
                let e = j;
                let along = if e % 2 == 0 { (z0, z1) } else { (x0, x1) };
                for k in 1..=n(reach[e]) {
                    let d = k as f64 * step;
                    let steep_row = (0..n(along.1 - along.0)).any(|i| {
                        let a = along.0 + i as f64 * step;
                        let (x, z) = match e {
                            0 => (x0 - d - step, a),
                            1 => (a, z0 - d - step),
                            2 => (x1 + d, a),
                            _ => (a, z1 + d),
                        };
                        steep(x, z)
                    });
                    if steep_row {
                        g[e] = d + step;
                    }
                }
            } else {
                let (v, hz) = [(0, 1), (2, 1), (0, 3), (2, 3)][j - 4];
                let r = reach[v].max(reach[hz]);
                for kz in 1..=n(r) {
                    for kx in 1..=n(r) {
                        let (ux, uz) = (kx as f64 * step, kz as f64 * step);
                        if ux.hypot(uz) > r {
                            break;
                        }
                        let x = if v == 0 { x0 - ux - step } else { x1 + ux };
                        let z = if hz == 1 { z0 - uz - step } else { z1 + uz };
                        if steep(x, z) {
                            let d = ux.hypot(uz) + step;
                            g[v] = g[v].max(d);
                            g[hz] = g[hz].max(d);
                        }
                    }
                }
            }
            g
        };
        let found: Vec<[f64; 4]> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..8).map(|j| sc.spawn(move || job(j))).collect();
            hs.into_iter().map(|h| h.join().expect("a gate job")).collect()
        });
        let mut gates = [base; 4];
        for g in found {
            for e in 0..4 {
                gates[e] = gates[e].max(g[e]);
            }
        }
        gates
    }

    /// The tile's ground carried past its edge: the edge's height plus its slope along the way out,
    /// times `taper(d)` (which is `d` for the first `step` and then levels off over `fade` feet).
    pub fn extend(&self, x: f64, z: f64, step: f64, fade: f64) -> Reach {
        let s = self.spacing;
        let [x0, z0, x1, z1] = self.keep.map(f64::from);
        let fx = (x - self.at[0]) / s;
        let fz = (z - self.at[1]) / s;
        let (qx, qz) = (fx.clamp(x0, x1), fz.clamp(z0, z1));
        let (ux, uz) = ((fx - qx) * s, (fz - qz) * s);
        let d = ux.hypot(uz);
        // The edge, blurred along its length the more the farther out: none on the pinned ring, then
        // σ = d′² / (d′ + fade) feet, so a hill cut by the edge does not run on as a ridge forever.
        let dp = (d - step).max(0.0);
        let sigma = dp * dp / (dp + fade) / s;
        // The slope is blurred faster, over d′ either way (openmx MD53): it is one cell's, ragged from
        // sample to sample, and times the taper it stood ribs across the join several feet a foot steep
        // along an edge whose own ground was gentle (3.7 beside nat17's 1.7 and 0.1, 27 ft out).
        let sigma_g = dp / s;
        let blurred = |edge: usize, a: f64| {
            let (h, g) = (self.ring.blurred(edge, a, sigma), self.ring.blurred(edge, a, sigma_g));
            [h[0], g[1], g[2]]
        };
        let (vert, horiz) = (if fx < x0 { Some(0) } else if fx > x1 { Some(2) } else { None }, if fz < z0 { Some(1) } else if fz > z1 { Some(3) } else { None });
        let (e, surface) = match (vert, horiz) {
            (Some(v), Some(h)) => {
                // A corner: the two edges' ends, by the angle out of it (each alone on its side's line).
                let a = blurred(v, qz - z0);
                let b = blurred(h, qx - x0);
                let (wa, wb) = (ux * ux / (d * d), uz * uz / (d * d));
                ([0, 1, 2].map(|i| wa * a[i] + wb * b[i]), self.ring.along(v, qz - z0).surface)
            }
            (Some(v), None) => (blurred(v, qz - z0), self.ring.along(v, qz - z0).surface),
            (None, Some(h)) => (blurred(h, qx - x0), self.ring.along(h, qx - x0).surface),
            (None, None) => {
                let e = self.ring.along(0, qz - z0);
                ([e.h, e.gx, e.gz], e.surface)
            }
        };
        if d == 0.0 {
            return Reach { d, h: e[0], surface };
        }
        let slope = (e[1] * ux + e[2] * uz) / d;
        let taper = if d <= step { d } else { step + fade * (1.0 - (-(d - step) / fade).exp()) };
        Reach { d, h: e[0] + slope * taper, surface }
    }
}

/// Modified Shepard weight for a distance past the pinned ring (`d′ > 0`) within the radius.
pub fn shepard(d: f64, radius: f64) -> f64 {
    if d >= radius {
        0.0
    } else {
        let w = (radius - d) / (radius * d);
        w * w
    }
}
