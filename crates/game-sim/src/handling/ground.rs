//! **The ground under SLUS-21104's bike** (F31c): the Free Ride map as MX vs. ATV's collision reads
//! it, for a [`super::Handling::MxVsAtv`] ride that starts where the original's bike is (C15's ride and
//! flip, state 4: Nat05; where the image keeps it: research/oracle.txt `# F31c`).
//!
//! The bike asks the ground in two places, both on the same terrain object over the map's `.trn`:
//!
//! - each wheel's suspension, every substep, asks the ground query ([`crate::atv_ground::query`],
//!   MD76) at its probe end, with its `+0x110` as the record byte;
//! - the bike's collision record, for each of its rays pointing down whose node is over the map, asks
//!   the terrain the way [`under`] does: the point's height, the blended normal, the cell's four
//!   surfaces and each corner's weight, which the terrain service hands back field by field.
//!
//! The wheels' own search (the bike's two trigger rays) and the record's rays search the props near
//! the bike too. Along C15's rides neither files a contact (`tests/mxvatv_bike_ground.rs` checks the
//! record count), so the ground here is the terrain alone.
//!
//! [`terrain`] is the object as [`crate::atv_ground::Terrain::of`] builds it from the map's file;
//! [`read_terrain`] reads it out of RAM, and [`terrain_words`] is its inverse.
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::seed::Words;
use crate::archive::Res;
use crate::atv_ground::{self, Terrain, UP};
use crate::contact_search::Ground;
use crate::track;
use ps2_float::fpu::Acc;
use ps2_float::{F, Field, Vf, vu};

/// The wheel's terrain object (wheel-relative), the same for both wheels.
pub const WHEEL_TERRAIN: u32 = 0x1e0;
/// The wheel's record byte, where the ground query puts the surface under it.
pub const WHEEL_RECORD: u32 = 0x110;

/// The map's terrain file inside its `.res`: `Env\<map>\<map>.ps2.trn`, any case (Nat05's is spelt
/// `Env\nat05\Nat05.ps2.trn`).
pub fn terrain_file(res: &[u8], map: &str) -> Option<Vec<u8>> {
    let archive = Res::parse(res).ok()?;
    let want = format!("env\\{0}\\{0}.ps2.trn", map.to_ascii_lowercase());
    let entry = archive.entries.iter().find(|e| e.name.to_ascii_lowercase() == want)?;
    archive.read(res, entry).ok()
}

/// The terrain object over `map` as SLUS-21104 builds it ([`Terrain::of`]).
pub fn terrain<'a>(map: &'a track::Terrain<'a>) -> Terrain<'a> {
    Terrain::of(map)
}

/// The terrain object at `at` in RAM, over `map` (the object's cell reader is the map's tree).
pub fn read_terrain<'a>(w: &impl Words, at: u32, map: Option<&'a track::Terrain<'a>>) -> Terrain<'a> {
    Terrain {
        map,
        min: w.v(at + 0x30),
        max: w.v(at + 0x40),
        margin: w.f(at + 0x50),
        per_sample: w.f(at + 0x788),
        height_scale: w.f(at + 0x90),
        coarse: w.b(at + 0x8b8),
        split_one_way: w.word(at + 0x770) == 1,
    }
}

/// The words [`read_terrain`] reads, address and value, for the terrain `t` at `at`: the box's `x`
/// and `z` (its `y` and `w` are not read), the margin, samples per foot, the height scale and both
/// flags.
pub fn terrain_words(t: &Terrain<'_>, at: u32) -> Vec<(u32, u32)> {
    let bits = |x: f32| x.to_bits();
    vec![
        (at + 0x30, bits(t.min.x())),
        (at + 0x38, bits(t.min.z())),
        (at + 0x40, bits(t.max.x())),
        (at + 0x48, bits(t.max.z())),
        (at + 0x50, bits(t.margin.0)),
        (at + 0x788, bits(t.per_sample.0)),
        (at + 0x90, bits(t.height_scale.0)),
        (at + 0x8b8, u32::from(t.coarse)),
        (at + 0x770, u32::from(t.split_one_way)),
    ]
}

/// The collision record's terrain query, on its buffer `g`: `g.point`'s `x` and `z` are read.
///
/// Where the terrain has a cell there, `g.point.y` becomes the ground's height, `g.normal` the corner
/// normals blended and normalised (`w` the blend's), `g.surfaces` the cell's four and `g.weights`
/// each corner's (the sample, `+x`, `+z`, both). Where it has none, the normal is straight up and the
/// surfaces none; the rest is left. Off the map (no cell reader) nothing is written.
///
/// The point is held in the box and the height worked out as the wheels' query does it
/// ([`atv_ground::query`]); only what it hands back differs.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn under(t: &Terrain<'_>, g: &mut Ground) {
    let Some(map) = t.map else { return };
    let (step, wide) = if t.coarse { (2, F(2.0)) } else { (1, F::ONE) };
    let m = t.margin;
    let hold = |p: f32, lo: f32, hi: f32| F(p).max(F(lo) + m).min(F(hi) - m);
    let fx = (t.per_sample * hold(g.point.x(), t.min.x(), t.max.x())).max(F::ZERO);
    let fz = (t.per_sample * hold(g.point.z(), t.min.z(), t.max.z())).max(F::ZERO);
    let (mut ix, mut iz) = (fx.to_i32(), fz.to_i32());
    if step == 2 {
        ix -= ix & 1;
        iz -= iz & 1;
    }
    if ix < 0 || iz < 0 {
        return;
    }
    let Some(c) = atv_ground::cell(map, ix, iz, step) else {
        g.normal = UP;
        g.surfaces = [0; 4];
        return;
    };
    let h = c.heights.mul(Vf::splat(t.height_scale.0));
    let [h0, h1, h2, h3] = h.0.map(F);
    g.surfaces = c.surfaces;

    let u = (fx - F::from(ix)) / wide;
    let v = (fz - F::from(iz)) / wide;
    let (nu, nv) = (F::ONE - u, F::ONE - v);
    let (uu, vv, nunu, nvnv) = (u * u, v * v, nu * nu, nv * nv);
    let d = [uu + vv, nunu + vv, uu + nvnv, nunu + nvnv];
    let w = Vf::new((nu * nv).0, (u * nv).0, (nu * v).0, (u * v).0);
    g.weights = w;

    let [n0, n1, n2, n3] = c.normals;
    let acc = n0.mul(w.bc(Field::X));
    let acc = Vf::madd(acc, n1, w.bc(Field::Y));
    let acc = Vf::madd(acc, n2, w.bc(Field::Z));
    let b = Vf::madd(acc, n3, w.bc(Field::W));
    g.normal = b.merge(ps2_float::Dest::XYZ, b.mul_q(vu::rsqrt(1.0, vu::dot3(b.0, b.0))));

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
    g.point = g.point.merge(ps2_float::Dest::Y, Vf::splat(y.0));
}

/// The surface the terrain service reports for [`under`]'s answer: the corner with the largest
/// weight over zero (the first, on a tie), the sample's when none is.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compare
pub fn surface(g: &Ground) -> u8 {
    let mut best = F::ZERO;
    let mut corner = 0;
    for (i, &x) in g.weights.0.iter().enumerate() {
        if !(F(x) <= best) {
            best = F(x);
            corner = i;
        }
    }
    g.surfaces[corner]
}
