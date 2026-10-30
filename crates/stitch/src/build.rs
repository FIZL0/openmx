//! Laying the tiles out, solving the join ground, and writing the world mod.

use crate::jsonw::{J, o, s};
use crate::json::Json;
use crate::{Source, Tile, shepard};
use game_sim::track::Terrain;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const AUTHOR: &str = "openmx-mods tools/stitch 0.1";

/// What a world is built with.
#[derive(Clone, Debug)]
pub struct Params {
    pub id: String,
    pub name: String,
    pub version: String,
    /// The steepest join ground allowed away from the seams, feet per foot. Not a number the game has
    /// (it has no climbing limit; the bike either makes it or does not): 0.5 is 26.6°.
    pub max_slope: f64,
    /// The narrowest gap between two interiors, feet.
    pub min_gap: f64,
    /// Join pieces are at most this many lattice cells a side (whole leaves, at most 256: [`cuts`]).
    pub chunk: u32,
    /// How far a tile's edge slope carries into the join before it levels off, feet.
    pub fade: f64,
    /// The tile that lights the world under Settings → World sky: Global (MD9). Default: the first.
    pub sky: Option<String>,
    /// Which axes the world wraps round (`"x"`, `"z"` or `"xz"`), written as mod.json's `wrap` (§3,
    /// openmx MD29). None, the world has edges.
    pub wrap: Option<String>,
    /// Lines the world spec adds to the world's todo (a gap the spec chose, e.g. a raised `max_slope`).
    pub todo: Vec<String>,
    /// Threads for the join ground (0: as many as the machine has).
    pub threads: usize,
    /// Give each point only the tiles that can have a say there (the spatial grid, [`Plan::bins`]).
    /// Off, every tile is a candidate everywhere; the ground is the same either way (C7's test).
    pub prune: bool,
}

impl Params {
    pub fn new(id: &str, name: &str) -> Params {
        Params {
            id: id.to_owned(),
            name: name.to_owned(),
            version: "1.0.0".into(),
            max_slope: 0.5,
            min_gap: 2.0 * crate::SHOOTOFF,
            chunk: 256,
            fade: 48.0,
            sky: None,
            wrap: None,
            todo: Vec::new(),
            threads: 0,
            prune: true,
        }
    }
}

impl Params {
    /// A world spec's build parameters (`worlds/<id>.json`: `id`, `name`, and optional `sky`, `wrap`,
    /// `max_slope`, `min_gap`, `fade`, `chunk`, `todo`), as openmx-mods' `stitch --mods --out` reads
    /// them; its `tiles` are the caller's to find.
    pub fn from_spec(j: &Json) -> Result<Params, String> {
        let g = |k: &str| j.get(k).and_then(Json::str).map(str::to_owned);
        let mut p = Params::new(&g("id").ok_or("no id")?, &g("name").ok_or("no name")?);
        p.sky = g("sky");
        p.wrap = g("wrap");
        p.todo = j.get("todo").and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::str).map(str::to_owned).collect();
        if let Some(v) = j.get("max_slope").and_then(Json::f64) {
            p.max_slope = v;
        }
        if let Some(v) = j.get("min_gap").and_then(Json::f64) {
            p.min_gap = v;
        }
        if let Some(v) = j.get("fade").and_then(Json::f64) {
            p.fade = v;
        }
        if let Some(v) = j.get("chunk").and_then(Json::f64) {
            p.chunk = v as u32;
        }
        Ok(p)
    }
}

/// One tile's place.
#[derive(Clone, Debug)]
pub struct Placed {
    /// World feet of the tile's sample (0, 0).
    pub at: [f64; 2],
    /// The interior, after the berm cut and the lattice alignment.
    pub keep: [u32; 4],
    /// Where in the layout's grid (column, row).
    pub cell: (usize, usize),
}

/// The whole layout: the join lattice, every tile's place, the blend's sources.
pub struct Plan {
    /// Feet between join samples, the same for every join piece.
    pub spacing: f64,
    /// The lattice's last sample on each axis: the world is `[0, size × spacing]`.
    pub size: [u32; 2],
    pub placed: Vec<Placed>,
    pub sources: Vec<Source>,
    /// The largest Shepard radius, feet.
    pub radius: f64,
    /// Each source's Shepard radius, feet ([`radii`]).
    pub radii: Vec<f64>,
    /// The sources each square of the world can hear from.
    pub bins: Bins,
    pub prune: bool,
    pub fade: f64,
    /// Every tile's spacing as `p/q` of the lattice's, where one with q ≤ 64 exists.
    pub ratio: Vec<Option<(i64, i64)>>,
    pub widen: f64,
    /// The joins at a tile's own spacing, round each tile whose spacing is not the lattice's (C6).
    pub collars: Vec<Collar>,
    /// Tiles off the lattice that have no collar, and why.
    pub uncollared: Vec<(usize, String)>,
}

/// A band of join ground at one tile's own spacing round its interior, for a tile whose spacing is not
/// the lattice's (3.5 ft beside 4.5 ft), so its seams meet join samples at every one of its own samples.
///
/// The collar's outer edges lie on lines both lattices have, and its corners and every `period` feet
/// along them (q tile samples, p lattice samples) are samples of both. Within `band` of an edge the field
/// is pulled onto the straight line between those shared samples ([`Plan::at`]), fully on the edge and
/// fading to nothing with zero slope, so the collar's piecewise-linear ground and the lattice's agree
/// exactly along it.
#[derive(Clone, Debug)]
pub struct Collar {
    /// The tile, which is also its source's index.
    pub tile: usize,
    pub spacing: f64,
    /// World feet, the interior grown every way by a whole number of periods, each edge on a whole foot;
    /// `width` is the widest edge's.
    pub rect: [f64; 4],
    pub width: f64,
    pub period: f64,
    pub band: f64,
}

/// `1` at 0, falling with zero slope at both ends to `0` at 1 and past.
fn fall(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    (1.0 - t) * (1.0 - t) * (1.0 + 2.0 * t)
}

/// The distance between two closed rectangles.
fn rect_gap(a: [f64; 4], b: [f64; 4]) -> f64 {
    let dx = (a[0] - b[2]).max(b[0] - a[2]).max(0.0);
    let dz = (a[1] - b[3]).max(b[1] - a[3]).max(0.0);
    dx.hypot(dz)
}

/// One join piece to compute: its origin in world feet, its spacing (the lattice's, or a collared tile's)
/// and its cells a side.
#[derive(Clone, Debug)]
pub struct Spec {
    pub name: String,
    pub at: [f64; 2],
    pub spacing: f64,
    pub cells: [u32; 2],
}

impl Spec {
    fn rect(&self) -> [f64; 4] {
        [
            self.at[0],
            self.at[1],
            self.at[0] + f64::from(self.cells[0]) * self.spacing,
            self.at[1] + f64::from(self.cells[1]) * self.spacing,
        ]
    }
}

/// Cells a terrain leaf spans: a leaf holds 17 × 17 samples, the last shared with the next leaf.
pub const LEAF: u32 = 16;

/// How a run of pieces meets the far end of what it covers ([`cuts`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    /// The last piece ends on the far end, moved back over the one before it: nothing past the end is
    /// written (a world's lattice, a collar).
    Back,
    /// The last piece runs a cell or more past the far end, onto what lies beyond (the ultra map's new
    /// ground, whose neighbour past the end starts right there).
    Over,
}

/// Cut cells `[a, b)` of one axis into pieces `(origin, cells)` for openmx (C11). openmx owns a join's
/// terrain over its tree's whole leaves, so every piece is whole leaves ([`LEAF`]) and its file holds the
/// ground on all of them, with no padding a reader could take for ground. It owns every cell but the
/// last row and column of them, so each piece reaches a leaf into the next, which covers that cell.
/// `chunk` is the most cells a piece has (rounded down to whole leaves, 32 to 256: a one-level tree
/// has 16 leaves a side); pieces start `chunk − 16` apart. `whole` says whether an origin is on a whole
/// foot; a piece moved back that is not is moved back a cell more until it is. A run shorter than a leaf
/// is one piece of whole leaves from `a`, past `b` whatever `end` says.
pub fn cuts(a: u32, b: u32, chunk: u32, end: End, whole: &dyn Fn(u32) -> bool) -> Vec<(u32, u32)> {
    let f = (chunk / LEAF * LEAF).clamp(2 * LEAF, LEAF * LEAF);
    let leaves = |n: u32| n.div_ceil(LEAF) * LEAF;
    let mut out = Vec::new();
    if b <= a {
        return out;
    }
    let mut o = a;
    let last = loop {
        // Back: the rest fits in this piece; Over: it fits with a cell to spare past `b`.
        let fits = match end {
            End::Back => o + f >= b,
            End::Over => o + f > b,
        };
        if fits {
            break o;
        }
        out.push((o, f));
        o += f - LEAF;
    };
    let rest = b - last;
    match end {
        End::Over => out.push((last, leaves(rest + 1))),
        End::Back => {
            let back = |n: u32| {
                let mut s = b - n;
                while s > a && !whole(s) {
                    s -= 1;
                }
                (s, n)
            };
            let n = leaves(rest);
            if b - a < LEAF {
                out.push((a, LEAF));
            } else if last == a && rest % LEAF != 0 {
                // One run of a few leaves and a part: its whole leaves, and one more ending on `b`.
                out.push((a, rest / LEAF * LEAF));
                out.push(back(LEAF));
            } else {
                out.push(back(n));
            }
        }
    }
    out
}

/// The fewest lattice samples, `at_least` or more, whose length is a whole number of feet: a world's far
/// edge starts the joins cut along it (an ultra map's), and openmx asks a whole foot of every `at` (an odd
/// count of 4.5 ft samples ends on a half).
pub fn whole_samples(at_least: u32, step: f64) -> u32 {
    (at_least..at_least + 64).find(|n| (f64::from(*n) * step).fract() == 0.0).unwrap_or(at_least)
}

/// `s / lattice` as a small fraction, if there is one.
pub fn ratio(s: f64, lattice: f64) -> Option<(i64, i64)> {
    let r = s / lattice;
    (1..=64).find_map(|q| {
        let p = (r * q as f64).round();
        ((r * q as f64 - p).abs() < 1e-9 && p > 0.0).then_some((p as i64, q))
    })
}

/// The lattice spacing: the commonest tile spacing, the finer on a tie.
pub fn lattice_spacing(tiles: &[Tile]) -> f64 {
    let mut count: BTreeMap<u64, usize> = BTreeMap::new();
    for t in tiles {
        *count.entry(t.spacing.to_bits()).or_default() += 1;
    }
    let mut v: Vec<(f64, usize)> = count.into_iter().map(|(b, n)| (f64::from_bits(b), n)).collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.total_cmp(&b.0)));
    v[0].0
}

/// The gap two edges at these heights need so the blend between them stays under the slope: the
/// Shepard profile is at most about 1.6 times the mean slope, plus the two edges' room. An edge's room
/// is `step + fade` (its taper), and as much more as its seam gate is past C1's `2 step + 3 fade` (C9).
pub fn gap_for(dh: f64, p: &Params, rooms: f64, widen: f64) -> f64 {
    (1.6 * dh.abs() / p.max_slope + rooms).max(p.min_gap) * widen
}

/// Place one interior's near edge at or past `want` (world feet) along one axis: the tile's origin on a
/// whole leaf (16 samples), and the interior's edges on the join lattice where the spacings allow it
/// (the near edge cut a few samples more, the far edge a few less). Returns (origin, lo, hi).
///
/// The origin is rounded to whole feet, which openmx asks of every `at` so world − `at` is exact. For a
/// spacing a float holds exactly that is the leaf itself; for one it holds only nearly (Untamed's
/// Nat_OR, 4.499999 ft) a whole number of its leaves is a hair off a foot, and the foot is kept.
fn place_axis(want: f64, s: f64, lo: u32, hi: u32, ratio: Option<(i64, i64)>) -> (f64, u32, u32) {
    let leaf = 16.0 * s;
    let k = ((want - f64::from(lo) * s) / leaf).ceil() as i64;
    let (mut lo2, mut hi2) = (lo, hi);
    if let Some((_, q)) = ratio
        && q > 1
    {
        let mut d = 0;
        while (16 * k + i64::from(lo) + d).rem_euclid(q) != 0 {
            d += 1;
        }
        lo2 = lo + d as u32;
        hi2 = lo2 + ((hi - lo2) as i64 / q * q) as u32;
    }
    ((k as f64 * leaf).round(), lo2, hi2)
}

/// Lay the tiles out in a grid, ordered by the height of their interiors' edges and filled in a snake,
/// so neighbours are close in height; each gap is as wide as its two facing edges' heights need.
/// `gates` are each tile's seam gates per edge ([`gates`], measured on its cut).
pub fn plan(tiles: &[Tile], cuts: &[[u32; 4]], gates: &[[f64; 4]], p: &Params, widen: f64) -> Result<Plan, String> {
    if tiles.len() < 2 {
        return Err("a world needs two tiles or more".into());
    }
    let step = lattice_spacing(tiles);
    let ratios: Vec<Option<(i64, i64)>> = tiles.iter().map(|t| ratio(t.spacing, step)).collect();
    let rings: Vec<crate::Ring> = tiles.iter().zip(cuts).map(|(t, k)| t.ring(*k)).collect();
    let mut order: Vec<usize> = (0..tiles.len()).collect();
    order.sort_by(|a, b| rings[*a].mean().total_cmp(&rings[*b].mean()));
    let cols = (tiles.len() as f64).sqrt().ceil() as usize;
    let rows = tiles.len().div_ceil(cols);
    let mut grid = vec![vec![None; cols]; rows];
    let mut cell = vec![(0, 0); tiles.len()];
    for (i, &t) in order.iter().enumerate() {
        let (r, mut c) = (i / cols, i % cols);
        if r % 2 == 1 {
            c = cols - 1 - c;
        }
        grid[r][c] = Some(t);
        cell[t] = (c, r);
    }
    let pad = crate::SHOOTOFF + p.min_gap / 2.0;
    let mut at = vec![[0.0; 2]; tiles.len()];
    let mut keep = cuts.to_vec();
    // axis 0: columns along x (keep[0], keep[2]; edges 0 and 2); axis 1: rows along z.
    for axis in 0..2 {
        let lines = if axis == 0 { cols } else { rows };
        let members = |l: usize| -> Vec<usize> {
            if axis == 0 { grid.iter().filter_map(|row| row[l]).collect() } else { grid[l].iter().flatten().copied().collect() }
        };
        let width = |t: usize| f64::from(cuts[t][axis + 2] - cuts[t][axis]) * tiles[t].spacing;
        let wmax_of = |m: &[usize]| m.iter().map(|&t| width(t)).fold(0.0, f64::max);
        // The first line starts far enough out that no tile's origin is below 0 (openmx refuses a
        // negative `at`): a tile cut deep, at its ridge (M23), has its interior far from its origin.
        let first = members(0);
        let mut cursor = first
            .iter()
            .map(|&t| f64::from(cuts[t][axis]) * tiles[t].spacing - (wmax_of(&first) - width(t)) / 2.0)
            .fold(pad, f64::max);
        for l in 0..lines {
            let m = members(l);
            let wmax = wmax_of(&m);
            let mut far = cursor;
            for &t in &m {
                let s = tiles[t].spacing;
                let want = (cursor + (wmax - width(t)) / 2.0).max(f64::from(cuts[t][axis]) * s);
                let (origin, lo, hi) = place_axis(want, s, cuts[t][axis], cuts[t][axis + 2], ratios[t]);
                at[t][axis] = origin;
                keep[t][axis] = lo;
                keep[t][axis + 2] = hi;
                far = far.max(origin + f64::from(hi) * s);
            }
            // The gap to the next line: the widest any facing pair needs.
            let mut gap = p.min_gap * widen;
            if l + 1 < lines {
                let pairs: Vec<(Option<usize>, Option<usize>)> = if axis == 0 {
                    grid.iter().map(|row| (row[l], row[l + 1])).collect()
                } else {
                    (0..cols).map(|c| (grid[l][c], grid[l + 1][c])).collect()
                };
                for (a, b) in pairs {
                    if let (Some(a), Some(b)) = (a, b) {
                        let dh = rings[a].edge_mean(axis + 2) - rings[b].edge_mean(axis);
                        let room = |t: usize, e: usize| gates[t][e] - (step + 2.0 * p.fade);
                        gap = gap.max(gap_for(dh, p, room(a, axis + 2) + room(b, axis), widen));
                    }
                }
            }
            cursor = far + gap;
        }
    }
    let mut far = [0.0f64; 2];
    let sources: Vec<Source> = (0..tiles.len())
        .map(|t| {
            let s = tiles[t].spacing;
            let k = keep[t];
            let rect = [
                at[t][0] + f64::from(k[0]) * s,
                at[t][1] + f64::from(k[1]) * s,
                at[t][0] + f64::from(k[2]) * s,
                at[t][1] + f64::from(k[3]) * s,
            ];
            far[0] = far[0].max(rect[2]);
            far[1] = far[1].max(rect[3]);
            Source { tile: t, rect, at: at[t], spacing: s, keep: k, ring: tiles[t].ring(k), gates: gates[t] }
        })
        .collect();
    let size = [0, 1].map(|a| whole_samples(((far[a] + pad) / step).ceil() as u32, step));
    let radii = radii(&sources, size, step);
    let radius = radii.iter().copied().fold(0.0, f64::max);
    let bins = Bins::new(&sources, &radii, size, step);
    let placed = (0..tiles.len()).map(|t| Placed { at: at[t], keep: keep[t], cell: cell[t] }).collect();
    let (collars, uncollared) = collars(&sources, &ratios, step, size, p.fade);
    Ok(Plan { spacing: step, size, placed, sources, radius, radii, bins, prune: p.prune, fade: p.fade, ratio: ratios, widen, collars, uncollared })
}

/// The share of `max_slope` a tile's extension alone may take past its seam gate: the rest is left for
/// the blend between tiles, which the widening brings down (C9).
pub const GATE_SHARE: f64 = 0.75;

/// A source's seam gates (C9): `2 lattice + 3 fade` (C1's), or farther out on an edge whose own
/// extension is steeper than [`GATE_SHARE`] of `max_slope` there.
pub fn gates(src: &Source, step: f64, p: &Params) -> [f64; 4] {
    src.edge_gates(step, p.fade, GATE_SHARE * p.max_slope, 2.0 * step + 3.0 * p.fade)
}

/// One tile as a source on its own at the origin, with its gates on a lattice of its own spacing (for
/// `stitch edges`).
pub fn lone_source(t: &Tile, p: &Params) -> Source {
    cut_source(t, t.find_keep(), t.spacing, p)
}

/// A tile's interior `k` as a source at the origin, with its gates on a lattice of `step` feet.
pub fn cut_source(t: &Tile, k: [u32; 4], step: f64, p: &Params) -> Source {
    let s = t.spacing;
    let rect = [f64::from(k[0]) * s, f64::from(k[1]) * s, f64::from(k[2]) * s, f64::from(k[3]) * s];
    let mut src = Source { tile: 0, rect, at: [0.0; 2], spacing: s, keep: k, ring: t.ring(k), gates: [0.0; 4] };
    src.gates = gates(&src, step, p);
    src
}

/// Each source's Shepard radius: twice the farthest any world point is from its nearest interior, over
/// the points of this source's own neighbourhood. Those are the points it is the nearest to, or within one
/// coarse cell's diagonal of the nearest (so the boundaries between neighbourhoods are not missed between
/// coarse points). On the boundary between two neighbourhoods, both tiles then reach at least twice as far
/// as the point is from either, so each point near the middle of a gap has two tiles' say, as C1's one
/// radius (twice the farthest point of the whole world) gave. A gap made wide by one tall neighbour no
/// longer widens every tile's reach across the world.
pub fn radii(sources: &[Source], size: [u32; 2], step: f64) -> Vec<f64> {
    let stride = 8;
    let cell = step * stride as f64;
    let slack = cell * std::f64::consts::SQRT_2;
    let mut far = vec![0.0f64; sources.len()];
    let mut d = vec![0.0; sources.len()];
    for kz in (0..=size[1]).step_by(stride) {
        for kx in (0..=size[0]).step_by(stride) {
            let (x, z) = (f64::from(kx) * step, f64::from(kz) * step);
            for (di, s) in d.iter_mut().zip(sources) {
                *di = s.distance(x, z);
            }
            let d1 = d.iter().copied().fold(f64::INFINITY, f64::min);
            for (f, di) in far.iter_mut().zip(&d) {
                if *di <= d1 + slack {
                    *f = f.max(d1);
                }
            }
        }
    }
    // The coarse lattice's margin, as C1 had: no point is more than a few coarse cells from one sampled.
    far.iter().map(|f| 2.0 * f + 4.0 * cell).collect()
}

/// A spatial grid over the world: for each square, the sources that can have a say at some point within
/// one lattice step of it (Shepard weight above 0, `d − step < radius`), in source order. A point past
/// the world's edge (a piece's normals margin) belongs to the nearest square.
pub struct Bins {
    /// Feet a square a side.
    pub side: f64,
    pub n: [usize; 2],
    pub cells: Vec<Vec<usize>>,
}

/// Lattice cells a side of one [`Bins`] square.
pub const BIN: u32 = 32;

impl Bins {
    pub fn new(sources: &[Source], radii: &[f64], size: [u32; 2], step: f64) -> Bins {
        let side = f64::from(BIN) * step;
        let n = [size[0].div_ceil(BIN).max(1) as usize, size[1].div_ceil(BIN).max(1) as usize];
        let mut cells = Vec::with_capacity(n[0] * n[1]);
        for bz in 0..n[1] {
            for bx in 0..n[0] {
                let r = [bx as f64 * side, bz as f64 * side, (bx + 1) as f64 * side, (bz + 1) as f64 * side];
                cells.push((0..sources.len()).filter(|&c| rect_gap(r, sources[c].rect) <= radii[c] + 2.0 * step).collect());
            }
        }
        Bins { side, n, cells }
    }

    fn index(&self, x: f64, z: f64) -> [usize; 2] {
        let i = |v: f64, n: usize| ((v / self.side).floor().max(0.0) as usize).min(n - 1);
        [i(x, self.n[0]), i(z, self.n[1])]
    }

    /// The sources that can have a say at a world point.
    pub fn at(&self, x: f64, z: f64) -> &[usize] {
        let [bx, bz] = self.index(x, z);
        &self.cells[bx + bz * self.n[0]]
    }

    /// Every source any square under a world rectangle lists, in source order.
    pub fn over(&self, rect: [f64; 4]) -> Vec<usize> {
        let ([x0, z0], [x1, z1]) = (self.index(rect[0], rect[1]), self.index(rect[2], rect[3]));
        let mut out: Vec<usize> = (z0..=z1).flat_map(|bz| (x0..=x1).flat_map(move |bx| self.cells[bx + bz * self.n[0]].iter().copied())).collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// A collar round every tile whose spacing is a small fraction p/q (q > 1) of the lattice's, where one
/// fits: its pull band must stay `2 × lattice` clear of every other interior (so no other seam's pinned
/// ring is touched) and of every other collar's band, and inside the world.
fn collars(sources: &[Source], ratios: &[Option<(i64, i64)>], step: f64, size: [u32; 2], fade: f64) -> (Vec<Collar>, Vec<(usize, String)>) {
    let mut out: Vec<Collar> = Vec::new();
    let mut skipped = Vec::new();
    for (t, src) in sources.iter().enumerate() {
        let Some((_, q)) = ratios[t] else {
            if (src.spacing - step).abs() > 1e-9 {
                skipped.push((t, "its spacing has no small ratio to the lattice's".to_owned()));
            }
            continue;
        };
        if q == 1 {
            continue; // every one of its samples is a lattice point already
        }
        let s = src.spacing;
        let period = q as f64 * s;
        let band = fade.max(period);
        // The pull stays off the tile's own pinned ring: width − band > 2 × the coarser spacing.
        let periods = ((band + 2.0 * step.max(s)) / period).ceil();
        let r = src.rect;
        // Each edge as many periods out as that, or a few more until it is on a whole foot: the collar's
        // joins start on its edges, and openmx asks a whole foot of every `at` (a 31.5 ft period puts
        // every other edge on a half).
        let edge = |e: usize| {
            let out = if e < 2 { -1.0 } else { 1.0 };
            (0..8).map(|n| r[e] + out * (periods + f64::from(n)) * period).find(|v| v.fract() == 0.0)
        };
        let edges = [edge(0), edge(1), edge(2), edge(3)];
        let rect = edges.map(|e| e.unwrap_or(f64::NAN));
        let width = (0..4).map(|e| (rect[e] - r[e]).abs()).fold(0.0, f64::max);
        let world = [f64::from(size[0]) * step, f64::from(size[1]) * step];
        let why = if edges.contains(&None) {
            Some(format!("no edge {period} ft periods out is on a whole foot"))
        } else if rect[0] < 0.0 || rect[1] < 0.0 || rect[2] > world[0] || rect[3] > world[1] {
            Some("it would reach past the world's edge".to_owned())
        } else if let Some(o) = sources.iter().find(|o| o.tile != t && rect_gap(rect, o.rect) < band + 2.0 * step) {
            Some(format!("tile {}'s interior is too near", o.tile))
        } else {
            out.iter().find(|c| rect_gap(rect, c.rect) < band + c.band).map(|c| format!("tile {}'s collar is too near", c.tile))
        };
        match why {
            Some(w) => skipped.push((t, format!("no collar of {width} ft fits: {w}"))),
            None => out.push(Collar { tile: t, spacing: s, rect, width, period, band }),
        }
    }
    (out, skipped)
}

/// The join ground at one world point.
pub struct Point {
    pub h: f64,
    pub surface: u8,
    /// Each candidate's share, summing to 1.
    pub w: Vec<f64>,
    /// Distance to the nearest interior.
    pub near: f64,
}

impl Plan {
    /// The ground at a world point, from the candidate sources `cands` (indices into `sources`): the
    /// blend, pulled straight between the shared samples along every collar edge it is near.
    pub fn at(&self, tiles: &[Tile], terrains: &[Terrain], cands: &[usize], x: f64, z: f64) -> Point {
        let mut pt = self.blend(tiles, terrains, cands, x, z);
        if pt.near > 2.0 * self.spacing && !self.collars.is_empty() {
            pt.h += self.pull(tiles, terrains, x, z);
        }
        pt
    }

    /// How far the collar edges near a point pull the blend there: for each edge within its band, the
    /// straight line between the shared samples either side of the point's foot on the edge, less the
    /// blend at the foot, times the fall-off across the edge and past its ends. On an edge that is the
    /// line itself. At a collar corner the other edge's pull is 0, its foot being a shared sample.
    fn pull(&self, tiles: &[Tile], terrains: &[Terrain], x: f64, z: f64) -> f64 {
        let all: Vec<usize> = (0..self.sources.len()).collect();
        let f = |p: [f64; 2]| self.blend(tiles, terrains, &all, p[0], p[1]).h;
        let p = [x, z];
        let mut out = 0.0;
        for c in &self.collars {
            // (axis held fixed, its value): x = x0, z = z0, x = x1, z = z1.
            for (fixed, v) in [(0, c.rect[0]), (1, c.rect[1]), (0, c.rect[2]), (1, c.rect[3])] {
                let along = 1 - fixed;
                let across = (p[fixed] - v).abs();
                let (lo, hi) = (c.rect[along], c.rect[along + 2]);
                let past = (lo - p[along]).max(p[along] - hi).max(0.0);
                if across >= c.band || past >= c.band {
                    continue;
                }
                let a = p[along].clamp(lo, hi);
                let i = ((a - lo) / c.period).floor().min(((hi - lo) / c.period).round() - 1.0).max(0.0);
                let (ka, kb) = (lo + i * c.period, lo + (i + 1.0) * c.period);
                let on = |u: f64| {
                    let mut q = [0.0; 2];
                    q[fixed] = v;
                    q[along] = u;
                    q
                };
                let (ha, hb) = (f(on(ka)), f(on(kb)));
                let line = ha + (hb - ha) * (a - ka) / c.period;
                out += fall(across / c.band) * fall(past / c.band) * (line - f(on(a)));
            }
        }
        out
    }

    /// The closed-form blend at a world point (no collar pull).
    fn blend(&self, tiles: &[Tile], terrains: &[Terrain], cands: &[usize], x: f64, z: f64) -> Point {
        let step = self.spacing;
        let mut w = vec![0.0; cands.len()];
        for (i, &c) in cands.iter().enumerate() {
            let src = &self.sources[c];
            if src.contains(x, z) {
                let t = src.tile;
                let (fx, fz) = ((x - src.at[0]) / src.spacing, (z - src.at[1]) / src.spacing);
                let (h, surface) = tiles[t].ground_in(&terrains[t], src.keep, fx, fz).unwrap_or((0.0, 0));
                w[i] = 1.0;
                return Point { h, surface, w, near: 0.0 };
            }
        }
        // The distances first (cheap), then a tile's extension only where its weight is not 0: the
        // nearest, and the ones within their radius. Pruned or not, the sums see the same terms in the
        // same order (a weight of 0 adds nothing), so the ground is the same to the bit.
        let ds: Vec<f64> = cands.iter().map(|&c| self.sources[c].extend_distance(x, z)).collect();
        let ni = (0..ds.len()).min_by(|a, b| ds[*a].total_cmp(&ds[*b])).expect("a point has a candidate");
        let say = |i: usize| !self.prune || i == ni || ds[i] - step < self.radii[cands[i]];
        let reach: Vec<Option<crate::Reach>> = (0..cands.len())
            .map(|i| {
                say(i).then(|| {
                    let r = self.sources[cands[i]].extend(x, z, step, self.fade);
                    debug_assert_eq!(r.d.to_bits(), ds[i].to_bits(), "extend_distance is extend's d");
                    r
                })
            })
            .collect();
        let nearest = reach[ni].expect("the nearest is always extended");
        if nearest.d <= step * (1.0 + 1e-9) {
            w[ni] = 1.0;
            return Point { h: nearest.h, surface: nearest.surface, w, near: nearest.d };
        }
        let mut sum = 0.0;
        for (i, r) in reach.iter().enumerate() {
            if let Some(r) = r {
                w[i] = shepard(r.d - step, self.radii[cands[i]]);
                sum += w[i];
            }
        }
        if sum == 0.0 {
            w[ni] = 1.0;
            sum = 1.0;
        }
        let mut h = 0.0;
        for (i, r) in reach.iter().enumerate() {
            if let Some(r) = r {
                w[i] /= sum;
                h += w[i] * r.h;
            }
        }
        Point { h, surface: nearest.surface, w, near: nearest.d }
    }

    /// How far a point is past every seam gate (C9): the least, over the sources, of its distance to
    /// the interior less that source's gate there. The rideable gate holds a cell to `max_slope` where
    /// this is above 0 at its corners. `srcs` are the sources that can be nearer than their gate.
    pub fn clear(&self, srcs: &[usize], x: f64, z: f64) -> f64 {
        srcs.iter().map(|&c| self.sources[c].distance(x, z) - self.sources[c].gate_at(x, z)).fold(f64::INFINITY, f64::min)
    }

    /// The sources whose seam gate reaches into a world rectangle grown by `margin`.
    pub fn gated(&self, rect: [f64; 4], margin: f64) -> Vec<usize> {
        (0..self.sources.len())
            .filter(|&c| {
                let g = self.sources[c].gates.iter().copied().fold(0.0, f64::max);
                rect_gap(rect, self.sources[c].rect) < g + margin
            })
            .collect()
    }

    /// The sources within reach of a world rectangle (and one lattice step round it): from the spatial
    /// grid, each within its own radius; every source when pruning is off.
    pub fn candidates(&self, rect: [f64; 4]) -> Vec<usize> {
        if !self.prune {
            return (0..self.sources.len()).collect();
        }
        let step = self.spacing;
        self.bins
            .over([rect[0] - step, rect[1] - step, rect[2] + step, rect[3] + step])
            .into_iter()
            .filter(|&c| rect_gap(rect, self.sources[c].rect) <= self.radii[c] + 2.0 * step)
            .collect()
    }

    /// Join pieces, in the order mod.json lists them (which is precedence, §3): first each collar's,
    /// `c<tile>_<x>_<z>` at its tile's spacing, then the lattice's, `j<x>_<z>`, each axis cut by
    /// [`cuts`] (whole leaves, a leaf of overlap, the last piece ending on the collar's or the world's
    /// edge). A chunk wholly inside one interior is left out, and so is a lattice chunk wholly inside a
    /// collar (the collar covers it).
    pub fn pieces(&self, chunk: u32) -> Vec<Spec> {
        let mut out = Vec::new();
        for c in &self.collars {
            let s = c.spacing;
            let n = [((c.rect[2] - c.rect[0]) / s).round() as u32, ((c.rect[3] - c.rect[1]) / s).round() as u32];
            let src = &self.sources[c.tile];
            let whole = |a: usize| move |o: u32| (c.rect[a] + f64::from(o) * s).fract() == 0.0;
            for (jz, (z0, cz)) in cuts(0, n[1], chunk, End::Back, &whole(1)).into_iter().enumerate() {
                for (jx, (x0, cx)) in cuts(0, n[0], chunk, End::Back, &whole(0)).into_iter().enumerate() {
                    let spec = Spec {
                        name: format!("c{:02}_{jx:03}_{jz:03}", c.tile),
                        at: [c.rect[0] + f64::from(x0) * s, c.rect[1] + f64::from(z0) * s],
                        spacing: s,
                        cells: [cx, cz],
                    };
                    let r = spec.rect();
                    if !(src.contains(r[0], r[1]) && src.contains(r[2], r[3])) {
                        out.push(spec);
                    }
                }
            }
        }
        let step = self.spacing;
        let within = |r: [f64; 4], q: [f64; 4]| r[0] >= q[0] - 1e-6 && r[1] >= q[1] - 1e-6 && r[2] <= q[2] + 1e-6 && r[3] <= q[3] + 1e-6;
        let whole = |o: u32| (f64::from(o) * step).fract() == 0.0;
        for (jz, (z0, cz)) in cuts(0, self.size[1], chunk, End::Back, &whole).into_iter().enumerate() {
            for (jx, (x0, cx)) in cuts(0, self.size[0], chunk, End::Back, &whole).into_iter().enumerate() {
                let spec = Spec {
                    name: format!("j{jx:03}_{jz:03}"),
                    at: [f64::from(x0) * step, f64::from(z0) * step],
                    spacing: step,
                    cells: [cx, cz],
                };
                let r = spec.rect();
                let inside = self.sources.iter().any(|s| s.contains(r[0], r[1]) && s.contains(r[2], r[3]))
                    || self.collars.iter().any(|c| within(r, c.rect));
                if !inside {
                    out.push(spec);
                }
            }
        }
        out
    }

    /// The steepest join ground on a coarse lattice (every `stride` samples), away from the seams.
    pub fn coarse_slope(&self, tiles: &[Tile], stride: u32) -> (f64, [f64; 2]) {
        let all: Vec<usize> = (0..self.sources.len()).collect();
        let near = |x: f64, z: f64| if self.prune { self.bins.at(x, z) } else { &all[..] };
        let step = self.spacing * f64::from(stride);
        let (nx, nz) = (self.size[0] / stride, self.size[1] / stride);
        let row = (nx + 1) as usize;
        // Rows on threads; each point keeps only its height and distance (not its weights), so a world
        // of tens of maps fits in memory however often the gaps are widened.
        let mut h = vec![Coarse { h: 0.0, clear: 0.0 }; row * (nz as usize + 1)];
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let rows_each = (nz as usize + 1).div_ceil(threads).max(1);
        std::thread::scope(|sc| {
            for (part, out) in h.chunks_mut(rows_each * row).enumerate() {
                let near = &near;
                sc.spawn(move || {
                    let terrains: Vec<Terrain> = tiles.iter().map(Tile::terrain).collect();
                    let z0 = (part * rows_each) as f64 * step;
                    let gated = self.gated([0.0, z0, nx as f64 * step, z0 + rows_each as f64 * step], step);
                    for (i, o) in out.iter_mut().enumerate() {
                        let (kx, kz) = (i % row, part * rows_each + i / row);
                        let (x, z) = (kx as f64 * step, kz as f64 * step);
                        let pt = self.at(tiles, &terrains, near(x, z), x, z);
                        *o = Coarse { h: pt.h, clear: self.clear(&gated, x, z) };
                    }
                });
            }
        });
        let mut worst = (0.0, [0.0; 2]);
        for kz in 0..nz as usize {
            for kx in 0..nx as usize {
                let c = [&h[kx + kz * row], &h[kx + 1 + kz * row], &h[kx + (kz + 1) * row], &h[kx + 1 + (kz + 1) * row]];
                if c.iter().any(|p| p.clear < step) {
                    continue;
                }
                let g = cell_slope([c[0].h, c[1].h, c[2].h, c[3].h], step);
                if g > worst.0 {
                    worst = (g, [kx as f64 * step, kz as f64 * step]);
                }
            }
        }
        worst
    }
}

/// A coarse lattice point: the ground's height and how far it is past every seam gate.
#[derive(Clone, Copy)]
struct Coarse {
    h: f64,
    clear: f64,
}

/// The slope of a cell's blend at its middle, from its four corners (sample, +x, +z, both).
pub fn cell_slope(h: [f64; 4], step: f64) -> f64 {
    let gx = (h[1] - h[0] + h[3] - h[2]) / (2.0 * step);
    let gz = (h[2] - h[0] + h[3] - h[1]) / (2.0 * step);
    gx.hypot(gz)
}

/// One join piece computed.
pub struct Piece {
    pub name: String,
    pub at: [f64; 2],
    pub samples: [u32; 2],
    pub between: Vec<usize>,
    pub trn: Vec<u8>,
    pub blend: Vec<u8>,
    /// Steepest cell away from the seams, and anywhere.
    pub far_slope: f64,
    pub near_slope: f64,
    pub clamped: usize,
}

/// Turn shares into bytes summing to 255, by largest remainder.
pub fn bytes_255(w: &[f64]) -> Vec<u8> {
    let sum: f64 = w.iter().sum();
    let exact: Vec<f64> = w.iter().map(|x| if sum > 0.0 { x / sum * 255.0 } else { 0.0 }).collect();
    let mut out: Vec<u32> = exact.iter().map(|x| x.floor() as u32).collect();
    let mut left = 255 - out.iter().sum::<u32>().min(255);
    let mut idx: Vec<usize> = (0..w.len()).collect();
    idx.sort_by(|a, b| (exact[*b] - exact[*b].floor()).total_cmp(&(exact[*a] - exact[*a].floor())).then(a.cmp(b)));
    for &i in idx.iter().cycle().take(w.len() * 255) {
        if left == 0 {
            break;
        }
        out[i] += 1;
        left -= 1;
    }
    out.into_iter().map(|x| x as u8).collect()
}

fn piece(plan: &Plan, tiles: &[Tile], terrains: &[Terrain], spec: &Spec) -> Result<Piece, String> {
    let name = &spec.name;
    let step = spec.spacing;
    let rect = spec.rect();
    let cands = plan.candidates(rect);
    let (sw, sd) = (spec.cells[0] as usize + 1, spec.cells[1] as usize + 1);
    // One sample of margin all round, for the normals.
    let (mw, md) = (sw + 2, sd + 2);
    let mut h = vec![0.0; mw * md];
    let mut near = vec![0.0; sw * sd];
    let mut clear = vec![0.0; sw * sd];
    let gated = plan.gated(rect, step);
    let mut surfaces = vec![0u8; sw * sd];
    let mut shares = vec![0.0; sw * sd * cands.len()];
    for mz in 0..md {
        for mx in 0..mw {
            let (kx, kz) = (mx as f64 - 1.0, mz as f64 - 1.0);
            let pt = plan.at(tiles, terrains, &cands, spec.at[0] + kx * step, spec.at[1] + kz * step);
            h[mx + mz * mw] = pt.h;
            if (1..=sw).contains(&mx) && (1..=sd).contains(&mz) {
                let i = (mx - 1) + (mz - 1) * sw;
                near[i] = pt.near;
                clear[i] = plan.clear(&gated, spec.at[0] + kx * step, spec.at[1] + kz * step);
                surfaces[i] = pt.surface & 0x7f;
                shares[i * cands.len()..(i + 1) * cands.len()].copy_from_slice(&pt.w);
            }
        }
    }
    let clamped = h.iter().filter(|v| **v < 0.0).count();
    let feet: Vec<f32> = (0..sd).flat_map(|z| (0..sw).map(move |x| (x, z))).map(|(x, z)| h[x + 1 + (z + 1) * mw].max(0.0) as f32).collect();
    let (mut grid, hs) = trnwrite::Grid::from_feet(sw as u32, sd as u32, &feet, step as f32)?;
    grid.surfaces = surfaces;
    // Normals from the ground itself, across the piece's edges too (trnwrite's formula, on feet).
    grid.normals = (0..sd)
        .flat_map(|z| (0..sw).map(move |x| (x, z)))
        .map(|(x, z)| {
            let at = |dx: i64, dz: i64| h[(x as i64 + 1 + dx) as usize + (z as i64 + 1 + dz) as usize * mw];
            let gx = (at(1, 0) - at(-1, 0)) / (2.0 * step);
            let gz = (at(0, 1) - at(0, -1)) / (2.0 * step);
            let len = (gx * gx + 1.0 + gz * gz).sqrt();
            [(-gx / len * 127.0) as i8, (1.0 / len * 127.0) as i8, (-gz / len * 127.0) as i8]
        })
        .collect();
    let trn = trnwrite::build(&grid, &trnwrite::Options::new(step as f32, hs))?;
    let (bytes, _) = trnwrite::write(&trn)?;
    // Slopes, away from the seams (past every seam gate) and anywhere.
    let (mut far_slope, mut near_slope) = (0.0f64, 0.0f64);
    for z in 0..sd - 1 {
        for x in 0..sw - 1 {
            let c = [x + z * sw, x + 1 + z * sw, x + (z + 1) * sw, x + 1 + (z + 1) * sw];
            if c.iter().any(|&i| near[i] == 0.0) {
                continue; // a tile's own cell
            }
            let hh = c.map(|i| h[(i % sw) + 1 + (i / sw + 1) * mw]);
            let g = cell_slope(hh, step);
            if c.iter().all(|&i| clear[i] > 0.0) {
                far_slope = far_slope.max(g);
            } else {
                near_slope = near_slope.max(g);
            }
        }
    }
    // The look: the tiles with a share anywhere in the piece, in the world's tile order.
    let n = cands.len();
    let mut used: Vec<usize> = (0..n).filter(|&c| (0..sw * sd).any(|i| shares[i * n + c] * 255.0 >= 0.5)).collect();
    used.sort_by_key(|&c| plan.sources[cands[c]].tile);
    let mut between: Vec<usize> = used.iter().map(|&c| plan.sources[cands[c]].tile).collect();
    let mut blend = Vec::with_capacity(sw * sd * between.len().max(2));
    for i in 0..sw * sd {
        let w: Vec<f64> = used.iter().map(|&c| shares[i * n + c]).collect();
        blend.extend(bytes_255(&w));
    }
    if between.len() < 2 {
        // modcheck wants two or more: the nearest other tile, at 0 everywhere.
        let other = plan
            .sources
            .iter()
            .filter(|s| !between.contains(&s.tile))
            .min_by(|a, b| a.distance(rect[0], rect[1]).total_cmp(&b.distance(rect[0], rect[1])))
            .map(|s| s.tile)
            .ok_or("one tile only")?;
        let mut padded = Vec::with_capacity(sw * sd * 2);
        let pos = between.iter().position(|&t| t > other).unwrap_or(between.len());
        for c in blend.chunks(between.len().max(1)) {
            let mut v = c.to_vec();
            v.insert(pos, 0);
            padded.extend(v);
        }
        between.insert(pos, other);
        blend = padded;
    }
    Ok(Piece {
        name: name.clone(),
        at: [rect[0], rect[1]],
        samples: [sw as u32, sd as u32],
        between,
        trn: bytes,
        blend,
        far_slope,
        near_slope,
        clamped,
    })
}

/// Compute every piece, on threads; `f` gets each one as it is done (in no order).
pub fn solve(plan: &Plan, tiles: &[Tile], chunk: u32, threads: usize, f: &(dyn Fn(Piece) -> Result<(), String> + Sync)) -> Result<usize, String> {
    let specs = plan.pieces(chunk);
    let next = AtomicUsize::new(0);
    let err: Mutex<Option<String>> = Mutex::new(None);
    let threads = if threads == 0 { std::thread::available_parallelism().map_or(4, |n| n.get()) } else { threads };
    std::thread::scope(|sc| {
        for _ in 0..threads.min(specs.len().max(1)) {
            sc.spawn(|| {
                let terrains: Vec<Terrain> = tiles.iter().map(Tile::terrain).collect();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= specs.len() || err.lock().expect("lock").is_some() {
                        break;
                    }
                    if let Err(e) = piece(plan, tiles, &terrains, &specs[i]).and_then(f) {
                        *err.lock().expect("lock") = Some(e);
                    }
                }
            });
        }
    });
    match err.into_inner().expect("lock") {
        Some(e) => Err(e),
        None => Ok(specs.len()),
    }
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), String> {
    for rel in crate::files(from)? {
        let dst = to.join(&rel);
        if let Some(p) = dst.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        std::fs::copy(from.join(&rel), &dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    }
    Ok(())
}

/// One piece written, for mod.json.
struct Done {
    name: String,
    at: [f64; 2],
    samples: [u32; 2],
    between: Vec<usize>,
    far: f64,
    near: f64,
    clamped: usize,
}

/// What a build came to.
pub struct Built {
    pub dir: PathBuf,
    pub pieces: usize,
    pub drops: usize,
    pub far_slope: f64,
    pub near_slope: f64,
    pub plan: Plan,
}

/// Build the world `p.id` from the tile folders into `out/<id>/`, replacing a world already there.
pub fn build(tile_dirs: &[PathBuf], out: &Path, p: &Params) -> Result<Built, String> {
    let tiles: Vec<Tile> = tile_dirs.iter().map(|d| Tile::load(d)).collect::<Result<_, _>>()?;
    build_tiles(&tiles, out, p, true)
}

/// [`build`] from tiles already read ([`Tile::load`] or [`Tile::from_parts`]). With `copy_tiles` off
/// no `tiles/` folder is written (P232: the shell's disc worlds, whose tiles stay on the disc): the
/// world is its mod.json and its joins, and every join byte is the same either way.
pub fn build_tiles(tiles: &[Tile], out: &Path, p: &Params, copy_tiles: bool) -> Result<Built, String> {
    let mut seen = std::collections::HashSet::new();
    for t in tiles {
        if !seen.insert(t.id.clone()) {
            return Err(format!("{} is given twice", t.id));
        }
    }
    let (cuts, ridged): (Vec<[u32; 4]>, Vec<[bool; 4]>) = tiles.iter().map(Tile::find_cut).unzip();
    for (t, k) in tiles.iter().zip(&cuts) {
        let ring = t.ring(*k);
        let steep = ring.edges.iter().flatten().map(|e| e.gx.hypot(e.gz)).fold(0.0, f64::max);
        eprintln!(
            "stitch: {}: {}×{} samples at {} ft, keep {:?}, edge height {:.1} ft (mean), steepest edge cell {:.3}",
            t.id, t.n.0, t.n.1, t.spacing, k, ring.mean(), steep
        );
    }
    // The seam gates (C9), once: they hang on each tile's edge alone, not on the layout.
    let t0 = std::time::Instant::now();
    let step = lattice_spacing(&tiles);
    let base = 2.0 * step + 3.0 * p.fade;
    let gates: Vec<[f64; 4]> = tiles.iter().zip(&cuts).map(|(t, k)| cut_source(t, *k, step, p).gates).collect();
    for (t, g) in tiles.iter().zip(&gates) {
        if g.iter().any(|v| *v > base) {
            eprintln!("stitch: {}: seam gates {:?} ft (C1's {base:.0}): its edges stay steep that far out", t.id, g.map(|v| v.round()));
        }
    }
    eprintln!("stitch: seam gates measured in {:.1} s", t0.elapsed().as_secs_f64());
    let dir = out.join(&p.id);
    if dir.exists() {
        let old = std::fs::read_to_string(dir.join("mod.json")).unwrap_or_default();
        if !old.contains("\"world\"") {
            return Err(format!("{} exists and is not a world mod: not replacing it", dir.display()));
        }
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let joins = dir.join("joins");
    let mut widen = 1.0;
    let (plan, results) = loop {
        let t0 = std::time::Instant::now();
        let plan = plan(&tiles, &cuts, &gates, p, widen)?;
        let (rmin, rmax) = plan.radii.iter().fold((f64::INFINITY, 0.0f64), |(a, b), r| (a.min(*r), b.max(*r)));
        eprintln!(
            "stitch: widen {widen:.3}: world {:.0} × {:.0} ft, {} pieces, radius {rmin:.0}–{rmax:.0} ft, planned in {:.1} s",
            f64::from(plan.size[0]) * plan.spacing,
            f64::from(plan.size[1]) * plan.spacing,
            plan.pieces(p.chunk).len(),
            t0.elapsed().as_secs_f64()
        );
        let t0 = std::time::Instant::now();
        let (coarse, at) = plan.coarse_slope(&tiles, 4);
        eprintln!("stitch: coarse pass {coarse:.3} steepest, {:.1} s", t0.elapsed().as_secs_f64());
        if coarse > p.max_slope {
            let (d, near) = plan
                .sources
                .iter()
                .map(|s| (s.distance(at[0], at[1]), s.tile))
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .expect("two tiles");
            eprintln!(
                "stitch: {:.3} at ({:.0}, {:.0}) on the coarse lattice, {:.0} ft from {}'s interior, past {}: widening the gaps",
                coarse, at[0], at[1], d, tiles[near].id, p.max_slope
            );
        } else {
            if joins.exists() {
                std::fs::remove_dir_all(&joins).map_err(|e| format!("{}: {e}", joins.display()))?;
            }
            let results: Mutex<Vec<Done>> = Mutex::new(Vec::new());
            let t0 = std::time::Instant::now();
            solve(&plan, &tiles, p.chunk, p.threads, &|pc: Piece| {
                let jd = joins.join(&pc.name);
                std::fs::create_dir_all(&jd).map_err(|e| format!("{}: {e}", jd.display()))?;
                std::fs::write(jd.join(format!("{}.ps2.trn", pc.name)), &pc.trn).map_err(|e| e.to_string())?;
                std::fs::write(jd.join(format!("{}.blend", pc.name)), &pc.blend).map_err(|e| e.to_string())?;
                results.lock().expect("lock").push(Done { name: pc.name, at: pc.at, samples: pc.samples, between: pc.between, far: pc.far_slope, near: pc.near_slope, clamped: pc.clamped });
                Ok(())
            })?;
            let mut results = results.into_inner().expect("lock");
            results.sort_by(|a, b| a.name.cmp(&b.name));
            eprintln!("stitch: {} pieces solved and written in {:.1} s", results.len(), t0.elapsed().as_secs_f64());
            let far = results.iter().map(|r| r.far).fold(0.0, f64::max);
            if far <= p.max_slope {
                break (plan, results);
            }
            let worst = results.iter().max_by(|a, b| a.far.total_cmp(&b.far)).expect("a piece");
            let near = plan
                .sources
                .iter()
                .min_by(|a, b| a.distance(worst.at[0], worst.at[1]).total_cmp(&b.distance(worst.at[0], worst.at[1])))
                .map(|s| tiles[s.tile].id.as_str())
                .unwrap_or("");
            eprintln!(
                "stitch: join ground {far:.3} steep away from the seams in {} (at {:.0}, {:.0}; nearest {near}), past {}: widening the gaps",
                worst.name, worst.at[0], worst.at[1], p.max_slope
            );
        }
        widen *= 1.25;
        if widen > 20.0 {
            return Err("the join ground stays too steep however wide the gaps".into());
        }
    };
    if copy_tiles {
        for t in tiles {
            copy_dir(&t.dir, &dir.join("tiles").join(&t.id))?;
        }
    }

    // Placements in the cut band.
    let mut drops = Vec::new();
    for (i, t) in tiles.iter().enumerate() {
        let k = plan.placed[i].keep;
        for (section, pos) in t.placements()? {
            let (fx, fz) = (pos[0] / t.spacing, pos[2] / t.spacing);
            let inside = fx >= f64::from(k[0]) && fx < f64::from(k[2]) && fz >= f64::from(k[1]) && fz < f64::from(k[3]);
            if !inside {
                let off = fx < 0.0 || fz < 0.0 || fx > f64::from(t.n.0) || fz > f64::from(t.n.1);
                let g = t.ground.map(f64::from);
                let hole = fx < g[0] || fz < g[1] || fx > g[2] || fz > g[3];
                let why = if off {
                    format!("off the terrain at ({:.1}, {:.1}) ft", pos[0], pos[2])
                } else if hole {
                    format!("over the terrain's missing cells at ({:.1}, {:.1}) ft, ground {:?}", pos[0], pos[2], t.ground)
                } else {
                    // The side it is past: a ring cut at its ridge (M23), or the berm.
                    let past = [fx < f64::from(k[0]), fz < f64::from(k[1]), fx >= f64::from(k[2]), fz >= f64::from(k[3])];
                    let band = if (0..4).any(|e| past[e] && ridged[i][e]) { "the ring past the ridge it is cut at" } else { "the cut berm band" };
                    format!("in {band} at ({:.1}, {:.1}) ft, keep {:?}", pos[0], pos[2], k)
                };
                drops.push(o(vec![("tile", s(&t.id)), ("section", s(section)), ("why", s(why))]));
            }
        }
    }
    let sky = match &p.sky {
        Some(sk) if tiles.iter().any(|t| &t.id == sk) => sk.clone(),
        Some(sk) => return Err(format!("sky {sk} is not one of the tiles")),
        None => tiles[0].id.clone(),
    };
    let spawn_tile = tiles
        .iter()
        .enumerate()
        .find(|(i, t)| {
            let k = plan.placed[*i].keep;
            let sp = t.spawn();
            let (fx, fz) = (sp[0] / t.spacing, sp[2] / t.spacing);
            fx >= f64::from(k[0]) && fx < f64::from(k[2]) && fz >= f64::from(k[1]) && fz < f64::from(k[3])
        })
        .map(|(_, t)| t.id.clone())
        .ok_or("no tile's spawn is inside its interior")?;
    // A wrapped world starts on its middle tile, as far from the seams as it can (openmx MD50).
    let spawn_tile = match &p.wrap {
        Some(_) => {
            let each: Vec<_> = tiles.iter().zip(&plan.placed).map(|(t, pl)| (pl.at, pl.keep, t.spacing, t.spawn())).collect();
            let extent = plan.size.map(|n| f64::from(n) * plan.spacing);
            crate::middle_tile(&each, extent).map_or(spawn_tile, |i| tiles[i].id.clone())
        }
        None => spawn_tile,
    };

    let mut games: Vec<String> = Vec::new();
    for t in tiles {
        let g = t.game();
        if !games.contains(&g) {
            games.push(g);
        }
    }
    let perms: Vec<String> = tiles.iter().map(|t| t.meta.get("permission").and_then(|v| v.str()).unwrap_or("").to_owned()).collect();
    let permission = if perms.iter().all(|x| *x == perms[0]) { perms[0].clone() } else { "own-disc".into() };

    let mut todo: Vec<J> = vec![s(
        "joins/*: generated ground (tools/stitch), no source game's: a closed-form blend of the tiles' edges; \
         no texture pages, the look is each <join>.blend over the tiles' own ground (openmx MD9)",
    )];
    todo.push(s("joins/*: surface bytes are the nearest tile's id with the top bit 0 (the bit's meaning is not known)"));
    todo.push(s(
        "the world's outer edge is flat join ground with no berm; the shootoff there is the world's (openmx MD8)",
    ));
    todo.extend(p.todo.iter().map(s));
    for t in tiles {
        let g = t.ground;
        if g != [0, 0, t.n.0, t.n.1] {
            todo.push(s(format!(
                "{}: its terrain has no cells outside samples {}..{} × {}..{} of its {}×{} (as on its disc): its \
                 interior is cut inside the cells that are there, so the ground past the hole is not in the world",
                t.id, g[0], g[2], g[1], g[3], t.n.0, t.n.1
            )));
        }
    }
    for (i, t) in tiles.iter().enumerate() {
        if ridged[i].iter().any(|r| *r) {
            let sides: Vec<&str> = ["x0", "z0", "x1", "z1"].iter().zip(&ridged[i]).filter(|(_, r)| **r).map(|(n, _)| *n).collect();
            todo.push(s(format!(
                "{}: cut at its ring's ridge on {} (M23), keep {:?}: the mountains' outer slopes and the flat shootoff \
                 plateau past them are not in the world, and the tile's neighbours meet it ridge to ridge over a join",
                t.id,
                sides.join(" "),
                plan.placed[i].keep
            )));
        }
        let rock = t.rock_cut();
        if rock.iter().any(|r| *r > 0) {
            let sides: Vec<String> =
                ["x0", "z0", "x1", "z1"].iter().zip(rock).filter(|(_, r)| *r > 0).map(|(n, r)| format!("{n} by {:.0} ft", f64::from(r) * t.spacing)).collect();
            todo.push(s(format!(
                "{}: cut past a rock face (ground steeper than {} ft a foot across the berm's cut line, openmx MD53) on {}, \
                 keep {:?}: the ground between, the map's own, is not in the world, and a join stands in its place",
                t.id,
                crate::ROCK,
                sides.join(" "),
                plan.placed[i].keep
            )));
        }
    }
    for c in &plan.collars {
        todo.push(s(format!(
            "{}: {} ft samples against the lattice's {} ft: its joins (c{:02}_*) are at its own spacing for {} ft round \
             its interior, and meet the lattice's along lines the two share, pulled straight between their shared \
             samples every {} ft (a generated crease of at most the field's curvature over that length)",
            tiles[c.tile].id, c.spacing, plan.spacing, c.tile, c.width, c.period
        )));
    }
    for (i, why) in &plan.uncollared {
        let t = &tiles[*i];
        todo.push(s(match plan.ratio[*i] {
            Some((pp, q)) => format!(
                "{}: {} ft samples against the joins' {} ft ({pp}:{q}), and {why}: the seam meets the join lattice exactly \
                 only at the lattice points; between them the two linear pieces differ by the tile's curvature",
                t.id, t.spacing, plan.spacing
            ),
            None => format!(
                "{}: {} ft samples have no small ratio to the joins' {} ft: its seams are not on the join lattice and \
                 meet the join only approximately",
                t.id, t.spacing, plan.spacing
            ),
        }));
    }
    let base = 2.0 * plan.spacing + 3.0 * plan.fade;
    let gated: Vec<&Source> = plan.sources.iter().filter(|g| g.gates.iter().any(|v| *v > base)).collect();
    for g in &gated {
        let far = g.gates.iter().copied().fold(0.0, f64::max);
        todo.push(s(format!(
            "{}: its edges' own relief (hills its cut runs through) keeps the join ground beside it steeper than \
             max_slope {} for up to {far:.0} ft out (seam gates x0 z0 x1 z1: {}); the joins are held to max_slope \
             only past them (C9)",
            tiles[g.tile].id,
            p.max_slope,
            g.gates.map(|v| format!("{v:.0}")).join(" ")
        )));
    }
    let clamped: usize = results.iter().map(|r| r.clamped).sum();
    if clamped > 0 {
        todo.push(s(format!("joins: {clamped} samples of the blend fell below 0 ft and were raised to 0 (a raw height is unsigned)")));
    }
    let far_slope = results.iter().map(|r| r.far).fold(0.0, f64::max);
    let near_slope = results.iter().map(|r| r.near).fold(0.0, f64::max);

    let tiles_j: Vec<J> = tiles
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let pl = &plan.placed[i];
            o(vec![
                ("tile", s(&t.id)),
                ("at", J::A(vec![J::N(pl.at[0]), J::N(pl.at[1])])),
                ("keep", J::A(pl.keep.iter().map(|k| J::N(f64::from(*k))).collect())),
            ])
        })
        .collect();
    let joins_j: Vec<J> = results
        .iter()
        .map(|r| {
            o(vec![
                ("join", s(&r.name)),
                ("at", J::A(vec![J::N(r.at[0]), J::N(r.at[1])])),
                ("between", J::A(r.between.iter().map(|&t| s(&tiles[t].id)).collect())),
                // Not a §3 field: the piece's samples a side, for stitch check.
                ("samples", J::A(vec![J::N(f64::from(r.samples[0])), J::N(f64::from(r.samples[1]))])),
            ])
        })
        .collect();
    let drops_n = drops.len();
    let mut meta = o(vec![
        ("standard", J::N(1.0)),
        ("kind", s("world")),
        ("id", s(&p.id)),
        ("name", s(&p.name)),
        ("source", o(vec![("game", s("Combined")), ("games", J::A(games.into_iter().map(J::S).collect()))])),
        ("version", s(&p.version)),
        ("author", s(AUTHOR)),
        ("permission", s(permission)),
        ("tiles", J::A(tiles_j)),
        ("joins", J::A(joins_j)),
        ("drop", J::A(drops)),
        ("sky", s(sky)),
        ("spawn", o(vec![("tile", s(spawn_tile))])),
        ("todo", J::A(todo)),
        (
            "stitch",
            o(vec![
                ("spacing", J::N(plan.spacing)),
                ("max_slope", J::N(p.max_slope)),
                ("fade", J::N(plan.fade)),
                ("radius", J::N(plan.radius)),
                ("radius_min", J::N(plan.radii.iter().copied().fold(f64::INFINITY, f64::min))),
                ("widen", J::N(plan.widen)),
                (
                    "collars",
                    J::A(
                        plan.collars
                            .iter()
                            .map(|c| {
                                o(vec![
                                    ("tile", s(&tiles[c.tile].id)),
                                    ("spacing", J::N(c.spacing)),
                                    ("rect", J::A(c.rect.iter().map(|v| J::N(*v)).collect())),
                                    ("period", J::N(c.period)),
                                    ("band", J::N(c.band)),
                                ])
                            })
                            .collect(),
                    ),
                ),
                // Per tile, the seam gates past C1's `2 lattice + 3 fade` (C9), for stitch check.
                (
                    "gates",
                    J::A(
                        gated
                            .iter()
                            .map(|g| o(vec![("tile", s(&tiles[g.tile].id)), ("edges", J::A(g.gates.iter().map(|v| J::N(*v)).collect()))]))
                            .collect(),
                    ),
                ),
                ("far_slope", J::N((far_slope * 1e4).round() / 1e4)),
                ("near_slope", J::N((near_slope * 1e4).round() / 1e4)),
            ]),
        ),
    ]);
    // The world's wrapped axes (openmx MD29), after `spawn`; a world with edges has no `wrap`.
    if let (Some(w), J::O(kv)) = (&p.wrap, &mut meta) {
        let at = kv.iter().position(|(k, _)| k == "spawn").map_or(kv.len(), |i| i + 1);
        kv.insert(at, ("wrap".into(), s(w)));
    }
    std::fs::write(dir.join("mod.json"), meta.pretty()).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(Built { dir, pieces: results.len(), drops: drops_n, far_slope, near_slope, plan })
}
