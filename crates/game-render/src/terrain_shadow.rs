//! Our own baked shadows of the ground's hills (V117), read under [`Shadows::All`] in place of the
//! track's light map ([`Shadow`]) where that covers.
//!
//! The disc's light maps (`<track>_shadow_01..16`) hold the hills' shadows already, but faint, at
//! under three feet a texel, and cast by the disc's own sun, not the scene's that the port lights and
//! casts its live shadows by. The ground is left out of the live sun maps (`gpu/shadows.rs`): a hill
//! is too big for them. So under All a map's ground is baked once more, here: from each texel the
//! heightfield is marched toward the scene's sun, and the least angle by which the ray clears the
//! ground on the way says how much of the sun it sees. A ray that clears a ridge by a hair is half
//! lit, and the same hair is more ground the further the ridge is, so the edge softens with the
//! distance to what casts it, as a real one does ([`PENUMBRA`]).
//!
//! The bake keeps the light map's tone, so On and All agree where nothing casts: a lit texel is the
//! light map's own value, lifted only where the light map is darker than the scene's sun lights that
//! slope by more than [`LIFT_TOLERANCE`] (a shadow the disc's sun cast and ours does not). A texel in
//! our shadow goes down to the light map's own level in its shadows ([`light_map_levels`]' first),
//! which is what the moving shadows darken to as well, and never below that: no shadow is darkened
//! twice. The result is a [`Shadow`] over the same rectangle as the light map, so the renderers read
//! it exactly as they read that one.
//!
//! None of this is the original's, which has no shadows of the hills but its light maps: it is ours,
//! matched by eye.
//!
//! [`Shadows::All`]: crate::shadow::Shadows::All
//! [`light_map_levels`]: crate::shadow::light_map_levels

use crate::mat::V3;
use crate::pst::Texture;
use crate::splat::Shadow;
use std::path::{Path, PathBuf};

/// The baked map's side in texels: 4096 over a free-ride map's 5760 lit feet is 1.4 ft a texel, half
/// the light map's. A browser's is half that again, as it bakes on one thread.
pub const SIZE: u32 = if cfg!(target_arch = "wasm32") { 2048 } else { 4096 };

/// How soft the edge is: the ray's clearance over the ground per foot of distance (a slope, as a
/// tangent) over which a texel goes from lit to shadowed, each way. Twice the moving shadows' own
/// spread ([`crate::shadow::PENUMBRA`]), as the hills' shadows are seen from further off.
pub const PENUMBRA: f32 = 0.04;

/// How far over the ground a ray starts, in feet, so a flat field does not shadow itself.
pub const START_BIAS: f32 = 0.5;

/// The furthest a ray is marched, in feet: past it, a hill's shadow would be a mile long.
pub const REACH: f32 = 4000.0;

/// How much darker than the scene's sun lights a slope the light map may be before the bake lifts it
/// (its own units, 128 is 1): a shadow cast by the disc's sun that ours does not cast.
pub const LIFT_TOLERANCE: f32 = 0.1;

/// The bake's version, in its cache key: a new bake makes every map's again.
pub const VERSION: u32 = 1;

/// The ground's heights, a grid of samples `spacing` feet apart from the world's origin, row by row
/// along z: `heights[z * nx + x]` is the height at `(x * spacing, z * spacing)`. A sample with no
/// ground is NaN, and casts nothing.
pub struct Field {
    pub heights: Vec<f32>,
    pub nx: usize,
    pub nz: usize,
    pub spacing: f32,
}

impl Field {
    /// The height at a world point, bilinear between the samples, or `None` off the field or beside
    /// a sample with no ground.
    pub fn at(&self, x: f32, z: f32) -> Option<f32> {
        let (fx, fz) = (x / self.spacing, z / self.spacing);
        if !(fx >= 0.0 && fz >= 0.0) {
            return None;
        }
        let (i, j) = (fx as usize, fz as usize);
        if i + 1 >= self.nx || j + 1 >= self.nz {
            return None;
        }
        let (u, v) = (fx - i as f32, fz - j as f32);
        let h = |a: usize, b: usize| self.heights[b * self.nx + a];
        let top = h(i, j) + (h(i + 1, j) - h(i, j)) * u;
        let bottom = h(i, j + 1) + (h(i + 1, j + 1) - h(i, j + 1)) * u;
        let y = top + (bottom - top) * v;
        y.is_finite().then_some(y)
    }

    /// The highest sample.
    fn top(&self) -> f32 {
        self.heights.iter().copied().filter(|h| h.is_finite()).fold(f32::MIN, f32::max)
    }

    /// How much of a sun along `sun` (the way its light goes) the ground at `(x, z)` sees: 1 lit, 0
    /// in shadow, between at a soft edge; `None` where there is no ground.
    pub fn sunlit(&self, x: f32, z: f32, sun: V3, top: f32) -> Option<f32> {
        let y0 = self.at(x, z)? + START_BIAS;
        let to = sun.norm().scale(-1.0);
        let flat = (to.x * to.x + to.z * to.z).sqrt();
        if to.y <= 0.0 {
            // A sun under the horizon lights nothing.
            return Some(0.0);
        }
        if flat < 1e-4 {
            return Some(1.0);
        }
        let (dx, dz, rise) = (to.x / flat, to.z / flat, to.y / flat);
        let step = self.spacing * 0.5;
        let reach = ((top - y0) / rise).min(REACH);
        let mut clear = f32::MAX;
        let mut r = step;
        while r <= reach + step {
            if let Some(h) = self.at(x + dx * r, z + dz * r) {
                clear = clear.min((y0 + rise * r - h) / r);
                if clear <= -PENUMBRA {
                    return Some(0.0);
                }
            }
            // Finer near, where a slope's own shadow starts; coarser far, where an edge is soft.
            r += step * (1.0 + r / 400.0);
        }
        Some(smoothstep(-PENUMBRA, PENUMBRA, clear))
    }

    /// How the scene's sun lights the ground at `(x, z)` by its slope alone: the cosine to the sun,
    /// 0 facing away.
    pub fn facing(&self, x: f32, z: f32, sun: V3) -> Option<f32> {
        let d = self.spacing;
        let (l, r, b, f) = (self.at(x - d, z)?, self.at(x + d, z)?, self.at(x, z - d)?, self.at(x, z + d)?);
        let n = crate::mat::v3(l - r, 2.0 * d, b - f).norm();
        Some((-n.dot(sun.norm())).max(0.0))
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The light map's tone against the scene's sun: `light ≈ a + b * facing` over the texels the bake
/// finds lit, by least squares, from every `stride`th texel each way.
fn tone(disc: &Shadow, field: &Field, sun: V3, size: usize, lit: &[f32]) -> (f32, f32) {
    let stride = (size / 256).max(1);
    let (mut n, mut sx, mut sy, mut sxx, mut sxy) = (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for j in (0..size).step_by(stride) {
        for i in (0..size).step_by(stride) {
            if lit[j * size + i] < 1.0 {
                continue;
            }
            let (x, z) = texel_world(disc, size, i, j);
            let (Some(d), Some(f)) = (disc.at(x, z), field.facing(x, z, sun)) else { continue };
            let (f, d) = (f as f64, d as f64);
            n += 1.0;
            sx += f;
            sy += d;
            sxx += f * f;
            sxy += f * d;
        }
    }
    let var = n * sxx - sx * sx;
    if n < 16.0 || var.abs() < 1e-9 {
        // Too little to fit: the light map's mean, whatever the slope.
        return (if n > 0.0 { (sy / n) as f32 } else { 1.0 }, 0.0);
    }
    let b = (n * sxy - sx * sy) / var;
    (((sy - b * sx) / n) as f32, b as f32)
}

/// The world point at the middle of texel `(i, j)` of a `size`-texel bake over `disc`'s rectangle,
/// its top row at `z1` as the light map's.
fn texel_world(disc: &Shadow, size: usize, i: usize, j: usize) -> (f32, f32) {
    let u = (i as f32 + 0.5) / size as f32;
    let v = (j as f32 + 0.5) / size as f32;
    (disc.x0 + u * (disc.x1 - disc.x0), disc.z1 - v * (disc.z1 - disc.z0))
}

/// Bake the hills' shadows for a sun along `sun` over the light map `disc` covers, `size` texels a
/// side, in the light map's units (128 is 1), with `shade` the level our shadows darken to
/// ([`crate::shadow::light_map_levels`]' first). One grey byte a texel, row by row from the top.
pub fn bake_grey(disc: &Shadow, field: &Field, sun: V3, size: u32, shade: f32, threads: usize) -> Vec<u8> {
    let size = size.max(1) as usize;
    let top = field.top();
    let rows = |lit: &mut [f32], first: usize| {
        for (k, row) in lit.chunks_mut(size).enumerate() {
            for (i, t) in row.iter_mut().enumerate() {
                let (x, z) = texel_world(disc, size, i, first + k);
                *t = field.sunlit(x, z, sun, top).unwrap_or(1.0);
            }
        }
    };
    let mut lit = vec![1.0f32; size * size];
    let threads = threads.max(1);
    if threads == 1 {
        rows(&mut lit, 0);
    } else {
        let band = size.div_ceil(threads);
        std::thread::scope(|scope| {
            for (b, chunk) in lit.chunks_mut(band * size).enumerate() {
                let rows = &rows;
                scope.spawn(move || rows(chunk, b * band));
            }
        });
    }
    let (a, b) = tone(disc, field, sun, size, &lit);
    let mut out = vec![128u8; size * size];
    for j in 0..size {
        for i in 0..size {
            let (x, z) = texel_world(disc, size, i, j);
            let Some(d) = disc.at(x, z) else { continue };
            let fit = field.facing(x, z, sun).map_or(d, |f| a + b * f);
            let lit_value = d.max(fit - LIFT_TOLERANCE);
            let dark = lit_value.min(shade);
            let s = lit[j * size + i];
            let v = dark + (lit_value - dark) * s;
            out[j * size + i] = (v * 128.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}

/// [`bake_grey`]'s bytes as a [`Shadow`] over `disc`'s rectangle.
pub fn as_shadow(disc: &Shadow, grey: &[u8], size: u32) -> Shadow {
    let rgba = grey.iter().flat_map(|&g| [g, g, g, 255]).collect();
    Shadow {
        tiles: vec![Texture { width: size as usize, height: size as usize, rgba }],
        across: 1,
        x0: disc.x0,
        x1: disc.x1,
        z0: disc.z0,
        z1: disc.z1,
    }
}

/// The cache key of a bake: [`VERSION`], the side, the sun, the field and the light map, hashed.
pub fn key(disc: &Shadow, field: &Field, sun: V3, size: u32) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend(VERSION.to_le_bytes());
    bytes.extend(size.to_le_bytes());
    for v in [sun.x, sun.y, sun.z, field.spacing, disc.x0, disc.x1, disc.z0, disc.z1] {
        bytes.extend(v.to_bits().to_le_bytes());
    }
    bytes.extend((field.nx as u64).to_le_bytes());
    for h in &field.heights {
        bytes.extend(h.to_bits().to_le_bytes());
    }
    for t in &disc.tiles {
        bytes.extend((t.width as u64).to_le_bytes());
        bytes.extend(t.rgba.iter().step_by(4));
    }
    format!("{:016x}", twox_hash::XxHash3_64::oneshot(&bytes))
}

/// Where a map's bakes are kept: `<mods>/cache/shadows/`.
pub fn cache_dir(mods: &Path) -> PathBuf {
    mods.join("cache").join("shadows")
}

const MAGIC: &[u8; 8] = b"OMXTSH01";

/// A map's bake: from `<cache>/<track>-<key>.grey` when one was written, else baked now and written
/// there, any older bake of the track going. The flag says whether it came from the cache. A cache
/// that cannot be read or written only costs the bake.
pub fn load_or_bake(cache: Option<&Path>, track: &str, disc: &Shadow, field: &Field, sun: V3, size: u32, shade: f32, threads: usize) -> (Shadow, bool) {
    let key = key(disc, field, sun, size);
    let file = cache.map(|c| c.join(format!("{track}-{key}.grey")));
    if let Some(bytes) = file.as_ref().and_then(|f| std::fs::read(f).ok()) {
        let n = (size as usize).pow(2);
        if bytes.len() == MAGIC.len() + n && bytes.starts_with(MAGIC) {
            return (as_shadow(disc, &bytes[MAGIC.len()..], size), true);
        }
    }
    let grey = bake_grey(disc, field, sun, size, shade, threads);
    if let (Some(dir), Some(file)) = (cache, &file) {
        if std::fs::create_dir_all(dir).is_ok() {
            if let Ok(old) = std::fs::read_dir(dir) {
                for e in old.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.starts_with(&format!("{track}-")) && name.ends_with(".grey") {
                        let _ = std::fs::remove_file(e.path());
                    }
                }
            }
            let mut bytes = MAGIC.to_vec();
            bytes.extend_from_slice(&grey);
            let part = file.with_extension("part");
            if std::fs::write(&part, &bytes).is_ok() {
                let _ = std::fs::rename(&part, file);
            }
        }
    }
    (as_shadow(disc, &grey, size), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mat::v3;

    /// A flat field 1000 ft a side at 10 ft a sample, with a ridge 100 ft high along x = 500 when
    /// `hill`.
    fn field(hill: bool) -> Field {
        let (nx, nz, spacing) = (101, 101, 10.0);
        let heights = (0..nx * nz)
            .map(|k| {
                let x = (k % nx) as f32 * spacing;
                if hill { (100.0 - (x - 500.0).abs() * 2.0).max(0.0) } else { 0.0 }
            })
            .collect();
        Field { heights, nx, nz, spacing }
    }

    /// A light map of one grey over the field's middle.
    fn disc(grey: u8) -> Shadow {
        let rgba = [grey, grey, grey, 255].repeat(64 * 64);
        Shadow { tiles: vec![Texture { width: 64, height: 64, rgba }], across: 1, x0: 50.0, x1: 950.0, z0: 50.0, z1: 950.0 }
    }

    fn grey_at(s: &Shadow, x: f32, z: f32) -> f32 {
        s.at(x, z).unwrap() * 128.0
    }

    /// A sun going down toward +x at 45 degrees: the ridge shadows the ground past it, 100 ft of it.
    #[test]
    fn a_ridge_shadows_the_ground_away_from_the_sun() {
        let sun = v3(1.0, -1.0, 0.0).norm();
        let d = disc(140);
        let shade = 80.0 / 128.0;
        let size = 128;
        let hill = as_shadow(&d, &bake_grey(&d, &field(true), sun, size, shade, 4), size);
        let flat = as_shadow(&d, &bake_grey(&d, &field(false), sun, size, shade, 4), size);
        // Behind the ridge, away from the sun, 40 ft past its foot: dark, and darker than flat ground.
        let behind = grey_at(&hill, 590.0, 500.0);
        assert!(behind < 90.0, "behind the ridge: {behind}");
        assert!(behind < grey_at(&flat, 590.0, 500.0) - 40.0);
        // In the open, before the ridge: the light map's own.
        assert!((grey_at(&hill, 200.0, 500.0) - 140.0).abs() <= 2.0, "{}", grey_at(&hill, 200.0, 500.0));
        assert!((grey_at(&flat, 590.0, 500.0) - 140.0).abs() <= 2.0);
        // Never under the light map's own shadow level.
        assert!(behind >= 79.0, "{behind}");
        // And past where the shadow reaches (the ridge's 100 ft at 45 degrees), lit again.
        assert!((grey_at(&hill, 800.0, 500.0) - 140.0).abs() <= 2.0);
    }

    /// The edge is soft, and softer the further it falls from the ridge.
    #[test]
    fn the_edge_softens_with_distance() {
        let f = field(true);
        let top = f.top();
        // A low sun: the shadow reaches 400 ft past the ridge's top.
        let sun = v3(4.0, -1.0, 0.0).norm();
        let width = |sun: V3| {
            let xs: Vec<f32> = (0..2000).map(|k| 500.0 + k as f32 * 0.25).collect();
            let s: Vec<f32> = xs.iter().map(|&x| f.sunlit(x, 500.0, sun, top).unwrap()).collect();
            let first = xs.iter().zip(&s).rev().find(|(_, s)| **s < 0.95).map(|(x, _)| *x).unwrap();
            let last = xs.iter().zip(&s).rev().find(|(_, s)| **s < 0.05).map(|(x, _)| *x).unwrap();
            (first - last, last)
        };
        let (soft_far, edge_far) = width(sun);
        let (soft_near, edge_near) = width(v3(1.0, -1.0, 0.0).norm());
        assert!(edge_far > edge_near + 200.0, "{edge_far} {edge_near}");
        assert!(soft_far > soft_near, "{soft_far} {soft_near}");
        assert!(soft_near > 0.5, "{soft_near}");
    }

    /// A shadow the light map has and our sun does not cast is lifted to the slope's own light.
    #[test]
    fn a_disc_shadow_our_sun_does_not_cast_is_lifted() {
        let sun = v3(1.0, -1.0, 0.0).norm();
        let mut d = disc(140);
        // Darken a patch the flat field's sun lights.
        for y in 10..20 {
            for x in 10..20 {
                d.tiles[0].rgba[4 * (y * 64 + x)] = 70;
            }
        }
        let (x, z) = (50.0 + 15.0 / 64.0 * 900.0, 950.0 - 15.0 / 64.0 * 900.0);
        let s = as_shadow(&d, &bake_grey(&d, &field(false), sun, 128, 80.0 / 128.0, 1), 128);
        let g = grey_at(&s, x, z);
        assert!(g > 120.0, "{g}");
    }

    #[test]
    fn a_bake_is_cached() {
        let dir = std::env::temp_dir().join(format!("terrain_shadow_cache_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (d, f, sun) = (disc(140), field(true), v3(1.0, -1.0, 0.0).norm());
        let (a, cached) = load_or_bake(Some(&dir), "t", &d, &f, sun, 64, 0.6, 2);
        assert!(!cached);
        let (b, cached) = load_or_bake(Some(&dir), "t", &d, &f, sun, 64, 0.6, 2);
        assert!(cached);
        assert_eq!(a.tiles[0].rgba, b.tiles[0].rgba);
        // Another sun bakes afresh, and the old one goes.
        let (_, cached) = load_or_bake(Some(&dir), "t", &d, &f, v3(-1.0, -1.0, 0.0).norm(), 64, 0.6, 2);
        assert!(!cached);
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
