//! **The ATV's ground** (MD76): MX vs. ATV Unleashed's ground query (SLUS-21104), the one the quad's
//! wheel suspension asks under each probe's end, with the terrain's cell reader under it. Proved
//! against SLUS-21104's own two functions on Desert Oasis (`tests/atv_ground.rs`).
//!
//! It is MX Unleashed's terrain query ([`crate::contact_search::ground`]) built again, with three
//! differences:
//!
//! - The point is held inside the terrain's box first, a margin (`+0x50`, 0.01 ft) in from each edge,
//!   before it is turned into samples. Past an edge the ground is the edge's.
//! - The cell reader hands the normals back scaled by `1/127` (the node's vertical scale on `x` and
//!   `z` besides), and a flattened node's height is one multiply-add.
//! - The two height sums are the FPU's accumulator ops (`adda 0, h`, `madda`, `madd`), so a `-0`
//!   ground comes out `+0`.
//!
//! The terrain object's fields move too (MX Unleashed's offset less 0x10 for the box, more for the
//! rest): box `+0x30`/`+0x40`, margin `+0x50`, height scale `+0x90`, cell reader `+0x94`, split one
//! way `+0x770 == 1`, samples per foot `+0x788`, coarse `+0x8b8`.
//!
//! The original keeps the last ten cells it read in a ring shared by every terrain, keyed by the
//! sample alone. The ring changes nothing a single loaded terrain answers (the test runs the original
//! with it warm and cold), so the port has none.
//!
//! The cell reader can take its normals from a table of 256 directions instead, when a global flag is
//! set; nothing in SLUS-21104 writes that flag (`./kit xref` lists four reads), so only the per-sample
//! bytes are ported.

#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use crate::contact_search::Cell;
use crate::track;
use ps2_float::fpu::Acc;
use ps2_float::{F, Field, Vf, vu};

/// Up, as the original keeps it: what the normal is when the terrain has no cell there.
pub const UP: Vf = Vf::new(0.0, 1.0, 0.0, 1.0);

/// How far in from the box's edges a point is held, `0x3c23d70a` (0.01 ft): the terrain object's
/// `+0x50` on Desert Oasis.
pub const MARGIN: f32 = 0.01;

/// What a per-sample normal byte is multiplied by, `1/127`, worked out by a divide each time.
const NORMAL_BYTE: f32 = 127.0;

/// The terrain object as the ground query reads it.
#[derive(Clone, Copy, Debug)]
pub struct Terrain<'a> {
    /// `+0x94`: the cell reader's terrain, or none (the query then writes nothing at all).
    pub map: Option<&'a track::Terrain<'a>>,
    /// `+0x30`, `+0x40`: the box the point is held in (`x` and `z`), less [`Terrain::margin`].
    pub min: Vf,
    pub max: Vf,
    /// `+0x50`.
    pub margin: F,
    /// `+0x788`: samples per foot.
    pub per_sample: F,
    /// `+0x90`: what the cell reader's heights are multiplied by.
    pub height_scale: F,
    /// `+0x8b8`: the terrain read two samples to a cell.
    pub coarse: bool,
    /// `+0x770 == 1`: every cell split the same way, not by the cell's parity.
    pub split_one_way: bool,
}

impl<'a> Terrain<'a> {
    /// The terrain object SLUS-21104 builds over a map's terrain file: the box from the origin to
    /// the samples times the file's scale (`y` is not read), [`MARGIN`], `1/scale` samples per
    /// foot, the scale as the height scale, one sample to a cell, split one way. On Desert Oasis
    /// these are the words the loaded object holds (the test checks them).
    pub fn of(map: &'a track::Terrain<'a>) -> Terrain<'a> {
        let [sx, sz] = map.samples().unwrap_or([0, 0]);
        let far = |s: u32| (F::from(s as i32) * map.scale).0;
        Terrain {
            map: Some(map),
            min: Vf::new(0.0, 0.0, 0.0, 1.0),
            max: Vf::new(far(sx), 0.0, far(sz), 1.0),
            margin: F(MARGIN),
            per_sample: F::ONE / map.scale,
            height_scale: map.scale,
            coarse: false,
            split_one_way: true,
        }
    }

    /// The query as the wheel suspension asks it: on a copy of `at`, the point moved onto the
    /// ground and the ground's normal, and the surface under it into `record` (the wheel's
    /// `+0x110`). `normal` is what the caller's buffer held, which comes back unchanged where the
    /// query writes none.
    pub fn ground(&self, at: Vf, normal: Vf, record: &mut u8) -> (Vf, Vf) {
        let (mut p, mut n) = (at, normal);
        query(self, &mut p, Some(&mut n), Some(record));
        (p, n)
    }
}

/// SLUS-21104's cell reader: the cell at sample `(ix, iz)`, `step` samples wide, down the map's
/// tree; `None` where it has none. Heights are the node's (a flattened node pulls the block's
/// relief towards its base by one multiply-add), normals the stored bytes over 127, `x` and `z`
/// times the node's vertical scale too.
pub fn cell(map: &track::Terrain<'_>, ix: i32, iz: i32, step: i32) -> Option<Cell> {
    let (node, lx, lz) = map.leaf(u32::try_from(ix).ok()?, u32::try_from(iz).ok()?)?;
    let block = map.block(node.block)?;
    let s = step as u32;
    let corners = [(lx, lz), (lx + s, lz), (lx, lz + s), (lx + s, lz + s)];
    let vs = node.vertical_scale;
    let mut heights = [0.0f32; 4];
    for (h, &(x, z)) in heights.iter_mut().zip(&corners) {
        let raw = i32::from(block.raw_height(x, z)?);
        *h = if vs == F(1.0) {
            node.height_scale * F::from(raw)
        } else {
            let base = block.base;
            node.height_scale * Acc::adda(F::ZERO, F::from(base)).madd(vs, F::from(raw.wrapping_sub(base)))
        }
        .0;
    }
    let k = F::ONE / NORMAL_BYTE;
    let side = vs * k;
    let mut normals = [Vf::ZERO; 4];
    let mut surfaces = [0u8; 4];
    for (i, &(x, z)) in corners.iter().enumerate() {
        let [nx, ny, nz] = block.raw_normal(x, z)?.map(|b| F::from(i32::from(b)));
        normals[i] = Vf::new((nx * side).0, (ny * k).0, (nz * side).0, 1.0);
        surfaces[i] = block.surface(x, z)?;
    }
    let [h0, h1, h2, h3] = heights;
    Some(Cell { heights: Vf::new(h0, h1, h2, h3), normals, surfaces })
}

/// The ground query: the ground under `point`'s `x` and `z`.
///
/// Where the terrain has a cell there, `point.y` becomes the ground's height (`x`, `z` and `w` are
/// kept), `normal` the corner normals blended by weight and normalised (`w` the blend's), and
/// `record` the surface of the corner nearest the point (the first, on a tie). Where it has none,
/// only `normal` is written, straight up. With no terrain, nothing is.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn query(t: &Terrain<'_>, point: &mut Vf, normal: Option<&mut Vf>, record: Option<&mut u8>) {
    let Some(map) = t.map else { return };
    let (step, wide) = if t.coarse { (2, F(2.0)) } else { (1, F::ONE) };
    let m = t.margin;
    let hold = |p: f32, lo: f32, hi: f32| F(p).max(F(lo) + m).min(F(hi) - m);
    let fx = (t.per_sample * hold(point.x(), t.min.x(), t.max.x())).max(F::ZERO);
    let fz = (t.per_sample * hold(point.z(), t.min.z(), t.max.z())).max(F::ZERO);
    let (mut ix, mut iz) = (fx.to_i32(), fz.to_i32());
    if step == 2 {
        ix -= ix & 1;
        iz -= iz & 1;
    }
    if ix < 0 || iz < 0 {
        return;
    }
    let Some(c) = cell(map, ix, iz, step) else {
        if let Some(n) = normal {
            *n = UP;
        }
        return;
    };
    let h = c.heights.mul(Vf::splat(t.height_scale.0));
    let [h0, h1, h2, h3] = h.0.map(F);

    let u = (fx - F::from(ix)) / wide;
    let nu = F::ONE - u;
    let v = (fz - F::from(iz)) / wide;
    let nv = F::ONE - v;
    let (uu, vv, nunu, nvnv) = (u * u, v * v, nu * nu, nv * nv);
    // Each corner's squared distance from the point: the sample, +x, +z, both.
    let d = [uu + vv, nunu + vv, uu + nvnv, nunu + nvnv];

    if let Some(r) = record {
        let mut best = F::from_bits(0x7f7f_ffff);
        let mut nearest = None;
        for (i, &di) in d.iter().enumerate() {
            if di < best {
                best = di;
                nearest = Some(i);
            }
        }
        // With no corner nearer than the largest float (a NaN point) the original reads a stale
        // register for the corner; the port leaves the record.
        if let Some(i) = nearest {
            *r = c.surfaces[i];
        }
    }
    if let Some(n) = normal {
        let w = Vf::new((nu * nv).0, (u * nv).0, (nu * v).0, (u * v).0);
        let [n0, n1, n2, n3] = c.normals;
        let acc = n0.mul(w.bc(Field::X));
        let acc = Vf::madd(acc, n1, w.bc(Field::Y));
        let acc = Vf::madd(acc, n2, w.bc(Field::Z));
        let b = Vf::madd(acc, n3, w.bc(Field::W));
        *n = b.merge(ps2_float::Dest::XYZ, b.mul_q(vu::rsqrt(1.0, vu::dot3(b.0, b.0))));
    }
    let y = if t.split_one_way || (ix & 1) != (iz & 1) {
        if d[0] < d[3] {
            Acc::adda(F::ZERO, h0).madda(u, h1 - h0).madd(v, h2 - h0)
        } else {
            Acc::adda(F::ZERO, h2).madda(u, h3 - h2).madd(F::ONE - v, h1 - h3)
        }
    } else if d[1] < d[2] {
        Acc::adda(F::ZERO, h0).madda(u, h1 - h0).madd(v, h3 - h1)
    } else {
        Acc::adda(F::ZERO, h2).madda(u, h3 - h2).madd(F::ONE - v, h0 - h2)
    };
    *point = point.merge(ps2_float::Dest::Y, Vf::splat(y.0));
}
