//! `.trn` — the ground a track is raced on: one height field, one palette, one ground texture.
//!
//! ```text
//! 0     "PLN\0" '7' "ps2\0" "UnCompressedPSXPal8\0"   magic, then zeros
//! 45    256 x RGBA                 the palette the ground texture is painted through
//! 1069  f32 spacing                world units between two neighbouring height samples
//! 1073  u32 levels, u32 tree       the tree's depth, and where its root node starts (MD3)
//! 1081  u32 columns, u32 rows      height samples across and along, 257 x 257 on a supercross track
//! 1089  (patches + 1) x u32        where each patch's block starts, relative to the end of this table
//! …     patches x 2129 bytes       one block per 16 x 16 quads
//! …     1 MiB                      the ground texture, eight-bit pixels into the palette above
//! …     the rest                   388-byte records, not decoded
//! ```
//!
//! A patch block holds, in its first 696 bytes, a 16-byte header and then a 17 x 17 grid of `u16`
//! heights — seventeen because the patches share their edges, so a 256-quad track is 257 samples
//! wide. Each row is 40 bytes: the seventeen samples and six bytes that are always zero. The
//! remaining 1433 bytes of a block are not decoded here; they look like per-vertex normals and the
//! per-vertex choice of ground texture, neither of which the first draw needs.
//!
//! A height word is a fraction of a per-track band, and the band is the file's own (MD3): the tree's
//! root node, at the header's tree offset, carries it as its second float — the `height_scale`
//! `game_sim::track::TerrainNode` reads, a raw word to a local height — and a local height is a world
//! one times the sample spacing, as the port's own terrain query has it. So a word is
//! `height_scale × spacing` feet, from zero: 1/485 of a foot on the first supercross track. With it the
//! disc's bales sit 1.2 to 1.6 feet above the dirt on every supercross track, which is where a bale's
//! origin is (`the_files_band_seats_the_bales`). Before MD3 the band was fitted to those bales, which
//! put it a tenth out with an offset of its own; this reader still walks the patches in file order
//! rather than through the tree, and takes every node at full relief.

use crate::mat::{V3, v3};
use crate::pst::Texture;
use crate::raster::{Frame, Shade};
use crate::scene::Scene;
use crate::world::{Camera, Lighting};

/// The height word the editors leave flat ground at: two thirds of `u16`.
pub const FLAT: u16 = 0xAAAA;
/// Bytes per patch block.
const BLOCK: usize = 2129;
/// Quads along one side of a patch; a patch therefore holds `PATCH + 1` samples per row.
const PATCH: usize = 16;

/// A track's ground: a height field, and the texture it is painted with.
#[derive(Clone, Debug)]
pub struct Terrain {
    /// Height samples across (x) and along (z). One more than the number of quads.
    pub cols: usize,
    pub rows: usize,
    /// World units between neighbouring samples.
    pub spacing: f32,
    /// The height words, row-major: `raw[row * cols + col]`.
    pub raw: Vec<u16>,
    /// A height word becomes a world height as `raw * scale + base`: the file's band, so `base` is 0.
    pub scale: f32,
    pub base: f32,
    /// The ground texture, as one tile of the disc's strip.
    pub ground: Texture,
    /// World units one repeat of [`Terrain::ground`] covers.
    pub tiling: f32,
}

impl Terrain {
    /// Parse a `.trn`, its heights in the file's own band.
    pub fn parse(d: &[u8]) -> Result<Terrain, String> {
        if d.len() < 1093 || &d[..4] != b"PLN\0" {
            return Err("not a .trn".to_string());
        }
        let spacing = f32::from_bits(le32(d, 1069));
        // The root node: two floats, the node's relief and then the band.
        let tree = le32(d, 1077) as usize;
        if tree.checked_add(8).is_none_or(|end| end > d.len()) {
            return Err(format!("the tree at {tree} is past the end of a {}-byte file", d.len()));
        }
        let band = f32::from_bits(le32(d, tree + 4));
        let cols = le32(d, 1081) as usize;
        let rows = le32(d, 1085) as usize;
        if !(2..=4097).contains(&cols) || !(2..=4097).contains(&rows) {
            return Err(format!("a {cols} x {rows} height field makes no sense"));
        }
        if (cols - 1) % PATCH != 0 || (rows - 1) % PATCH != 0 {
            // The off-road tracks are not a whole number of patches across and are laid out some
            // other way; the first draw is a supercross track, so say so instead of guessing.
            return Err(format!("{cols} x {rows} is not a whole number of patches"));
        }
        let (pcols, prows) = ((cols - 1) / PATCH, (rows - 1) / PATCH);
        let patches = pcols * prows;
        let table = 1089;
        let base = table + 4 * (patches + 1);
        if base + 4 > d.len() {
            return Err("the patch table runs off the end".to_string());
        }
        let at = |p: usize| base + le32(d, table + 4 * p) as usize;
        for p in 0..=patches {
            // Every block is the same size and they are stored in order, which is what makes the
            // table checkable: a file that disagrees is a file this code has misread.
            if at(p) != at(0) + p * BLOCK {
                return Err(format!("patch {p} is not where the table says it is"));
            }
        }
        if at(patches) + BLOCK > d.len() {
            return Err("the patches run off the end".to_string());
        }

        let mut raw = vec![FLAT; cols * rows];
        for row in 0..rows {
            for col in 0..cols {
                // The last sample of a row belongs to the last patch, as its seventeenth.
                let (pr, r) = split(row, prows);
                let (pc, c) = split(col, pcols);
                let o = at(pr * pcols + pc) + 16 + 40 * r + 2 * c;
                raw[row * cols + col] = u16::from_le_bytes([d[o], d[o + 1]]);
            }
        }

        let ground = ground_texture(d, at(patches) + BLOCK)?;
        Ok(Terrain {
            cols,
            rows,
            spacing,
            raw,
            scale: band * spacing,
            base: 0.0,
            ground,
            // The scene's `DetailMapTilingMultiplier`, which is 8 on every track on this disc.
            tiling: spacing * 8.0,
        })
    }

    /// How high the scene's bales stand above this ground: how many there are on it, and the median
    /// of each one's origin less the ground under it. A bale's origin is not at its feet, so a band
    /// read right puts this a foot or so up, the same on every track; one read wrong would scatter
    /// it. The check of [`Terrain::parse`]'s band against the scene, not a part of it.
    pub fn props_rest(&self, scene: &Scene) -> Option<(usize, f32)> {
        let mut above: Vec<f32> = scene
            .models
            .iter()
            .filter(|p| p.model.to_ascii_lowercase().contains("bale"))
            .filter_map(|p| {
                let (col, row) = self.sample_of(p.pos.x, p.pos.z)?;
                Some(p.pos.y - self.height(col, row))
            })
            .collect();
        if above.is_empty() {
            return None;
        }
        above.sort_by(f32::total_cmp);
        Some((above.len(), above[above.len() / 2]))
    }

    /// The world height of one sample.
    pub fn height(&self, col: usize, row: usize) -> f32 {
        let col = col.min(self.cols - 1);
        let row = row.min(self.rows - 1);
        self.raw[row * self.cols + col] as f32 * self.scale + self.base
    }

    /// The ground height under a point, interpolated across the quad it falls in. This is what puts
    /// the bike on the dirt rather than through it.
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let (u, v) = (x / self.spacing, z / self.spacing);
        let (c, r) = (
            (u.floor() as isize).clamp(0, self.cols as isize - 2) as usize,
            (v.floor() as isize).clamp(0, self.rows as isize - 2) as usize,
        );
        let (fu, fv) = ((u - c as f32).clamp(0.0, 1.0), (v - r as f32).clamp(0.0, 1.0));
        let h00 = self.height(c, r);
        let h10 = self.height(c + 1, r);
        let h01 = self.height(c, r + 1);
        let h11 = self.height(c + 1, r + 1);
        let top = h00 + (h10 - h00) * fu;
        let bottom = h01 + (h11 - h01) * fu;
        top + (bottom - top) * fv
    }

    /// The corner of the world a point sits at, or `None` if it is off the terrain.
    fn sample_of(&self, x: f32, z: f32) -> Option<(usize, usize)> {
        let col = (x / self.spacing).round();
        let row = (z / self.spacing).round();
        if col < 0.0 || row < 0.0 || col >= self.cols as f32 || row >= self.rows as f32 {
            return None;
        }
        Some((col as usize, row as usize))
    }

    /// The upward normal at a sample, from the slope of its neighbours.
    fn normal(&self, col: usize, row: usize) -> V3 {
        let left = self.height(col.saturating_sub(1), row);
        let right = self.height(col + 1, row);
        let back = self.height(col, row.saturating_sub(1));
        let front = self.height(col, row + 1);
        v3(left - right, 2.0 * self.spacing, back - front).norm()
    }

    /// The world position of a sample.
    fn point(&self, col: usize, row: usize) -> V3 {
        v3(
            col as f32 * self.spacing,
            self.height(col, row),
            row as f32 * self.spacing,
        )
    }
}

/// Which patch a sample belongs to, and where in it: the last sample of the field is the extra,
/// seventeenth sample of the last patch.
fn split(index: usize, patches: usize) -> (usize, usize) {
    let p = (index / PATCH).min(patches - 1);
    (p, index - p * PATCH)
}

/// The ground texture: one 256 x 256 tile out of the megabyte that follows the patches.
///
/// The megabyte is 256 pixels wide and 4096 tall — a 1024-wide reading of it repeats four times
/// across, which is how the width was settled. Whether its sixteen tiles are sixteen ground types or
/// one tall image is not settled, and neither is which part of it belongs under which quad: the
/// first draw repeats the first tile across the whole track, at the scene's own detail-map tiling.
fn ground_texture(d: &[u8], at: usize) -> Result<Texture, String> {
    if at + 256 * 256 > d.len() {
        return Err("the ground texture runs off the end".to_string());
    }
    let mut palette = [[0u8; 4]; 256];
    for (i, slot) in palette.iter_mut().enumerate() {
        // The same palette order as a `.pst`, and opaque throughout.
        let p = 45 + 4 * crate::pst::unswizzle(i);
        *slot = [d[p], d[p + 1], d[p + 2], 255];
    }
    let mut rgba = vec![0u8; 256 * 256 * 4];
    for (i, &p) in d[at..at + 256 * 256].iter().enumerate() {
        rgba[4 * i..4 * i + 4].copy_from_slice(&palette[p as usize]);
    }
    Ok(Texture {
        width: 256,
        height: 256,
        rgba,
    })
}

/// Draw the ground, one quad at a time, and return how many triangles went to the rasteriser.
///
/// Quads whose corners are all behind the camera or all past the fog are dropped before they are
/// transformed, which is the difference between a tenth of a second and a second on a supercross
/// track's hundred and thirty thousand triangles.
pub fn draw_terrain(frame: &mut Frame, terrain: &Terrain, camera: &Camera, light: &Lighting) -> usize {
    let view_projection = camera.view_projection(frame.width, frame.height);
    let forward = camera.at.sub(camera.eye).norm();
    let mut drawn = 0;
    for row in 0..terrain.rows - 1 {
        for col in 0..terrain.cols - 1 {
            let corners = [
                (col, row),
                (col + 1, row),
                (col + 1, row + 1),
                (col, row + 1),
            ];
            let points = corners.map(|(c, r)| terrain.point(c, r));
            let middle = points[0].add(points[2]).scale(0.5);
            let to_middle = middle.sub(camera.eye);
            let distance = to_middle.len();
            if distance > light.fog_far || to_middle.dot(forward) < -2.0 * terrain.spacing {
                continue;
            }
            let shades: Vec<Shade> = corners
                .iter()
                .zip(points.iter())
                .map(|(&(c, r), &p)| {
                    let normal = terrain.normal(c, r);
                    let lambert = (-normal.dot(light.sun)).max(0.0);
                    let mut lit = [0.0f32; 3];
                    for k in 0..3 {
                        lit[k] = light.ambient[k] + light.sun_color[k] * lambert;
                    }
                    let fog = ((p.sub(camera.eye).len() - light.fog_near)
                        / (light.fog_far - light.fog_near).max(1.0))
                    .clamp(0.0, 1.0);
                    Shade {
                        clip: view_projection.clip(p),
                        uv: [p.x / terrain.tiling, p.z / terrain.tiling],
                        light: lit,
                        fog,
                    }
                })
                .collect();
            for tri in [[0, 1, 2], [0, 2, 3]] {
                let shade = [shades[tri[0]], shades[tri[1]], shades[tri[2]]];
                if shade.iter().all(|s| s.clip[3] <= 0.0) {
                    continue;
                }
                frame.triangle(shade, Some(&terrain.ground), light.fog_color);
                drawn += 1;
            }
        }
    }
    drawn
}

// ---- the ground's level of detail (V32) -----------------------------------------------------------
//
// With no fog the whole field is in view, a million cells of it on a freeride track and four times
// that in a four-way split, so the GPU draws the field in square chunks and each chunk at a level of
// detail: level `l` keeps every `2^l`-th corner of the chunk's grid, split two triangles a quad the
// way the full grid is. A chunk's level is the coarsest whose worst height error, seen from the
// camera, is under a pixel ([`lod_level`]), which is what keeps a change of level from being seen.
// Two neighbours may differ by one level at most ([`lod_balance`]), and the finer one bends its edge
// onto the coarser one's ([`lod_indices`]), so the two share every edge and leave no crack.

/// A chunk's sides, as [`lod_indices`]'s `coarser` lists them: the row at the chunk's first `z`, the
/// column at its last `x`, the row at its last `z` and the column at its first `x`.
pub const SIDE_Z0: usize = 0;
pub const SIDE_X1: usize = 1;
pub const SIDE_Z1: usize = 2;
pub const SIDE_X0: usize = 3;

/// The coarsest level a chunk `cells` cells a side has: one quad.
pub fn lod_levels(cells: u32) -> u32 {
    cells.max(1).trailing_zeros()
}

/// A chunk's triangles at `level`, as indices into its `(cells + 1)²` corners, row by row along `x`.
///
/// `coarser[side]` says that the neighbour across that side is drawn one level coarser: every other
/// corner of that edge is then moved onto the one before it, so the edge is the neighbour's own
/// segments, and the triangles that collapse doing so are left out. `cells` is a power of two.
pub fn lod_indices(cells: u32, level: u32, coarser: [bool; 4]) -> Vec<u32> {
    lod_indices_where(cells, level, coarser, |_, _| true)
}

/// [`lod_indices`] for a chunk with holes in it: only the quads whose first corner, `(x, z)` of the
/// grid, `keep` says yes to.
pub fn lod_indices_where(cells: u32, level: u32, coarser: [bool; 4], keep: impl Fn(u32, u32) -> bool) -> Vec<u32> {
    let step = 1u32 << level.min(lod_levels(cells));
    // A chunk drawn as one quad has nothing coarser beside it.
    let coarser = if step >= cells { [false; 4] } else { coarser };
    let row = cells + 1;
    let corner = |i: u32, j: u32| {
        let (mut i, mut j) = (i, j);
        // Odd along a coarser side: onto the even corner before it.
        if (j == 0 && coarser[SIDE_Z0]) || (j == cells && coarser[SIDE_Z1]) {
            if (i / step) % 2 == 1 {
                i -= step;
            }
        }
        if (i == 0 && coarser[SIDE_X0]) || (i == cells && coarser[SIDE_X1]) {
            if (j / step) % 2 == 1 {
                j -= step;
            }
        }
        j * row + i
    };
    let mut out = Vec::with_capacity(((cells / step) * (cells / step) * 6) as usize);
    for j in (0..cells).step_by(step as usize) {
        for i in (0..cells).step_by(step as usize) {
            if !keep(i, j) {
                continue;
            }
            let (a, b, c, d) = (corner(i, j), corner(i + step, j), corner(i + step, j + step), corner(i, j + step));
            for t in [[a, b, c], [a, c, d]] {
                if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
                    out.extend(t);
                }
            }
        }
    }
    out
}

/// How far, in height, each level of a chunk strays from its full grid: for every level, the most any
/// corner of the full grid lies above or below that level's triangles. `heights` is the chunk's
/// `(cells + 1)²` corners, row by row along `x`. Never smaller than a finer level's.
pub fn lod_errors(heights: &[f32], cells: u32) -> Vec<f32> {
    let row = (cells + 1) as usize;
    let h = |i: u32, j: u32| heights[j as usize * row + i as usize];
    let mut out: Vec<f32> = Vec::new();
    for level in 0..=lod_levels(cells) {
        let step = 1u32 << level;
        let mut worst = out.last().copied().unwrap_or(0.0);
        for j0 in (0..cells).step_by(step as usize) {
            for i0 in (0..cells).step_by(step as usize) {
                let (a, b, c, d) = (h(i0, j0), h(i0 + step, j0), h(i0 + step, j0 + step), h(i0, j0 + step));
                for dj in 0..=step {
                    for di in 0..=step {
                        let (u, v) = (di as f32 / step as f32, dj as f32 / step as f32);
                        // The split `lod_indices` makes: (0,0) (1,0) (1,1) below the diagonal, (0,0)
                        // (1,1) (0,1) above it.
                        let flat = if u >= v { a + (b - a) * u + (c - b) * v } else { a + (c - d) * u + (d - a) * v };
                        worst = worst.max((h(i0 + di, j0 + dj) - flat).abs());
                    }
                }
            }
        }
        out.push(worst);
    }
    out
}

/// The coarsest level whose error, `distance` away through a lens of `focal` pixels, is at most
/// `pixels` pixels on screen. Level 0 when the camera is inside the chunk's sphere.
pub fn lod_level(errors: &[f32], distance: f32, focal: f32, pixels: f32) -> u32 {
    if distance <= 0.0 {
        return 0;
    }
    let mut level = 0;
    for (l, e) in errors.iter().enumerate() {
        if e * focal <= pixels * distance {
            level = l as u32;
        } else {
            break;
        }
    }
    level
}

/// Make the levels of a grid of chunks, `across` by `along`, differ by at most one between any two
/// side neighbours, by making the coarser of a pair finer — never coarser, so no chunk's error grows.
pub fn lod_balance(levels: &mut [u32], across: usize, along: usize) {
    loop {
        let mut moved = false;
        for z in 0..along {
            for x in 0..across {
                let here = z * across + x;
                let mut cap = u32::MAX;
                if x > 0 {
                    cap = cap.min(levels[here - 1] + 1);
                }
                if x + 1 < across {
                    cap = cap.min(levels[here + 1] + 1);
                }
                if z > 0 {
                    cap = cap.min(levels[here - across] + 1);
                }
                if z + 1 < along {
                    cap = cap.min(levels[here + across] + 1);
                }
                if levels[here] > cap {
                    levels[here] = cap;
                    moved = true;
                }
            }
        }
        if !moved {
            return;
        }
    }
}

/// Which of a chunk's sides border a coarser neighbour, for [`lod_indices`]: `levels` as
/// [`lod_balance`] left it.
pub fn lod_coarser(levels: &[u32], across: usize, along: usize, x: usize, z: usize) -> [bool; 4] {
    let here = levels[z * across + x];
    let at = |x: usize, z: usize| levels[z * across + x] > here;
    [
        z > 0 && at(x, z - 1),
        x + 1 < across && at(x + 1, z),
        z + 1 < along && at(x, z + 1),
        x > 0 && at(x - 1, z),
    ]
}

/// A number from the file, little-endian like the machine that wrote it.
fn le32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-patch terrain, built the way the disc builds one, to prove the walk through the patch
    /// table and the 40-byte rows: three feet a sample, and a root node whose band is `band`.
    fn one_patch(words: impl Fn(usize, usize) -> u16) -> Vec<u8> {
        one_patch_in(words, 1.0 / 1620.0)
    }

    fn one_patch_in(words: impl Fn(usize, usize) -> u16, band: f32) -> Vec<u8> {
        let mut d = vec![0u8; 1089];
        d[..4].copy_from_slice(b"PLN\0");
        d[1069..1073].copy_from_slice(&3.0f32.to_bits().to_le_bytes());
        d[1081..1085].copy_from_slice(&17u32.to_le_bytes());
        d[1085..1089].copy_from_slice(&17u32.to_le_bytes());
        // One patch, so two offsets: the block, and the end of it.
        d.extend_from_slice(&0u32.to_le_bytes());
        d.extend_from_slice(&(BLOCK as u32).to_le_bytes());
        let mut block = vec![0u8; BLOCK];
        for r in 0..17 {
            for c in 0..17 {
                let at = 16 + 40 * r + 2 * c;
                block[at..at + 2].copy_from_slice(&words(c, r).to_le_bytes());
            }
        }
        d.extend_from_slice(&block);
        d.extend_from_slice(&vec![0u8; BLOCK]); // the end-of-table block
        d.extend_from_slice(&vec![7u8; 256 * 256]); // the ground texture
        // The tree's root: full relief, then the band.
        let tree = d.len() as u32;
        d[1077..1081].copy_from_slice(&tree.to_le_bytes());
        d.extend_from_slice(&1.0f32.to_bits().to_le_bytes());
        d.extend_from_slice(&band.to_bits().to_le_bytes());
        d.extend_from_slice(&[0u8; 89]);
        d
    }

    #[test]
    fn heights_come_out_of_the_patch_grid() {
        let d = one_patch(|c, r| (1000 + 10 * r + c) as u16);
        let t = Terrain::parse(&d).expect("a one-patch terrain");
        assert_eq!((t.cols, t.rows), (17, 17));
        assert_eq!(t.spacing, 3.0);
        assert_eq!(t.raw[0], 1000);
        assert_eq!(t.raw[16], 1016, "the seventeenth sample of the row");
        assert_eq!(t.raw[16 * 17 + 16], 1176, "the far corner");
    }

    #[test]
    fn flat_ground_is_flat_and_sits_where_the_band_says() {
        // A band of 1/1620 a word at 3 feet a sample is a word to 1/540 of a foot, which puts the
        // editors' flat word at 80.9 feet.
        let d = one_patch(|_, _| FLAT);
        let t = Terrain::parse(&d).expect("a flat terrain");
        assert_eq!((t.scale, t.base), ((1.0 / 1620.0) * 3.0, 0.0));
        assert!((t.height_at(7.5, 11.0) - FLAT as f32 / 540.0).abs() < 1e-3);
        // A flat quad points straight up.
        let n = t.normal(3, 3);
        assert!(n.y > 0.999, "{n:?}");
    }

    #[test]
    fn a_terrain_that_is_not_a_whole_number_of_patches_is_refused() {
        let mut d = one_patch(|_, _| FLAT);
        d[1081..1085].copy_from_slice(&18u32.to_le_bytes());
        assert!(Terrain::parse(&d).is_err());
    }

    /// The band is the root node's (MD3), not a fit: a terrain whose root says 0.002 a word at three
    /// feet a sample is 0.006 feet a word from zero, and bales standing a foot and a half above it
    /// say so, whatever the slope.
    #[test]
    fn the_band_is_the_root_nodes_and_the_bales_stand_on_it() {
        let d = one_patch_in(|c, r| (40000 + 100 * (r + c)) as u16, 0.002);
        let t = Terrain::parse(&d).expect("a sloped terrain");
        assert_eq!((t.scale, t.base), (0.002 * 3.0, 0.0));
        let ground = |col: usize, row: usize| (40000 + 100 * (row + col)) as f32 * 0.006;
        assert!((t.height_at(6.0, 6.0) - ground(2, 2)).abs() < 1e-2);
        let mut scene = Scene::parse("[SceneInfo]\nSceneName=test\n");
        for i in 0..40 {
            let (col, row) = (i % 17, i / 17);
            scene.models.push(crate::scene::Placement {
                model: "bale_single_01".to_string(),
                pos: v3(col as f32 * 3.0, ground(col, row) + 1.5, row as f32 * 3.0),
                look: v3(0.0, 0.0, 1.0),
                up: v3(0.0, 1.0, 0.0),
            });
        }
        let (bales, above) = t.props_rest(&scene).expect("bales on it");
        assert_eq!(bales, 40);
        assert!((above - 1.5).abs() < 1e-2, "{above}");
        // A tree offset past the end is a file misread, not a band of zero.
        let mut d = d;
        d[1077..1081].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(Terrain::parse(&d).is_err());
    }

    /// The corners `lod_indices` uses, as (x, z) of the grid.
    fn grid_at(index: u32, cells: u32) -> (i64, i64) {
        ((index % (cells + 1)) as i64, (index / (cells + 1)) as i64)
    }

    /// Twice the signed area, across x and z, of each triangle.
    fn areas(indices: &[u32], cells: u32) -> Vec<i64> {
        indices
            .chunks(3)
            .map(|t| {
                let [a, b, c] = [t[0], t[1], t[2]].map(|i| grid_at(i, cells));
                (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
            })
            .collect()
    }

    /// The segments of a side of the chunk that only one triangle has: what a neighbour must match.
    fn open_edges(indices: &[u32], cells: u32, on_side: impl Fn((i64, i64)) -> bool) -> Vec<((i64, i64), (i64, i64))> {
        let mut count: std::collections::HashMap<((i64, i64), (i64, i64)), u32> = Default::default();
        for t in indices.chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (grid_at(a, cells), grid_at(b, cells));
                *count.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        let mut out: Vec<_> = count.into_iter().filter(|(e, n)| *n == 1 && on_side(e.0) && on_side(e.1)).map(|(e, _)| e).collect();
        out.sort();
        out
    }

    #[test]
    fn every_level_and_stitch_tiles_the_chunk_with_no_gap_or_fold() {
        let cells = 32;
        assert_eq!(lod_levels(cells), 5);
        assert_eq!(lod_indices(cells, 0, [false; 4]).len(), 32 * 32 * 6);
        assert_eq!(lod_indices(cells, 5, [false; 4]).len(), 6);
        for level in 0..=lod_levels(cells) {
            for mask in 0..16u32 {
                let coarser = [mask & 1 != 0, mask & 2 != 0, mask & 4 != 0, mask & 8 != 0];
                let tris = lod_indices(cells, level, coarser);
                let a = areas(&tris, cells);
                // Every triangle wound the same way and none folded over, and together they cover the
                // chunk exactly: nothing missing, nothing twice.
                assert!(a.iter().all(|&a| a > 0), "level {level} mask {mask}: a folded or empty triangle");
                assert_eq!(a.iter().sum::<i64>(), 2 * (cells * cells) as i64, "level {level} mask {mask}");
            }
        }
    }

    #[test]
    fn neighbouring_chunks_share_their_edges() {
        let cells = 16;
        for level in 0..lod_levels(cells) {
            // A finer chunk with a coarser one across its last x, both ways round of each side.
            for (side, mine, theirs) in [
                (SIDE_X1, (|p: (i64, i64)| p.0 == 16) as fn((i64, i64)) -> bool, (|p: (i64, i64)| p.0 == 0) as fn((i64, i64)) -> bool),
                (SIDE_Z1, |p: (i64, i64)| p.1 == 16, |p: (i64, i64)| p.1 == 0),
            ] {
                let mut coarser = [false; 4];
                coarser[side] = true;
                let fine = open_edges(&lod_indices(cells, level, coarser), cells, mine);
                let coarse = open_edges(&lod_indices(cells, level + 1, [false; 4]), cells, theirs);
                // The same segments, the one chunk's side moved onto the other's.
                let along = |e: &((i64, i64), (i64, i64))| if side == SIDE_X1 { (e.0.1, e.1.1) } else { (e.0.0, e.1.0) };
                let fine: Vec<_> = fine.iter().map(along).collect();
                let coarse: Vec<_> = coarse.iter().map(along).collect();
                assert_eq!(fine, coarse, "level {level} side {side}");
                assert_eq!(fine.len(), (cells >> (level + 1)) as usize);
                // And with no stitch the finer side has twice the segments: the crack the stitch closes.
                let open = open_edges(&lod_indices(cells, level, [false; 4]), cells, mine);
                assert_eq!(open.len(), 2 * coarse.len());
            }
        }
    }

    #[test]
    fn the_level_gets_coarser_with_distance() {
        // A bumpy chunk: every other corner a foot up.
        let cells = 16;
        let heights: Vec<f32> = (0..17 * 17).map(|k| if (k % 17 + k / 17) % 2 == 1 { 1.0 } else { 0.0 }).collect();
        let errors = lod_errors(&heights, cells);
        assert_eq!(errors.len(), 5);
        assert_eq!(errors[0], 0.0);
        assert!(errors.windows(2).all(|w| w[0] <= w[1]), "{errors:?}");
        assert!(errors[1] > 0.0);
        let focal = 1000.0;
        let mut last = 0;
        for d in [0.0, 10.0, 100.0, 1000.0, 2000.0, 10_000.0] {
            let l = lod_level(&errors, d, focal, 1.0);
            assert!(l >= last, "{d}: {l} after {last}");
            last = l;
        }
        assert_eq!(lod_level(&errors, 0.0, focal, 1.0), 0);
        assert_eq!(lod_level(&errors, 10.0, focal, 1.0), 0);
        // A foot of error is under a pixel past a thousand feet through a 1000-pixel lens.
        assert_eq!(lod_level(&errors, 2000.0, focal, 1.0), 4);
        // Flat ground is one quad wherever it is.
        let flat = lod_errors(&vec![5.0; 17 * 17], cells);
        assert_eq!(lod_level(&flat, 1.0, focal, 1.0), 4);
    }

    #[test]
    fn balanced_neighbours_differ_by_one_at_most() {
        let (across, along) = (5, 4);
        let mut levels = vec![5; across * along];
        levels[2 * across + 2] = 0;
        lod_balance(&mut levels, across, along);
        for z in 0..along {
            for x in 0..across {
                let l = levels[z * across + x];
                if x + 1 < across {
                    assert!(l.abs_diff(levels[z * across + x + 1]) <= 1);
                }
                if z + 1 < along {
                    assert!(l.abs_diff(levels[(z + 1) * across + x]) <= 1);
                }
            }
        }
        assert_eq!(levels[2 * across + 2], 0);
        assert_eq!(levels[2 * across + 3], 1);
        assert_eq!(lod_coarser(&levels, across, along, 2, 2), [true; 4]);
    }
}
