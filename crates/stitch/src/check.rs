//! `stitch check`: a world mod read back with openmx's readers, the parts of §5 step 3 that are C1's.
//!
//! - (openmx-mods' modcheck, §1, is its CLI's to run beside this: every tile as a track, every join
//!   terrain read, `.blend` sums, `drop`, `sky`, `spawn`.)
//! - Each tile: `at` on a whole leaf, `keep` inside its terrain, no two interiors overlapping, and (given
//!   the mods the world was built from) every file of the tile byte for byte its source mod's.
//! - Every seam point, read from the tile's kept cell and the join's cell outside it with
//!   `Terrain::cell`: the same height and the same slope across the seam, to the join's height quantum.
//!   A seam point is a sample of the join that covers the edge, which is every tile sample on it when
//!   that join is at the tile's spacing (always, or the tile's collar, C6). Tile samples between the
//!   join's samples (a tile with no collar whose spacing differs) are measured and reported, not failed.
//! - Where a join at one spacing meets a join at another (a collar's edge), the two are the same height
//!   at every sample of either along the line.
//! - Neighbouring join pieces agree on their shared samples.
//! - Join ground away from the seams is no steeper than the world's `max_slope`.
//! - The look: on every seam point the touching tile's weight is 255, and no weight jumps by more than
//!   [`MAX_BLEND_STEP`] between neighbouring samples.
//! - An ultra map (C5, `stitch.worlds`): on every lattice sample of each world's edge where new ground
//!   meets it, the same height and slope across on both sides ([`world_seams`]); the new joins held to
//!   `max_slope` past each world's gates too; and (given `--mods`) every copied join byte for byte its
//!   world's.

use crate::Tile;
use game_sim::track::Terrain;
use crate::json::Json;
use ps2_float::F;
use std::collections::HashMap;
use std::path::Path;

/// A blend weight may change by at most this between two neighbouring samples ("never a hard edge").
pub const MAX_BLEND_STEP: u8 = 64;

#[derive(Debug, Default)]
pub struct Report {
    pub fail: Vec<String>,
    pub note: Vec<String>,
    /// Seam points checked strictly, and the worst height and slope steps there.
    pub seam_points: usize,
    pub worst_height: f64,
    pub worst_slope: f64,
    /// Tile samples between lattice points: count and worst height step.
    pub between_points: usize,
    pub between_height: f64,
    /// Points where joins of two spacings meet, checked strictly: count, worst height step, and (not
    /// failed: each side's cell is its own spacing's) worst slope step across.
    pub join_seam_points: usize,
    pub join_seam_height: f64,
    pub join_seam_slope: f64,
    pub far_slope: f64,
    /// Ultra maps (C5): lattice points where a world's edge meets the new ground, and the worst height and
    /// slope steps there (failed past the quantum, as a tile's seam).
    pub world_seam_points: usize,
    pub world_seam_height: f64,
    pub world_seam_slope: f64,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.fail.is_empty()
    }
}

struct Join {
    name: String,
    at: [f64; 2],
    samples: [u32; 2],
    between: Vec<String>,
    bytes: Vec<u8>,
    blend: Vec<u8>,
    spacing: f64,
    scale: F,
    quantum: f64,
}

impl Join {
    fn terrain(&self) -> Terrain<'_> {
        Terrain::parse(&self.bytes).expect("parsed when loaded")
    }

    fn rect(&self) -> [f64; 4] {
        [
            self.at[0],
            self.at[1],
            self.at[0] + f64::from(self.samples[0] - 1) * self.spacing,
            self.at[1] + f64::from(self.samples[1] - 1) * self.spacing,
        ]
    }

    fn covers(&self, x: f64, z: f64) -> bool {
        let r = self.rect();
        const E: f64 = 1e-6;
        x >= r[0] - E && x <= r[2] + E && z >= r[1] - E && z <= r[3] + E
    }

    /// The ground of one cell at a point (local sample units), blended as game-sim's `Ground::at`.
    fn in_cell(&self, t: &Terrain, cx: u32, cz: u32, fx: f64, fz: f64) -> Option<f64> {
        let q = t.cell(cx, cz, 1)?;
        let (tx, tz) = (F((fx - f64::from(cx)) as f32), F((fz - f64::from(cz)) as f32));
        let w = [(F::ONE - tx) * (F::ONE - tz), tx * (F::ONE - tz), (F::ONE - tx) * tz, tx * tz];
        let mut y = F::ZERO;
        for i in 0..4 {
            y = y + w[i] * q.height[i];
        }
        Some(f64::from((self.scale * y).0))
    }

    /// The ground at a world point, from the cell that holds `probe` (a point nudged to one side).
    fn height_by(&self, t: &Terrain, x: f64, z: f64, probe: [f64; 2]) -> Option<f64> {
        let f = [(x - self.at[0]) / self.spacing, (z - self.at[1]) / self.spacing];
        let c = [0, 1].map(|ax| (((probe[ax] - self.at[ax]) / self.spacing).floor().max(0.0) as u32).min(self.samples[ax] - 2));
        self.in_cell(t, c[0], c[1], f[0], f[1])
    }

    /// Whether a world coordinate along `axis` is one of the join's sample lines.
    fn on_lattice(&self, axis: usize, v: f64) -> bool {
        let f = (v - self.at[axis]) / self.spacing;
        (f - f.round()).abs() < 1e-6
    }

    fn sample(&self, t: &Terrain, x: u32, z: u32) -> Option<f64> {
        let (cx, cz) = (x.min(self.samples[0] - 2), z.min(self.samples[1] - 2));
        self.in_cell(t, cx, cz, f64::from(x), f64::from(z))
    }
}

fn nums(j: Option<&Json>) -> Vec<f64> {
    j.and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::f64).collect()
}

/// Check the world in `dir`. `mods`, if given, is where its tiles came from (a folder holding
/// `<tile>/` or `tracks/<tile>/`), for the byte comparison. The mod format's own checks (openmx-mods'
/// modcheck, §1) are not here: its CLI runs them beside this (P232).
pub fn check(dir: &Path, mods: Option<&Path>) -> Report {
    let mut r = Report::default();

    let meta = match std::fs::read_to_string(dir.join("mod.json")).map_err(|e| e.to_string()).and_then(|t| Json::parse(&t)) {
        Ok(j) => j,
        Err(e) => {
            r.fail.push(format!("mod.json: {e}"));
            return r;
        }
    };
    let st = meta.get("stitch");
    let lattice = st.and_then(|s| s.get("spacing")).and_then(Json::f64);
    let max_slope = st.and_then(|s| s.get("max_slope")).and_then(Json::f64).unwrap_or(0.5);
    let fade = st.and_then(|s| s.get("fade")).and_then(Json::f64).unwrap_or(48.0);

    // The tiles.
    let mut tiles: Vec<(Tile, [f64; 2], [u32; 4])> = Vec::new();
    for t in meta.get("tiles").and_then(Json::arr).unwrap_or_default() {
        let name = t.get("tile").and_then(Json::str).unwrap_or_default();
        let at = nums(t.get("at"));
        let keep = nums(t.get("keep"));
        if at.len() != 2 || keep.len() != 4 {
            r.fail.push(format!("tiles[{name}]: at or keep malformed"));
            continue;
        }
        let tile = match Tile::load(&dir.join("tiles").join(name)) {
            Ok(t) => t,
            Err(e) => {
                r.fail.push(format!("tiles/{name}: {e}"));
                continue;
            }
        };
        let keep = [keep[0] as u32, keep[1] as u32, keep[2] as u32, keep[3] as u32];
        // A whole foot (openmx's own rule) on a whole leaf: to the bit where the spacing makes the leaf
        // whole, and within a thousandth of a foot where it is a float only near a whole (4.499999).
        let leaf = 16.0 * tile.spacing;
        for a in at.iter() {
            if a.fract() != 0.0 {
                r.fail.push(format!("tiles[{name}]: at {a} is not a whole number of feet"));
            } else if (a / leaf - (a / leaf).round()).abs() * leaf > 1e-3 {
                r.fail.push(format!("tiles[{name}]: at {a} is not a whole leaf ({leaf} ft)"));
            }
        }
        if keep[0] >= keep[2] || keep[1] >= keep[3] || keep[2] > tile.n.0 || keep[3] > tile.n.1 {
            r.fail.push(format!("tiles[{name}]: keep {keep:?} is not inside its {}×{} samples", tile.n.0, tile.n.1));
            continue;
        }
        let g = tile.ground;
        if keep[0] < g[0] || keep[1] < g[1] || keep[2] > g[2] || keep[3] > g[3] {
            r.fail.push(format!("tiles[{name}]: keep {keep:?} runs over missing cells (its cells are all there only in {g:?})"));
            continue;
        }
        if let Some(m) = mods {
            // An ultra map's tiles (C5) also come from its worlds' own copies.
            let in_worlds = st.and_then(|s| s.get("worlds")).and_then(Json::arr).unwrap_or_default().iter().filter_map(|w| w.get("world")?.str()).flat_map(|w| {
                [m.join(w).join("tiles").join(name), m.join("tracks").join(w).join("tiles").join(name)]
            });
            let src = [m.join(name), m.join("tracks").join(name)].into_iter().chain(in_worlds).find(|p| p.join("mod.json").exists());
            match src {
                None => r.note.push(format!("tiles/{name}: no source mod under {} to compare with", m.display())),
                Some(src) => compare(&src, &tile.dir, name, &mut r),
            }
        }
        tiles.push((tile, [at[0], at[1]], keep));
    }
    if mods.is_none() {
        r.note.push("no --mods: the tiles were not compared with the mods they came from".into());
    }
    let rects: Vec<[f64; 4]> = tiles
        .iter()
        .map(|(t, at, k)| {
            let s = t.spacing;
            [at[0] + f64::from(k[0]) * s, at[1] + f64::from(k[1]) * s, at[0] + f64::from(k[2]) * s, at[1] + f64::from(k[3]) * s]
        })
        .collect();
    for i in 0..rects.len() {
        for j in i + 1..rects.len() {
            let (a, b) = (rects[i], rects[j]);
            if a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3] {
                r.fail.push(format!("tiles {} and {}: their interiors overlap", tiles[i].0.id, tiles[j].0.id));
            }
        }
    }

    // The joins.
    let mut joins: Vec<Join> = Vec::new();
    for jn in meta.get("joins").and_then(Json::arr).unwrap_or_default() {
        let name = jn.get("join").and_then(Json::str).unwrap_or_default().to_owned();
        let at = nums(jn.get("at"));
        let samples = nums(jn.get("samples"));
        let between: Vec<String> = jn.get("between").and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::str).map(str::to_owned).collect();
        let jd = dir.join("joins").join(&name);
        let (Ok(bytes), Ok(blend)) = (std::fs::read(jd.join(format!("{name}.ps2.trn"))), std::fs::read(jd.join(format!("{name}.blend")))) else {
            r.fail.push(format!("joins/{name}: files missing"));
            continue;
        };
        if at.len() != 2 || samples.len() != 2 {
            r.fail.push(format!("joins[{name}]: at or samples malformed"));
            continue;
        }
        if at.iter().any(|a| a.fract() != 0.0) {
            r.fail.push(format!("joins[{name}]: at {at:?} is not a whole number of feet"));
        }
        let t = match Terrain::parse(&bytes) {
            Ok(t) => t,
            Err(e) => {
                r.fail.push(format!("joins/{name}: Terrain::parse: {e}"));
                continue;
            }
        };
        let hs = t.root().map_or(F::ZERO, |n| n.height_scale);
        let (spacing, scale) = (f64::from(t.scale.0), t.scale);
        drop(t);
        let samples = [samples[0] as u32, samples[1] as u32];
        if blend.len() != (samples[0] * samples[1]) as usize * between.len() {
            r.fail.push(format!("joins/{name}.blend: {} bytes for {}×{} samples of {} tiles", blend.len(), samples[0], samples[1], between.len()));
            continue;
        }
        joins.push(Join { name, at: [at[0], at[1]], samples, between, bytes, blend, spacing, scale, quantum: f64::from((scale * hs).0) });
    }
    if joins.is_empty() {
        r.fail.push("no joins".into());
        return r;
    }
    let worlds: Vec<UltraWorld> = st
        .and_then(|s| s.get("worlds"))
        .and_then(Json::arr)
        .unwrap_or_default()
        .iter()
        .filter_map(|w| {
            let (rect, gates) = (nums(w.get("rect")), nums(w.get("gates")));
            (rect.len() == 4 && gates.len() == 4).then(|| UltraWorld {
                id: w.get("world").and_then(Json::str).unwrap_or_default().to_owned(),
                prefix: w.get("prefix").and_then(Json::str).unwrap_or_default().to_owned(),
                rect: [rect[0], rect[1], rect[2], rect[3]],
                gates: [gates[0], gates[1], gates[2], gates[3]],
            })
        })
        .collect();
    if !worlds.is_empty() {
        if let Some(m) = mods {
            for w in &worlds {
                let src = [m.join(&w.id), m.join("tracks").join(&w.id)].into_iter().find(|p| p.join("mod.json").exists());
                let Some(src) = src else {
                    r.note.push(format!("{}: no source world under {} to compare its joins with", w.id, m.display()));
                    continue;
                };
                let (mut same, mut n) = (0, 0);
                for j in joins.iter().filter(|j| j.name.starts_with(&w.prefix)) {
                    let old = &j.name[w.prefix.len()..];
                    n += 1;
                    let ok = ["ps2.trn", "blend"].iter().all(|ext| {
                        let a = std::fs::read(src.join("joins").join(old).join(format!("{old}.{ext}"))).ok();
                        a.is_some() && a == std::fs::read(dir.join("joins").join(&j.name).join(format!("{}.{ext}", j.name))).ok()
                    });
                    if ok {
                        same += 1;
                    } else {
                        r.fail.push(format!("joins/{}: not byte for byte {}'s {old}", j.name, w.id));
                    }
                }
                r.note.push(format!("{}: {same} of its {n} joins byte for byte the world's own", w.id));
            }
        }
    }

    let jt: Vec<Terrain> = joins.iter().map(Join::terrain).collect();
    let tt: Vec<Terrain> = tiles.iter().map(|t| t.0.terrain()).collect();

    // Seams.
    for (ti, (tile, at, k)) in tiles.iter().enumerate() {
        let t = &tt[ti];
        let s = tile.spacing;
        for edge in 0..4 {
            // Outward normal and the edge's run, in tile samples.
            let (fixed, along_axis, n) = match edge {
                0 => (k[0], 1, -1.0),
                1 => (k[1], 0, -1.0),
                2 => (k[2], 1, 1.0),
                _ => (k[3], 0, 1.0),
            };
            let normal_axis = 1 - along_axis;
            let (a0, a1) = (f64::from(k[along_axis]), f64::from(k[along_axis + 2]));
            // Points along the edge, in tile samples: every tile sample, and every sample of each join
            // that reaches the edge's line (counted only where that join is the one covering the seam).
            let mut pts: Vec<(f64, Option<usize>)> = (k[along_axis]..=k[along_axis + 2]).map(|a| (f64::from(a), None)).collect();
            let w0 = at[along_axis] + a0 * s;
            let w1 = at[along_axis] + a1 * s;
            let line = at[normal_axis] + f64::from(fixed) * s;
            for (ji, j) in joins.iter().enumerate() {
                let r = j.rect();
                if line < r[normal_axis] - 1e-6 || line > r[normal_axis + 2] + 1e-6 {
                    continue;
                }
                let (lo, hi) = (w0.max(r[along_axis]), w1.min(r[along_axis + 2]));
                let origin = j.at[along_axis];
                let mut l = ((lo - origin) / j.spacing - 1e-9).ceil();
                while origin + l * j.spacing <= hi + 1e-6 {
                    pts.push(((origin + l * j.spacing - at[along_axis]) / s, Some(ji)));
                    l += 1.0;
                }
            }
            for (a, from_join) in pts {
                let mut world = [0.0; 2];
                world[normal_axis] = line;
                world[along_axis] = at[along_axis] + a * s;
                // The tile side: the kept cell on this edge.
                let cn = if n < 0.0 { fixed } else { fixed - 1 };
                let ca = (a.floor() as u32).clamp(k[along_axis], k[along_axis + 2] - 1);
                let (cx, cz) = if normal_axis == 0 { (cn, ca) } else { (ca, cn) };
                let Some(q) = t.cell(cx, cz, 1) else {
                    r.fail.push(format!("{}: no cell {cx},{cz}", tile.id));
                    continue;
                };
                let ta = F((a - f64::from(ca)) as f32);
                // Corners: index = dx + 2 dz. Along the edge at the near and far side of the cell.
                let corner = |dn: usize, da: usize| -> f64 {
                    let i = if normal_axis == 0 { dn + 2 * da } else { da + 2 * dn };
                    f64::from(q.height[i].0)
                };
                let lerp = |dn: usize| f64::from((tile.scale * (F(corner(dn, 0) as f32) * (F::ONE - ta) + F(corner(dn, 1) as f32) * ta)).0);
                let (inner, outer) = if n < 0.0 { (lerp(1), lerp(0)) } else { (lerp(0), lerp(1)) };
                let th = outer;
                let tslope = (outer - inner) / s;
                // The join side: the first join covering a point just outside.
                let mut probe = world;
                probe[normal_axis] += n * 1e-3;
                let Some(ji) = joins.iter().position(|j| j.covers(probe[0], probe[1])) else {
                    r.fail.push(format!("{} edge {edge} at {:.2},{:.2}: no join outside it", tile.id, world[0], world[1]));
                    continue;
                };
                if from_join.is_some_and(|f| f != ji) {
                    continue; // a sample of a join that does not own this stretch of the seam
                }
                let j = &joins[ji];
                let on_lattice = from_join.is_some() || j.on_lattice(along_axis, world[along_axis]);
                let f = [(world[0] - j.at[0]) / j.spacing, (world[1] - j.at[1]) / j.spacing];
                let mut c = [0u32; 2];
                for ax in 0..2 {
                    let v = if ax == normal_axis {
                        if n > 0.0 { (f[ax] + 1e-6).floor() } else { (f[ax] - 1e-6).ceil() - 1.0 }
                    } else {
                        f[ax].floor()
                    };
                    c[ax] = (v.max(0.0) as u32).min(j.samples[ax] - 2);
                }
                let mut f2 = f;
                f2[normal_axis] += n;
                let (Some(jh), Some(jh2)) = (j.in_cell(&jt[ji], c[0], c[1], f[0], f[1]), j.in_cell(&jt[ji], c[0], c[1], f2[0], f2[1])) else {
                    r.fail.push(format!("joins/{}: no cell {:?}", j.name, c));
                    continue;
                };
                let jslope = (jh2 - jh) / j.spacing;
                let dh = (jh - th).abs();
                let ds = (jslope - tslope).abs();
                if on_lattice {
                    r.seam_points += 1;
                    r.worst_height = r.worst_height.max(dh);
                    r.worst_slope = r.worst_slope.max(ds);
                    let (tol_h, tol_s) = (j.quantum + 1e-4 * th.abs().max(1.0), 2.0 * j.quantum / j.spacing + 1e-4);
                    if dh > tol_h || ds > tol_s {
                        r.fail.push(format!(
                            "{} edge {edge} at ({:.3}, {:.3}): tile {th:.4} ft slope {tslope:.5}, join {} {jh:.4} ft slope {jslope:.5}",
                            tile.id, world[0], world[1], j.name
                        ));
                    }
                    // The look: the touching tile owns the seam.
                    let (sx, sz) = (f[0].round() as u32, f[1].round() as u32);
                    match j.between.iter().position(|b| *b == tile.id) {
                        Some(bi) => {
                            let at = (sx + sz * j.samples[0]) as usize * j.between.len() + bi;
                            if j.blend.get(at) != Some(&255) {
                                r.fail.push(format!("joins/{}.blend: {} is {:?}, not 255, on its seam at {sx},{sz}", j.name, tile.id, j.blend.get(at)));
                            }
                        }
                        None => r.fail.push(format!("joins/{}: on {}'s seam but not between it", j.name, tile.id)),
                    }
                } else {
                    r.between_points += 1;
                    r.between_height = r.between_height.max(dh);
                }
            }
        }
    }
    if r.between_points > 0 {
        r.note.push(format!(
            "{} tile samples on seams fall between join lattice points (spacings differ): worst height step {:.4} ft",
            r.between_points, r.between_height
        ));
    }
    join_seams(&tiles, &joins, &jt, &mut r);
    if !worlds.is_empty() {
        world_seams(&worlds, &joins, &jt, lattice.unwrap_or(joins[0].spacing), &mut r);
    }
    // Joins against each other, slopes, blend steps.
    let mut shared: HashMap<(i64, i64), (usize, f64)> = HashMap::new();
    // Each tile's seam gates (C9): mod.json's stitch.gates where the build widened them, C1's
    // `2 lattice + 3 fade` elsewhere. A cell is held to max_slope where it is past every one.
    let base = 2.0 * lattice.unwrap_or(joins[0].spacing) + 3.0 * fade;
    let listed: HashMap<String, Vec<f64>> = st
        .and_then(|s| s.get("gates"))
        .and_then(Json::arr)
        .unwrap_or_default()
        .iter()
        .filter_map(|g| Some((g.get("tile")?.str()?.to_owned(), nums(g.get("edges")))))
        .collect();
    let gates: Vec<[f64; 4]> = tiles
        .iter()
        .map(|(t, _, _)| match listed.get(&t.id) {
            Some(v) if v.len() == 4 => [v[0], v[1], v[2], v[3]],
            _ => [base; 4],
        })
        .collect();
    // An ultra map's new joins (no world's prefix) are held past each world's gates too.
    let clear = |x: f64, z: f64, new: bool| -> f64 {
        let past_worlds = if new {
            worlds
                .iter()
                .map(|w| {
                    let src = crate::Source { tile: 0, rect: w.rect, at: [0.0; 2], spacing: 1.0, keep: [0; 4], ring: crate::Ring::new(Default::default()), gates: w.gates };
                    src.distance(x, z) - src.gate_at(x, z)
                })
                .fold(f64::INFINITY, f64::min)
        } else {
            f64::INFINITY
        };
        rects
            .iter()
            .zip(&gates)
            .map(|(a, g)| {
                let (ux, uz) = ((a[0] - x).max(x - a[2]).max(0.0), (a[1] - z).max(z - a[3]).max(0.0));
                let v = if x < a[0] { Some(g[0]) } else if x > a[2] { Some(g[2]) } else { None };
                let h = if z < a[1] { Some(g[1]) } else if z > a[3] { Some(g[3]) } else { None };
                let gate = match (v, h) {
                    (Some(p), Some(q)) => p.max(q),
                    (Some(p), None) | (None, Some(p)) => p,
                    (None, None) => g.iter().copied().fold(0.0, f64::max),
                };
                ux.hypot(uz) - gate
            })
            .fold(past_worlds, f64::min)
    };
    for (ji, j) in joins.iter().enumerate() {
        let t = &jt[ji];
        let new = !worlds.is_empty() && !worlds.iter().any(|w| j.name.starts_with(&w.prefix));
        let [w, d] = j.samples;
        let mut h = vec![0.0; (w * d) as usize];
        for z in 0..d {
            for x in 0..w {
                h[(x + z * w) as usize] = j.sample(t, x, z).unwrap_or(f64::NAN);
            }
        }
        for z in 0..d {
            for x in 0..w {
                if x == 0 || z == 0 || x == w - 1 || z == d - 1 {
                    // Keyed by the sample's world position in hundredths of a foot: joins of two
                    // spacings share only the samples their lattices both have.
                    let wx = j.at[0] + f64::from(x) * j.spacing;
                    let wz = j.at[1] + f64::from(z) * j.spacing;
                    let key = ((wx * 100.0).round() as i64, (wz * 100.0).round() as i64);
                    let v = h[(x + z * w) as usize];
                    // To both joins' quanta: where pieces overlap (C11), an ultra map's new piece holds a
                    // world's edge as one of its joins has it, rounded again to its own quantum.
                    match shared.get(&key) {
                        Some(&(other, ov)) if (ov - v).abs() > j.quantum + joins[other].quantum + 1e-4 => {
                            r.fail.push(format!("joins {} and {} differ at lattice {key:?}: {ov:.4} and {v:.4}", joins[other].name, j.name));
                        }
                        Some(_) => {}
                        None => {
                            shared.insert(key, (ji, v));
                        }
                    }
                }
                if x + 1 < w && z + 1 < d {
                    let cx = j.at[0] + (f64::from(x) + 0.5) * j.spacing;
                    let cz = j.at[1] + (f64::from(z) + 0.5) * j.spacing;
                    let corners = [(x, z), (x + 1, z), (x, z + 1), (x + 1, z + 1)];
                    if corners.iter().all(|&(a, b)| clear(j.at[0] + f64::from(a) * j.spacing, j.at[1] + f64::from(b) * j.spacing, new) > 0.0) {
                        let g = crate::build::cell_slope(corners.map(|(a, b)| h[(a + b * w) as usize]), j.spacing);
                        if g > r.far_slope {
                            r.far_slope = g;
                        }
                        if g > max_slope + 1e-6 {
                            r.fail.push(format!("joins/{}: slope {g:.3} at ({cx:.0}, {cz:.0}), past {max_slope}", j.name));
                        }
                    }
                }
                let nb = j.between.len();
                let i = (x + z * w) as usize;
                for (dx, dz) in [(1, 0), (0, 1)] {
                    if x + dx < w && z + dz < d {
                        let o = ((x + dx) + (z + dz) * w) as usize;
                        for b in 0..nb {
                            let step = j.blend[i * nb + b].abs_diff(j.blend[o * nb + b]);
                            if step > MAX_BLEND_STEP {
                                r.fail.push(format!("joins/{}.blend: {} steps by {step} at {x},{z}", j.name, j.between[b]));
                            }
                        }
                    }
                }
            }
        }
    }
    // A failure list of thousands helps nobody: keep the first of each kind.
    if r.fail.len() > 50 {
        let n = r.fail.len();
        r.fail.truncate(50);
        r.fail.push(format!("... {} more", n - 50));
    }
    r
}

/// Who rides a world point: a tile inside its `keep` (`Ok`), else the first join covering it (`Err`).
fn ridden(tiles: &[(Tile, [f64; 2], [u32; 4])], joins: &[Join], x: f64, z: f64) -> Option<Result<usize, usize>> {
    let tile = tiles.iter().position(|(t, at, k)| {
        let (fx, fz) = ((x - at[0]) / t.spacing, (z - at[1]) / t.spacing);
        fx >= f64::from(k[0]) && fx < f64::from(k[2]) && fz >= f64::from(k[1]) && fz < f64::from(k[3])
    });
    match tile {
        Some(t) => Some(Ok(t)),
        None => joins.iter().position(|j| j.covers(x, z)).map(Err),
    }
}

/// Where a join meets a join of another spacing: along each edge of every join, at its own samples and at
/// the other spacings' samples, the ground just inside (this join, as ridden) and just outside (another
/// join, as ridden) must be the same height to the coarser join's height quantum.
fn join_seams(tiles: &[(Tile, [f64; 2], [u32; 4])], joins: &[Join], jt: &[Terrain], r: &mut Report) {
    // Every lattice the joins are on: (spacing, origin), one entry per spacing and alignment.
    let mut lattices: Vec<(f64, [f64; 2])> = Vec::new();
    for j in joins {
        let o = j.at.map(|a| a.rem_euclid(j.spacing));
        let same = |l: &(f64, [f64; 2])| (l.0 - j.spacing).abs() < 1e-9 && (0..2).all(|ax| ((l.1[ax] - o[ax]).abs() % j.spacing).min(j.spacing - (l.1[ax] - o[ax]).abs() % j.spacing) < 1e-6);
        if !lattices.iter().any(same) {
            lattices.push((j.spacing, o));
        }
    }
    if lattices.len() < 2 {
        return;
    }
    const E: f64 = 1e-3;
    for (ji, j) in joins.iter().enumerate() {
        let rect = j.rect();
        for edge in 0..4 {
            let (normal_axis, n) = match edge {
                0 => (0, -1.0),
                1 => (1, -1.0),
                2 => (0, 1.0),
                _ => (1, 1.0),
            };
            let along = 1 - normal_axis;
            let line = if n < 0.0 { rect[normal_axis] } else { rect[normal_axis + 2] };
            let (lo, hi) = (rect[along], rect[along + 2]);
            let mut pts: Vec<f64> = Vec::new();
            for (sp, o) in &lattices {
                let mut l = ((lo - o[along]) / sp - 1e-9).ceil();
                while o[along] + l * sp <= hi + 1e-6 {
                    pts.push(o[along] + l * sp);
                    l += 1.0;
                }
            }
            for a in pts {
                let mut p = [0.0; 2];
                p[normal_axis] = line;
                p[along] = a;
                let (mut pin, mut pout) = (p, p);
                pin[normal_axis] -= n * E;
                pout[normal_axis] += n * E;
                let (Some(Err(i)), Some(Err(o))) = (ridden(tiles, joins, pin[0], pin[1]), ridden(tiles, joins, pout[0], pout[1])) else {
                    continue;
                };
                let k = &joins[o];
                if i != ji || (k.spacing - j.spacing).abs() < 1e-9 {
                    continue;
                }
                let (Some(h_in), Some(h_out)) = (j.height_by(&jt[ji], p[0], p[1], pin), k.height_by(&jt[o], p[0], p[1], pout)) else {
                    r.fail.push(format!("joins {} and {}: no cell at ({:.3}, {:.3})", j.name, k.name, p[0], p[1]));
                    continue;
                };
                // Slope across, each from one cell of its own (only reported).
                let mut qin = p;
                qin[normal_axis] -= n * j.spacing;
                let mut qout = p;
                qout[normal_axis] += n * k.spacing;
                if let (Some(a), Some(b)) = (j.height_by(&jt[ji], qin[0], qin[1], pin), k.height_by(&jt[o], qout[0], qout[1], pout)) {
                    r.join_seam_slope = r.join_seam_slope.max(((h_in - a) / j.spacing - (b - h_out) / k.spacing).abs());
                }
                let dh = (h_in - h_out).abs();
                r.join_seam_points += 1;
                r.join_seam_height = r.join_seam_height.max(dh);
                if dh > j.quantum.max(k.quantum) + 1e-4 * h_in.abs().max(1.0) {
                    r.fail.push(format!("joins {} and {} at ({:.3}, {:.3}): {h_in:.4} ft against {h_out:.4}", j.name, k.name, p[0], p[1]));
                }
            }
        }
    }
    if r.join_seam_points > 0 {
        r.note.push(format!(
            "{} points where joins of two spacings meet: worst height step {:.5} ft, slope step {:.4} (not failed)",
            r.join_seam_points, r.join_seam_height, r.join_seam_slope
        ));
    }
}

/// A world of an ultra map (C5), from `stitch.worlds`.
struct UltraWorld {
    id: String,
    prefix: String,
    rect: [f64; 4],
    gates: [f64; 4],
}

/// Where an ultra map's new ground meets a world's edge: at every lattice sample of the edge, the world's
/// side (its own join just inside) and the new side (the first join just outside) must be the same height,
/// and the cell either side must have the same slope across, to the joins' height quantum (a tile's seam's
/// tolerance). A point of the edge with nothing outside it is the ultra map's own outer edge.
fn world_seams(worlds: &[UltraWorld], joins: &[Join], jt: &[Terrain], step: f64, r: &mut Report) {
    const E: f64 = 1e-3;
    let first = |p: [f64; 2]| joins.iter().position(|j| j.covers(p[0], p[1]));
    for w in worlds {
        for edge in 0..4 {
            let (normal_axis, n) = match edge {
                0 => (0, -1.0),
                1 => (1, -1.0),
                2 => (0, 1.0),
                _ => (1, 1.0),
            };
            let along = 1 - normal_axis;
            let line = if n < 0.0 { w.rect[normal_axis] } else { w.rect[normal_axis + 2] };
            let count = ((w.rect[along + 2] - w.rect[along]) / step).round() as u32;
            for k in 0..=count {
                let mut p = [0.0; 2];
                p[normal_axis] = line;
                p[along] = w.rect[along] + f64::from(k) * step;
                let at = |d: f64| {
                    let mut q = p;
                    q[normal_axis] += n * d;
                    q
                };
                let Some(o) = first(at(E)) else { continue };
                let Some(i) = first(at(-E)) else {
                    r.fail.push(format!("{} edge {edge} at ({:.1}, {:.1}): no join inside it", w.id, p[0], p[1]));
                    continue;
                };
                let (ji, jo) = (&joins[i], &joins[o]);
                if !ji.name.starts_with(&w.prefix) || jo.name.starts_with(&w.prefix) {
                    r.fail.push(format!("{} edge {edge} at ({:.1}, {:.1}): {} inside and {} outside", w.id, p[0], p[1], ji.name, jo.name));
                    continue;
                }
                let (pin, pout, qin, qout) = (at(-E), at(E), at(-step), at(step));
                let (Some(hi), Some(hi2), Some(ho), Some(ho2)) = (
                    ji.height_by(&jt[i], p[0], p[1], pin),
                    ji.height_by(&jt[i], qin[0], qin[1], pin),
                    jo.height_by(&jt[o], p[0], p[1], pout),
                    jo.height_by(&jt[o], qout[0], qout[1], pout),
                ) else {
                    r.fail.push(format!("{} edge {edge} at ({:.1}, {:.1}): no cell in {} or {}", w.id, p[0], p[1], ji.name, jo.name));
                    continue;
                };
                let (si, so) = ((hi - hi2) / step, (ho2 - ho) / step);
                let (dh, ds) = ((ho - hi).abs(), (so - si).abs());
                let q = ji.quantum.max(jo.quantum);
                r.world_seam_points += 1;
                r.world_seam_height = r.world_seam_height.max(dh);
                r.world_seam_slope = r.world_seam_slope.max(ds);
                if dh > q + 1e-4 * hi.abs().max(1.0) || ds > 2.0 * q / step + 1e-4 {
                    r.fail.push(format!(
                        "{} edge {edge} at ({:.1}, {:.1}): {} {hi:.4} ft slope {si:.5}, {} {ho:.4} ft slope {so:.5}",
                        w.id, p[0], p[1], ji.name, jo.name
                    ));
                }
            }
        }
    }
    r.note.push(format!(
        "{} points where new ground meets a world's edge: worst height step {:.5} ft, slope step {:.6}",
        r.world_seam_points, r.world_seam_height, r.world_seam_slope
    ));
}

fn compare(src: &Path, copy: &Path, name: &str, r: &mut Report) {
    let (Ok(a), Ok(b)) = (crate::files(src), crate::files(copy)) else {
        r.fail.push(format!("tiles/{name}: cannot list it or its source"));
        return;
    };
    if a != b {
        r.fail.push(format!("tiles/{name}: its files are not its source's ({} against {})", b.len(), a.len()));
        return;
    }
    for f in &a {
        if std::fs::read(src.join(f)).ok() != std::fs::read(copy.join(f)).ok() {
            r.fail.push(format!("tiles/{name}/{f}: differs from {}", src.join(f).display()));
        }
    }
    r.note.push(format!("tiles/{name}: {} files byte for byte its source's", a.len()));
}

/// The world's ground every `step` feet as it would be ridden (a tile inside its `keep`, else the first
/// join covering the point), for a picture: `(columns, rows, heights)`, NaN off the world.
pub fn heightmap(dir: &Path, step: f64) -> Result<(usize, usize, Vec<f64>), String> {
    let meta = Json::parse(&std::fs::read_to_string(dir.join("mod.json")).map_err(|e| e.to_string())?)?;
    let mut tiles = Vec::new();
    for t in meta.get("tiles").and_then(Json::arr).unwrap_or_default() {
        let name = t.get("tile").and_then(Json::str).unwrap_or_default();
        let (at, k) = (nums(t.get("at")), nums(t.get("keep")));
        tiles.push((Tile::load(&dir.join("tiles").join(name))?, [at[0], at[1]], [k[0] as u32, k[1] as u32, k[2] as u32, k[3] as u32]));
    }
    let mut joins = Vec::new();
    for jn in meta.get("joins").and_then(Json::arr).unwrap_or_default() {
        let name = jn.get("join").and_then(Json::str).unwrap_or_default().to_owned();
        let (at, samples) = (nums(jn.get("at")), nums(jn.get("samples")));
        let bytes = std::fs::read(dir.join("joins").join(&name).join(format!("{name}.ps2.trn"))).map_err(|e| e.to_string())?;
        let t = Terrain::parse(&bytes)?;
        let (spacing, scale) = (f64::from(t.scale.0), t.scale);
        drop(t);
        joins.push(Join { name, at: [at[0], at[1]], samples: [samples[0] as u32, samples[1] as u32], between: Vec::new(), bytes, blend: Vec::new(), spacing, scale, quantum: 0.0 });
    }
    let tt: Vec<Terrain> = tiles.iter().map(|t| t.0.terrain()).collect();
    let jt: Vec<Terrain> = joins.iter().map(Join::terrain).collect();
    let (mut wx, mut wz) = (0.0f64, 0.0f64);
    for j in &joins {
        let r = j.rect();
        wx = wx.max(r[2]);
        wz = wz.max(r[3]);
    }
    let (cols, rows) = ((wx / step) as usize + 1, (wz / step) as usize + 1);
    let mut out = vec![f64::NAN; cols * rows];
    for r in 0..rows {
        for c in 0..cols {
            let (x, z) = (c as f64 * step, r as f64 * step);
            let tile = tiles.iter().enumerate().find_map(|(i, (t, at, k))| {
                let (fx, fz) = ((x - at[0]) / t.spacing, (z - at[1]) / t.spacing);
                (fx >= f64::from(k[0]) && fx < f64::from(k[2]) && fz >= f64::from(k[1]) && fz < f64::from(k[3]))
                    .then(|| t.ground_in(&tt[i], *k, fx, fz).map(|g| g.0))
                    .flatten()
            });
            out[c + r * cols] = match tile {
                Some(h) => h,
                None => joins
                    .iter()
                    .enumerate()
                    .find(|(_, j)| j.covers(x, z))
                    .and_then(|(i, j)| {
                        let (fx, fz) = ((x - j.at[0]) / j.spacing, (z - j.at[1]) / j.spacing);
                        let (cx, cz) = ((fx.floor() as u32).min(j.samples[0] - 2), (fz.floor() as u32).min(j.samples[1] - 2));
                        j.in_cell(&jt[i], cx, cz, fx, fz)
                    })
                    .unwrap_or(f64::NAN),
            };
        }
    }
    Ok((cols, rows, out))
}
