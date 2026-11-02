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
    /// Tiles laid out in the grid's middle cells, whatever their height ([`plan`]; openmx P253): on a
    /// wrapped world the ride starts there (MD50). Empty, the height order alone lays the grid out.
    pub middle: Vec<String>,
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
            middle: Vec::new(),
            threads: 0,
            prune: true,
        }
    }
}

impl Params {
    /// A world spec's build parameters (`worlds/<id>.json`: `id`, `name`, and optional `sky`, `wrap`,
    /// `max_slope`, `min_gap`, `fade`, `chunk`, `todo`, `middle`), as openmx-mods' `stitch --mods --out` reads
    /// them; its `tiles` are the caller's to find.
    pub fn from_spec(j: &Json) -> Result<Params, String> {
        let g = |k: &str| j.get(k).and_then(Json::str).map(str::to_owned);
        let mut p = Params::new(&g("id").ok_or("no id")?, &g("name").ok_or("no name")?);
        p.sky = g("sky");
        p.wrap = g("wrap");
        p.todo = j.get("todo").and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::str).map(str::to_owned).collect();
        p.middle = j.get("middle").and_then(Json::arr).unwrap_or_default().iter().filter_map(Json::str).map(str::to_owned).collect();
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
    /// Each source's relief for far join ground (MD57), or none: the blend alone.
    pub relief: Vec<Relief>,
    /// What every relief is carried at on top of its own gain (the build lowers it where the join
    /// ground would be steeper than `max_slope` with it).
    pub relief_scale: f64,
    /// How much of the relief the blend has room for, over the world (none: all of it).
    pub room: Option<Room>,
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
/// so neighbours are close in height, with [`Params::middle`]'s tiles in the cells nearest the grid's
/// middle; each gap is as wide as its two facing edges' heights need.
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
    let snake: Vec<(usize, usize)> = (0..tiles.len())
        .map(|i| {
            let (r, c) = (i / cols, i % cols);
            (r, if r % 2 == 1 { cols - 1 - c } else { c })
        })
        .collect();
    // The spec's `middle` tiles (P253) take the snake's cells nearest the grid's middle, in height
    // order among themselves; the rest fill the other cells in the snake's order.
    let middle: Vec<usize> = p
        .middle
        .iter()
        .map(|m| tiles.iter().position(|t| &t.id == m).ok_or_else(|| format!("middle {m} is not one of the tiles")))
        .collect::<Result<_, _>>()?;
    let mut near: Vec<usize> = (0..snake.len()).collect();
    let mid = [(rows - 1) as f64 / 2.0, (cols - 1) as f64 / 2.0];
    let d = |i: usize| (snake[i].0 as f64 - mid[0]).powi(2) + (snake[i].1 as f64 - mid[1]).powi(2);
    near.sort_by(|a, b| d(*a).total_cmp(&d(*b)));
    let mut middle_cells: Vec<usize> = near[..middle.len()].to_vec();
    middle_cells.sort();
    let (inner, outer): (Vec<usize>, Vec<usize>) = order.iter().partition(|t| middle.contains(t));
    let outer_cells: Vec<usize> = (0..snake.len()).filter(|i| !middle_cells.contains(i)).collect();
    for (i, t) in middle_cells.into_iter().zip(inner).chain(outer_cells.into_iter().zip(outer)) {
        let (r, c) = snake[i];
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
    Ok(Plan { spacing: step, size, placed, sources, radius, radii, bins, prune: p.prune, fade: p.fade, ratio: ratios, widen, collars, uncollared, relief: Vec::new(), relief_scale: 1.0, room: None })
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

// ---- relief on far join ground (MD57) --------------------------------------------------------------
//
// The blend alone is the maps' edges carried out and averaged: a few hundred feet from every map it is
// smooth ground with no hills. Past the seam gates (C9), far join ground takes the nearest maps' own
// relief, the way the shell paints it with their paint (game-render's `terrain::Regions`, MD48): each
// map's ground read at the point folded back and forth into its keep, smoothed of what no join could
// hold to `max_slope` (jumps, ruts), less its mean, and shared among the nearest maps by distance. It
// rises from nothing at the gates, no higher or deeper than [`RELIEF_RISE`] times its distance past
// them, so every seam and everything near one is the blend's alone.

/// Feet between a [`Relief`] grid's nodes: each node is the mean of the map's ground over its cell.
pub const RELIEF_CELL: f64 = 16.0;

/// How far the relief may stand above or below the blend, feet per foot past the nearest seam gate or
/// the world's edge: it is eased under that (`cap · tanh(r / cap)`), which adds at most a third of
/// this to its slope however high a map's hills are.
pub const RELIEF_RISE: f64 = 0.1;

/// Feet over which [`Room`] keeps the relief off steep blend and fades it back.
pub const RELIEF_RAMP: f64 = 600.0;

/// The radius of a [`Room::dent`], feet.
pub const RELIEF_DENT: f64 = 1200.0;

/// How many dents a build makes before it lowers the relief over the whole world instead.
const RELIEF_DENTS: usize = 60;

/// The steepest a map's smoothed relief may be in a join, feet per foot: a map whose own is steeper is
/// carried at the gain that brings it to this, leaving the rest of `max_slope` to the blend.
pub const RELIEF_SLOPE: f64 = 0.25;

/// A map's weight in the relief `d` feet from its keep: the inverse square. Not the paint's weights
/// (game-render `terrain::reach`), which hand a gap's middle from one map to the other within a couple
/// of hundred feet: fine for a colour, but two maps' hills of opposite sign meeting there would make a
/// cliff of the swap.
fn relief_weight(d: f64) -> f64 {
    1.0 / d.max(1.0).powi(2)
}

/// `v` folded back and forth into `lo..hi` (game-render `terrain::mirror`): continuous, and the map's
/// own ground at every distance however wide the join.
fn fold(v: f64, lo: f64, hi: f64) -> f64 {
    let span = hi - lo;
    if span <= 0.0 {
        return lo;
    }
    let t = (v - lo).rem_euclid(2.0 * span);
    lo + if t > span { 2.0 * span - t } else { t }
}

/// The highest field at or under `v` (a grid of `n` nodes `cell` feet apart) that is nowhere steeper
/// than `s` between neighbouring nodes, straight or diagonal: two chamfer passes.
fn envelope(v: &[f64], n: [usize; 2], cell: [f64; 2], s: f64) -> Vec<f64> {
    let mut u = v.to_vec();
    let (wx, wz, wd) = (s * cell[0], s * cell[1], s * cell[0].hypot(cell[1]));
    let (nx, nz) = (n[0] as isize, n[1] as isize);
    let at = |i: isize, j: isize| (i + j * nx) as usize;
    let mut pass = |order: &mut dyn Iterator<Item = (isize, isize)>, back: isize| {
        for (i, j) in order {
            let mut best = u[at(i, j)];
            for (di, dj, w) in [(-1, 0, wx), (-1, -1, wd), (0, -1, wz), (1, -1, wd)] {
                let (ii, jj) = (i + di * back, j + dj * back);
                if ii >= 0 && ii < nx && jj >= 0 && jj < nz {
                    best = best.min(u[at(ii, jj)] + w);
                }
            }
            u[at(i, j)] = best;
        }
    };
    pass(&mut (0..nz).flat_map(|j| (0..nx).map(move |i| (i, j))), 1);
    pass(&mut (0..nz).rev().flat_map(|j| (0..nx).rev().map(move |i| (i, j))), -1);
    u
}

/// One map's relief for far join ground: its ground over its keep, averaged over [`RELIEF_CELL`] cells
/// and smoothed over two more either way, less its mean, held to [`RELIEF_SLOPE`].
#[derive(Clone, Debug)]
pub struct Relief {
    /// The first node, tile-local feet (the middle of the keep's first cell).
    origin: [f64; 2],
    cell: [f64; 2],
    n: [usize; 2],
    v: Vec<f64>,
    /// The smoothed ground's standard deviation over the keep, feet.
    pub spread: f64,
    /// What of it the relief keeps, held to [`RELIEF_SLOPE`] and times the gain.
    pub kept: f64,
    /// The held field's steepest slope between nodes, before the gain.
    pub slope: f64,
    /// `RELIEF_SLOPE / slope`, at most 1.
    pub gain: f64,
}

impl Relief {
    /// Map `t`'s relief over `keep` (samples, half-open).
    pub fn of(t: &Tile, keep: [u32; 4]) -> Relief {
        let terrain = t.terrain();
        let s = t.spacing;
        let lo = [f64::from(keep[0]) * s, f64::from(keep[1]) * s];
        let width = [f64::from(keep[2] - keep[0]) * s, f64::from(keep[3] - keep[1]) * s];
        let n = width.map(|w| ((w / RELIEF_CELL).ceil() as usize).max(2));
        let cell = [width[0] / n[0] as f64, width[1] / n[1] as f64];
        let k = ((RELIEF_CELL / s).ceil() as usize).max(1);
        let mut v = vec![0.0; n[0] * n[1]];
        for j in 0..n[1] {
            for i in 0..n[0] {
                let mut sum = 0.0;
                for b in 0..k {
                    for a in 0..k {
                        let x = lo[0] + (i as f64 + (a as f64 + 0.5) / k as f64) * cell[0];
                        let z = lo[1] + (j as f64 + (b as f64 + 0.5) / k as f64) * cell[1];
                        sum += t.ground_in(&terrain, keep, x / s, z / s).map_or(0.0, |g| g.0);
                    }
                }
                v[i + j * n[0]] = sum / (k * k) as f64;
            }
        }
        // Two passes of a three-node box each way: [1 2 3 2 1] / 9 across five cells.
        for _ in 0..2 {
            for axis in 0..2 {
                let old = v.clone();
                for j in 0..n[1] {
                    for i in 0..n[0] {
                        let at = |d: isize| {
                            let (mut ii, mut jj) = (i as isize, j as isize);
                            if axis == 0 { ii += d } else { jj += d }
                            let (ii, jj) = (ii.clamp(0, n[0] as isize - 1) as usize, jj.clamp(0, n[1] as isize - 1) as usize);
                            old[ii + jj * n[0]]
                        };
                        v[i + j * n[0]] = (at(-1) + at(0) + at(1)) / 3.0;
                    }
                }
            }
        }
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        v.iter_mut().for_each(|h| *h -= mean);
        let spread = (v.iter().map(|h| h * h).sum::<f64>() / v.len() as f64).sqrt();
        // Steeper than RELIEF_SLOPE nowhere: the mean of the highest field under it and the lowest over
        // it that are no steeper than that (a little less, for the cells' diagonals). Where the map's
        // ground is gentler it is that ground; a peak is cut to a cone and a gully filled to one.
        let s = RELIEF_SLOPE / 1.1;
        let under = envelope(&v, n, cell, s);
        let neg: Vec<f64> = v.iter().map(|h| -h).collect();
        let over = envelope(&neg, n, cell, s);
        v = under.iter().zip(&over).map(|(u, o)| (u - o) / 2.0).collect();
        let mut slope = 0.0f64;
        for j in 0..n[1] - 1 {
            for i in 0..n[0] - 1 {
                let c = [v[i + j * n[0]], v[i + 1 + j * n[0]], v[i + (j + 1) * n[0]], v[i + 1 + (j + 1) * n[0]]];
                let gx = (c[1] - c[0] + c[3] - c[2]) / (2.0 * cell[0]);
                let gz = (c[2] - c[0] + c[3] - c[1]) / (2.0 * cell[1]);
                slope = slope.max(gx.hypot(gz));
            }
        }
        let gain = if slope > RELIEF_SLOPE { RELIEF_SLOPE / slope } else { 1.0 };
        v.iter_mut().for_each(|h| *h *= gain);
        let kept = (v.iter().map(|h| h * h).sum::<f64>() / v.len() as f64).sqrt();
        Relief { origin: [lo[0] + cell[0] / 2.0, lo[1] + cell[1] / 2.0], cell, n, v, spread, kept, slope, gain }
    }

    /// The relief at a tile-local point, bilinear between the nodes, the edge nodes' past the grid.
    fn at(&self, x: f64, z: f64) -> f64 {
        let f = |a: usize, v: f64| {
            let t = ((v - self.origin[a]) / self.cell[a]).clamp(0.0, (self.n[a] - 1) as f64);
            let i = (t.floor() as usize).min(self.n[a] - 2);
            (i, t - i as f64)
        };
        let ((i, tx), (j, tz)) = (f(0, x), f(1, z));
        let v = |i: usize, j: usize| self.v[i + j * self.n[0]];
        (v(i, j) * (1.0 - tx) + v(i + 1, j) * tx) * (1.0 - tz) + (v(i, j + 1) * (1.0 - tx) + v(i + 1, j + 1) * tx) * tz
    }
}

/// The join ground at one world point.
pub struct Point {
    pub h: f64,
    pub surface: u8,
    /// Each candidate's share, summing to 1.
    pub w: Vec<f64>,
    /// Distance to the nearest interior.
    pub near: f64,
    /// What of `h` is the maps' relief (MD57), not the blend.
    pub relief: f64,
}

impl Plan {
    /// The ground at a world point, from the candidate sources `cands` (indices into `sources`): the
    /// blend, pulled straight between the shared samples along every collar edge it is near.
    /// The maps' relief ([`Plan::relief_at`]) is added before the pull, so a collar's edge is straight
    /// between its shared samples with it too.
    pub fn at(&self, tiles: &[Tile], terrains: &[Terrain], cands: &[usize], x: f64, z: f64) -> Point {
        self.at_with(tiles, terrains, cands, None, x, z)
    }

    /// [`Plan::at`], with the relief taken over `relief_cands` ([`Plan::relief_cands`] of a rectangle
    /// holding the point; `None`, every source): the same height to the bit either way.
    pub fn at_with(&self, tiles: &[Tile], terrains: &[Terrain], cands: &[usize], relief_cands: Option<&[usize]>, x: f64, z: f64) -> Point {
        let mut pt = self.blend(tiles, terrains, cands, x, z);
        if pt.near > 0.0 {
            pt.relief = self.relief_at(relief_cands, x, z, pt.h);
            pt.h += pt.relief;
        }
        if pt.near > 2.0 * self.spacing && !self.collars.is_empty() {
            pt.h += self.pull(tiles, terrains, x, z);
        }
        pt
    }

    /// The maps' relief at a join point (MD57): 0 within every map's widest seam gate and on the
    /// world's edge, and past them, eased under [`RELIEF_RISE`] times the distance (and the
    /// [`Room`]), each near map's [`Relief`] read at the point folded into its keep, weighted by
    /// [`relief_weight`] and normalised to keep the spread of one (`Σ wᵢ rᵢ / √Σ wᵢ²`). A map more
    /// than twice as far as the nearest fades out of it by three times as far, so the sum is over a
    /// handful of maps and the same whichever superset of them `cands` lists. Below the blend it is
    /// eased under `blend` too, so it takes ground near 0 ft no lower (a raw height is unsigned).
    pub fn relief_at(&self, cands: Option<&[usize]>, x: f64, z: f64, blend: f64) -> f64 {
        if self.relief.is_empty() || self.relief_scale <= 0.0 {
            return 0.0;
        }
        let all: Vec<usize>;
        let cands = match cands {
            Some(c) => c,
            None => {
                all = (0..self.sources.len()).collect();
                &all
            }
        };
        let (mut dmin, mut clear) = (f64::INFINITY, f64::INFINITY);
        for &c in cands {
            let s = &self.sources[c];
            let d = s.distance(x, z);
            dmin = dmin.min(d);
            // The widest of its gates, not the gate of the side the point is past ([`Source::gate_at`]),
            // which steps where a point comes round a corner: the cap must be continuous.
            clear = clear.min(d - s.gates.iter().copied().fold(0.0, f64::max));
        }
        if clear <= 0.0 || dmin <= 0.0 {
            return 0.0;
        }
        // And from nothing on the world's own edge, where openmx closes a wrapped world with a strip
        // made from both edges' ground (MD29): that stays the blend's.
        let world = [f64::from(self.size[0]) * self.spacing, f64::from(self.size[1]) * self.spacing];
        let edge = x.min(z).min(world[0] - x).min(world[1] - z).max(0.0);
        // The room lowers the cap with the relief: by a gate or the world's edge the relief stands at
        // its cap, whose slope a lower relief alone would not lessen.
        let room = self.room.as_ref().map_or(1.0, |r| r.at(x, z));
        let cap = RELIEF_RISE * room * clear.min(edge);
        if cap <= 0.0 {
            return 0.0;
        }
        let (mut sum, mut sq) = (0.0, 0.0);
        for &c in cands {
            let s = &self.sources[c];
            let d = s.distance(x, z);
            if d >= 3.0 * dmin {
                continue;
            }
            let w = relief_weight(d) * fall(d / dmin - 2.0);
            if w <= 0.0 {
                continue;
            }
            let (fx, fz) = (fold(x, s.rect[0], s.rect[2]), fold(z, s.rect[1], s.rect[3]));
            sum += w * self.relief[c].at(fx - s.at[0], fz - s.at[1]);
            sq += w * w;
        }
        if sq <= 0.0 {
            return 0.0;
        }
        let r = self.relief_scale * room * sum / sq.sqrt();
        let cap = if r < 0.0 { cap.min(blend) } else { cap };
        if cap <= 0.0 {
            return 0.0;
        }
        cap * (r / cap).tanh()
    }

    /// The sources [`Plan::relief_at`] can hear anywhere in a world rectangle: every one within three
    /// times, or its widest gate past, the farthest the nearest can be from a point of it.
    pub fn relief_cands(&self, rect: [f64; 4]) -> Vec<usize> {
        if self.relief.is_empty() {
            return Vec::new();
        }
        let corners = [[rect[0], rect[1]], [rect[2], rect[1]], [rect[0], rect[3]], [rect[2], rect[3]]];
        // A rectangle's farthest point from a convex set is a corner.
        let far = self.sources.iter().map(|s| corners.iter().map(|c| s.distance(c[0], c[1])).fold(0.0, f64::max)).fold(f64::INFINITY, f64::min);
        (0..self.sources.len())
            .filter(|&c| {
                let s = &self.sources[c];
                let g = s.gates.iter().copied().fold(0.0, f64::max);
                rect_gap(rect, s.rect) <= (3.0 * far).max(far + g) + 1e-6
            })
            .collect()
    }

    /// How far the collar edges near a point pull the blend there: for each edge within its band, the
    /// straight line between the shared samples either side of the point's foot on the edge, less the
    /// blend at the foot, times the fall-off across the edge and past its ends. On an edge that is the
    /// line itself. At a collar corner the other edge's pull is 0, its foot being a shared sample.
    fn pull(&self, tiles: &[Tile], terrains: &[Terrain], x: f64, z: f64) -> f64 {
        let all: Vec<usize> = (0..self.sources.len()).collect();
        let f = |p: [f64; 2]| {
            let b = self.blend(tiles, terrains, &all, p[0], p[1]);
            b.h + if b.near > 0.0 { self.relief_at(Some(&all), p[0], p[1], b.h) } else { 0.0 }
        };
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
                return Point { h, surface, w, near: 0.0, relief: 0.0 };
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
            return Point { h: nearest.h, surface: nearest.surface, w, near: nearest.d, relief: 0.0 };
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
        Point { h, surface: nearest.surface, w, near: nearest.d, relief: 0.0 }
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
        self.coarse_lattice(tiles, stride).worst()
    }

    /// The world's ground on a lattice `stride` samples apart.
    fn coarse_lattice(&self, tiles: &[Tile], stride: u32) -> Lattice {
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
                    let band = [0.0, z0, nx as f64 * step, z0 + rows_each as f64 * step];
                    let gated = self.gated(band, step);
                    let rc = self.relief_cands(band);
                    for (i, o) in out.iter_mut().enumerate() {
                        let (kx, kz) = (i % row, part * rows_each + i / row);
                        let (x, z) = (kx as f64 * step, kz as f64 * step);
                        let pt = self.at_with(tiles, &terrains, near(x, z), Some(&rc), x, z);
                        *o = Coarse { h: pt.h, clear: self.clear(&gated, x, z) };
                    }
                });
            }
        });
        Lattice { h, n: [row, nz as usize + 1], step }
    }
}

/// The world's ground on a coarse lattice ([`Plan::coarse_lattice`]), row by row.
struct Lattice {
    h: Vec<Coarse>,
    n: [usize; 2],
    step: f64,
}

impl Lattice {
    /// Each cell's slope, or none for a cell within a lattice step of a seam gate.
    fn cells(&self) -> impl Iterator<Item = (usize, usize, Option<f64>)> + '_ {
        let (row, step) = (self.n[0], self.step);
        (0..self.n[1] - 1).flat_map(move |kz| {
            (0..row - 1).map(move |kx| {
                let h = &self.h;
                let c = [&h[kx + kz * row], &h[kx + 1 + kz * row], &h[kx + (kz + 1) * row], &h[kx + 1 + (kz + 1) * row]];
                let g = if c.iter().any(|p| p.clear < step) { None } else { Some(cell_slope([c[0].h, c[1].h, c[2].h, c[3].h], step)) };
                (kx, kz, g)
            })
        })
    }

    /// The steepest cell past the gates and where.
    fn worst(&self) -> (f64, [f64; 2]) {
        let mut worst = (0.0, [0.0; 2]);
        for (kx, kz, g) in self.cells() {
            if let Some(g) = g.filter(|&g| g > worst.0) {
                worst = (g, [kx as f64 * self.step, kz as f64 * self.step]);
            }
        }
        worst
    }
}

/// How much of the maps' relief the blend has room for under `max_slope` (MD57), on a lattice over
/// the world: 1 where the blend is no steeper than `max_slope` less [`RELIEF_SLOPE`] and a tenth,
/// falling to 0 where it is within a tenth of `max_slope`; then the least of it within
/// [`RELIEF_RAMP`] taken and smoothed over as far again either way, so the relief fades out before
/// steep blend, never on it, and gently.
pub struct Room {
    v: Vec<f64>,
    n: [usize; 2],
    step: f64,
}

impl Room {
    fn of(l: &Lattice, max_slope: f64) -> Room {
        let (n, step) = (l.n, l.step);
        let (lo, hi) = (max_slope * 0.9 - RELIEF_SLOPE, max_slope * 0.9);
        let mut v = vec![1.0f64; n[0] * n[1]];
        for (kx, kz, g) in l.cells() {
            // A cell by the gates is the blend's whatever the room: only steep blend past them counts.
            let Some(g) = g else { continue };
            let r = ((hi - g) / (hi - lo)).clamp(0.0, 1.0);
            for i in [kx + kz * n[0], kx + 1 + kz * n[0], kx + (kz + 1) * n[0], kx + 1 + (kz + 1) * n[0]] {
                v[i] = v[i].min(r);
            }
        }
        let k = (RELIEF_RAMP / step).ceil() as usize;
        let mut v = sweep(&sweep(&v, n, k, true, false), n, k, false, false);
        for _ in 0..2 {
            v = sweep(&sweep(&v, n, k / 2, true, true), n, k / 2, false, true);
        }
        Room { v, n, step }
    }

    /// A quarter less room about a world point, fading to none lost [`RELIEF_DENT`] away: where the
    /// relief made the ground too steep after all (two maps' hills of opposite sign meeting in a gap),
    /// it is lowered there, not over the whole world.
    fn dent(&mut self, x: f64, z: f64) {
        let k = (RELIEF_DENT / self.step).ceil() as isize;
        let (ci, cj) = ((x / self.step).round() as isize, (z / self.step).round() as isize);
        for j in (cj - k).max(0)..=(cj + k).min(self.n[1] as isize - 1) {
            for i in (ci - k).max(0)..=(ci + k).min(self.n[0] as isize - 1) {
                let d = (i as f64 * self.step - x).hypot(j as f64 * self.step - z) / RELIEF_DENT;
                if d < 1.0 {
                    self.v[i as usize + j as usize * self.n[0]] *= 1.0 - 0.25 * (1.0 - d * d).powi(2);
                }
            }
        }
    }

    /// The room at a world point, read off the lattice bilinearly.
    pub fn at(&self, x: f64, z: f64) -> f64 {
        let fx = (x / self.step).clamp(0.0, (self.n[0] - 1) as f64);
        let fz = (z / self.step).clamp(0.0, (self.n[1] - 1) as f64);
        let (i, j) = ((fx as usize).min(self.n[0] - 2), (fz as usize).min(self.n[1] - 2));
        let (tx, tz) = (fx - i as f64, fz - j as f64);
        let v = |i: usize, j: usize| self.v[i + j * self.n[0]];
        (v(i, j) * (1.0 - tx) + v(i + 1, j) * tx) * (1.0 - tz) + (v(i, j + 1) * (1.0 - tx) + v(i + 1, j + 1) * tx) * tz
    }
}

/// Each node's window `k` nodes either way along x (`along_x`) or z, cut at the lattice's ends:
/// its mean (`mean`) or its least.
fn sweep(v: &[f64], n: [usize; 2], k: usize, along_x: bool, mean: bool) -> Vec<f64> {
    let (len, lines) = if along_x { (n[0], n[1]) } else { (n[1], n[0]) };
    let at = |line: usize, i: usize| if along_x { i + line * n[0] } else { line + i * n[0] };
    let mut out = vec![0.0; v.len()];
    for line in 0..lines {
        for i in 0..len {
            let (a, b) = (i.saturating_sub(k), (i + k).min(len - 1));
            let mut acc = v[at(line, a)];
            for j in a + 1..=b {
                acc = if mean { acc + v[at(line, j)] } else { acc.min(v[at(line, j)]) };
            }
            out[at(line, i)] = if mean { acc / (b - a + 1) as f64 } else { acc };
        }
    }
    out
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
    /// [`Piece::far_slope`] of the ground without the maps' relief (MD57).
    pub far_base: f64,
    /// Where its far slope is steepest, world feet.
    pub far_at: [f64; 2],
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
    let mut relief = vec![0.0; mw * md];
    let rc = plan.relief_cands([rect[0] - step, rect[1] - step, rect[2] + step, rect[3] + step]);
    let mut near = vec![0.0; sw * sd];
    let mut clear = vec![0.0; sw * sd];
    let gated = plan.gated(rect, step);
    let mut surfaces = vec![0u8; sw * sd];
    let mut shares = vec![0.0; sw * sd * cands.len()];
    for mz in 0..md {
        for mx in 0..mw {
            let (kx, kz) = (mx as f64 - 1.0, mz as f64 - 1.0);
            let pt = plan.at_with(tiles, terrains, &cands, Some(&rc), spec.at[0] + kx * step, spec.at[1] + kz * step);
            h[mx + mz * mw] = pt.h;
            relief[mx + mz * mw] = pt.relief;
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
    let (mut far_slope, mut near_slope, mut far_base) = (0.0f64, 0.0f64, 0.0f64);
    let mut far_at = spec.at;
    for z in 0..sd - 1 {
        for x in 0..sw - 1 {
            let c = [x + z * sw, x + 1 + z * sw, x + (z + 1) * sw, x + 1 + (z + 1) * sw];
            if c.iter().any(|&i| near[i] == 0.0) {
                continue; // a tile's own cell
            }
            let m = c.map(|i| (i % sw) + 1 + (i / sw + 1) * mw);
            let g = cell_slope(m.map(|i| h[i]), step);
            if c.iter().all(|&i| clear[i] > 0.0) {
                if g > far_slope {
                    far_at = [spec.at[0] + (x as f64 + 0.5) * step, spec.at[1] + (z as f64 + 0.5) * step];
                }
                far_slope = far_slope.max(g);
                far_base = far_base.max(cell_slope(m.map(|i| h[i] - relief[i]), step));
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
        far_base,
        far_at,
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
    far_base: f64,
    far_at: [f64; 2],
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

/// The relief's scale a step lower ([`Plan::relief_scale`]): 0 once it is too small to show.
fn lower(scale: f64) -> f64 {
    let s = scale * 0.8;
    if s < 0.05 { 0.0 } else { s }
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
    // Each map's relief for the far joins (MD57), once: it hangs on the map's own ground alone.
    let t0 = std::time::Instant::now();
    let reliefs: Vec<Relief> = std::thread::scope(|sc| {
        let jobs: Vec<_> = tiles.iter().zip(&cuts).map(|(t, k)| sc.spawn(move || Relief::of(t, *k))).collect();
        jobs.into_iter().map(|j| j.join().expect("relief")).collect()
    });
    for (t, r) in tiles.iter().zip(&reliefs) {
        eprintln!("stitch: {}: relief {:.1} ft spread, {:.1} kept (steepest {:.3}, at {:.2})", t.id, r.spread, r.kept, r.slope, r.gain);
    }
    eprintln!("stitch: relief read in {:.1} s", t0.elapsed().as_secs_f64());
    let mut widen = 1.0;
    let mut relief_scale = 1.0f64;
    // Where the relief has been lowered ([`Room::dent`]), kept over a rebuild of the same plan.
    let mut dented: Vec<[f64; 2]> = Vec::new();
    let (plan, results) = loop {
        let t0 = std::time::Instant::now();
        let mut plan = plan(&tiles, &cuts, &gates, p, widen)?;
        let (rmin, rmax) = plan.radii.iter().fold((f64::INFINITY, 0.0f64), |(a, b), r| (a.min(*r), b.max(*r)));
        eprintln!(
            "stitch: widen {widen:.3}: world {:.0} × {:.0} ft, {} pieces, radius {rmin:.0}–{rmax:.0} ft, planned in {:.1} s",
            f64::from(plan.size[0]) * plan.spacing,
            f64::from(plan.size[1]) * plan.spacing,
            plan.pieces(p.chunk).len(),
            t0.elapsed().as_secs_f64()
        );
        let t0 = std::time::Instant::now();
        let lattice = plan.coarse_lattice(&tiles, 4);
        let (coarse, at) = lattice.worst();
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
            // The relief on top, lowered (never widened for) until the coarse lattice holds it.
            plan.relief = reliefs.clone();
            let mut room = Room::of(&lattice, p.max_slope);
            dented.iter().for_each(|d| room.dent(d[0], d[1]));
            plan.room = Some(room);
            loop {
                plan.relief_scale = relief_scale;
                if relief_scale <= 0.0 {
                    break;
                }
                let t0 = std::time::Instant::now();
                let (c, at) = plan.coarse_slope(&tiles, 4);
                eprintln!("stitch: coarse pass with the relief at {relief_scale:.3}: {c:.3} steepest, {:.1} s", t0.elapsed().as_secs_f64());
                if c <= p.max_slope {
                    break;
                }
                if dented.len() < RELIEF_DENTS {
                    dented.push(at);
                    eprintln!("stitch: {c:.3} at ({:.0}, {:.0}) with the maps' relief, past {}: lowering it there", at[0], at[1], p.max_slope);
                    plan.room.as_mut().expect("room").dent(at[0], at[1]);
                    continue;
                }
                eprintln!("stitch: {c:.3} at ({:.0}, {:.0}) with the maps' relief, past {}: lowering it", at[0], at[1], p.max_slope);
                relief_scale = lower(relief_scale);
            }
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
                results.lock().expect("lock").push(Done {
                    name: pc.name,
                    at: pc.at,
                    samples: pc.samples,
                    between: pc.between,
                    far: pc.far_slope,
                    near: pc.near_slope,
                    far_base: pc.far_base,
                    far_at: pc.far_at,
                    clamped: pc.clamped,
                });
                Ok(())
            })?;
            let mut results = results.into_inner().expect("lock");
            results.sort_by(|a, b| a.name.cmp(&b.name));
            eprintln!("stitch: {} pieces solved and written in {:.1} s", results.len(), t0.elapsed().as_secs_f64());
            let far = results.iter().map(|r| r.far).fold(0.0, f64::max);
            if far <= p.max_slope {
                break (plan, results);
            }
            let base = results.iter().map(|r| r.far_base).fold(0.0, f64::max);
            if base <= p.max_slope && relief_scale > 0.0 {
                let worst = results.iter().max_by(|a, b| a.far.total_cmp(&b.far)).expect("a piece");
                let there = dented.len() < RELIEF_DENTS;
                eprintln!(
                    "stitch: join ground {far:.3} steep with the maps' relief in {} (at {:.0}, {:.0}), {base:.3} without, past {}: lowering it{}",
                    worst.name,
                    worst.far_at[0],
                    worst.far_at[1],
                    p.max_slope,
                    if there { " there" } else { "" }
                );
                if there {
                    dented.push(worst.far_at);
                } else {
                    relief_scale = lower(relief_scale);
                }
                continue;
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
        dented.clear();
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
    todo.push(s(format!(
        "joins/*: past the seam gates the ground carries the nearest maps' own relief (MD57): \
         each map's ground folded back and forth from its keep, smoothed over about {:.0} ft and held to {RELIEF_SLOPE} a foot \
         (no jumps, ruts or cliffs of its own), rising from nothing at the gates by at most {RELIEF_RISE} ft a foot, \
         kept off blend too steep to hold it, carried at {:.2} over the world; generated, no source game's",
        5.0 * RELIEF_CELL,
        plan.relief_scale
    )));
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
        // MD66: the pick's and the store's category for every world (standard §1).
        ("category", s("World")),
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
                // The maps' relief on the far joins (MD57): its scale over the whole world, and each
                // map's own gain and smoothed spread (feet).
                (
                    "relief",
                    o(vec![
                        ("scale", J::N((plan.relief_scale * 1e4).round() / 1e4)),
                        (
                            "tiles",
                            J::A(
                                tiles
                                    .iter()
                                    .zip(&plan.relief)
                                    .map(|(t, r)| {
                                        o(vec![("tile", s(&t.id)), ("gain", J::N((r.gain * 1e4).round() / 1e4)), ("spread", J::N((r.spread * 100.0).round() / 100.0))])
                                    })
                                    .collect(),
                            ),
                        ),
                    ]),
                ),
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
