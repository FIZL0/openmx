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

// ---- a mod track's ground, from its terrain's own pages (MD4) --------------------------------------
//
// MX vs. ATV paints its maps' ground the way MXU paints its nationals: the `.trn` carries one aerial
// picture of the whole map cut into 256x256 pages (`game_sim::track::Terrain::textures`), and the
// ground's material, `TerrainPlain.psm`, lays a detail map over it. Which page a leaf draws is the
// original's own lookup (MXU's terrain renderer, read in the journal): a leaf's 4x4 block of leaves,
// within its parent, is one page; the parent node's sixteen-word table names it, the word being twice
// the page's number; and the leaf takes the quarter of that page its own place in the block says.
// Each page keeps an 8-pixel border that repeats its neighbours' edges, so the 64 samples a page
// covers map onto its middle 240 pixels: `1/32 + 15/16 x`. Across all 56 MX vs. ATV maps the pixels
// either side of a page seam agree exactly that way and by 32 levels a channel without it.
//
// The detail map is drawn the way its `.psm` blends it, `(Cs - Cd) * As + Cd`: a grey a little darker
// than the dirt, pressed in by the map's alpha. It repeats once every `DetailMapTilingMultiplier`
// samples, which is the reading V1 gave the scene's number and a choice by eye — see [`Pages`].

/// How many leaves a page covers along each side.
const PAGE_LEAVES: u32 = 4;
/// Samples a leaf covers along each side, and so samples a page covers: `PAGE_LEAVES` of them.
const LEAF: u32 = 16;
/// The original's mapping of a page's 64 samples onto its pixels: everything but an 8-pixel border.
const PAGE_BORDER: f32 = 1.0 / 32.0;
const PAGE_INNER: f32 = 15.0 / 16.0;

/// A track's ground as MX vs. ATV draws it: the terrain's pages, which leaf takes which, and the
/// detail map over them.
#[derive(Clone, Debug)]
pub struct Pages {
    /// The pages, through the terrain's palette, opaque.
    pub pages: Vec<Texture>,
    /// Leaves across (x) and along (z).
    pub across: u32,
    pub along: u32,
    /// Each leaf's page, row by row along z; `None` where the terrain has no leaf or its table names
    /// no page.
    pub page_of: Vec<Option<u16>>,
    /// World units between two samples.
    pub spacing: f32,
    /// The detail map, as a `.pst` decodes it (its alpha doubled), and the world units one repeat of it
    /// covers.
    pub detail: Option<Texture>,
    pub detail_repeat: f32,
}

impl Pages {
    /// The pages of a terrain `across` x `along` leaves, `spacing` feet a sample; `None` when it has no
    /// pages, or no palette to paint them through, or a page that is not one byte a pixel.
    pub fn of(terrain: &game_sim::track::Terrain, spacing: f32, (across, along): (u32, u32)) -> Option<Pages> {
        let palette = terrain.palette()?;
        if terrain.textures.is_empty() || terrain.levels == 0 {
            return None;
        }
        let mut colours = [[0u8; 4]; 256];
        for (i, c) in colours.iter_mut().enumerate() {
            let p = 4 * crate::pst::unswizzle(i);
            *c = [palette[p], palette[p + 1], palette[p + 2], 255];
        }
        let mut pages = Vec::with_capacity(terrain.textures.len());
        for t in &terrain.textures {
            let level = t.levels.first()?;
            let (w, h) = (level.width as usize, level.height as usize);
            if w == 0 || h == 0 || level.pixels.len() != w * h {
                return None;
            }
            pages.push(Texture { width: w, height: h, rgba: level.pixels.iter().flat_map(|&p| colours[p as usize]).collect() });
        }
        // V84: replaced as one set, so the pages stay the one size their array is.
        let pages = crate::replace::swap_all(pages);
        let mut page_of = vec![None; (across * along) as usize];
        for lz in 0..along {
            for lx in 0..across {
                page_of[(lz * across + lx) as usize] = leaf_page(terrain, lx, lz).filter(|&p| (p as usize) < pages.len());
            }
        }
        Some(Pages { pages, across, along, page_of, spacing, detail: None, detail_repeat: spacing * 8.0 })
    }

    /// Lay a detail map over the pages, repeating every `multiplier` samples (the scene's
    /// `DetailMapTilingMultiplier`).
    pub fn with_detail(mut self, detail: Option<Texture>, multiplier: f32) -> Pages {
        self.detail = detail.filter(|d| d.width > 0 && d.height > 0);
        self.detail_repeat = self.spacing * multiplier.max(1.0);
        self
    }

    /// Which page a point of the world is painted from, and where on it: `None` off the leaves, and
    /// where a leaf has no page.
    pub fn page_at(&self, x: f32, z: f32) -> Option<(usize, [f32; 2])> {
        let (sx, sz) = (x / self.spacing, z / self.spacing);
        let (lx, lz) = ((sx / LEAF as f32).floor(), (sz / LEAF as f32).floor());
        if lx < 0.0 || lz < 0.0 || lx >= self.across as f32 || lz >= self.along as f32 {
            return None;
        }
        let (lx, lz) = (lx as u32, lz as u32);
        let page = self.page_of[(lz * self.across + lx) as usize]?;
        let on_page = |l: u32, s: f32| {
            let samples = (l % PAGE_LEAVES * LEAF) as f32 + (s - (l * LEAF) as f32);
            PAGE_BORDER + PAGE_INNER * samples / (PAGE_LEAVES * LEAF) as f32
        };
        Some((page as usize, [on_page(lx, sx), on_page(lz, sz)]))
    }

    /// The ground's colour at a point of the world before any light, or `None` where no page covers it.
    pub fn paint(&self, x: f32, z: f32) -> Option<[u8; 3]> {
        let (page, [u, v]) = self.page_at(x, z)?;
        let t = self.pages[page].sample_smooth(u, v);
        let mut c = [t[0] as f32, t[1] as f32, t[2] as f32];
        if let Some(d) = &self.detail {
            let s = d.sample_smooth(x / self.detail_repeat, z / self.detail_repeat);
            c = detail_over(c, s);
        }
        Some(c.map(|c| c.clamp(0.0, 255.0) as u8))
    }

    /// The detail map a track's archives hold: the one `TerrainPlain.psm` names as its second texture,
    /// or else `<stem>_d.pst`, the name every MX vs. ATV map gives it.
    pub fn load_detail(archives: &[crate::disc::Res], stem: &str) -> Option<Texture> {
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        let named = read("terrainplain.psm").and_then(|psm| {
            let sections = crate::ini::parse(&crate::ini::text(&psm));
            sections.iter().find_map(|s| s.get("TextureMap_2").map(|n| n.trim().to_string())).filter(|n| !n.is_empty())
        });
        let name = named.unwrap_or_else(|| format!("{stem}_d"));
        Texture::parse(&read(&format!("{name}.pst"))?).ok().map(crate::replace::swap)
    }
}

/// The detail map's texel `d` (alpha as a `.pst` decodes it, doubled) over the colour `c`, as the
/// ground's material blends it: `(Cs - Cd) * As + Cd`, alpha 128 being one.
pub fn detail_over(c: [f32; 3], d: [u8; 4]) -> [f32; 3] {
    let a = d[3] as f32 * 0.5 / 128.0;
    [0, 1, 2].map(|k| c[k] + (d[k] as f32 - c[k]) * a)
}

/// The scene's `[Environment]` `DetailMapTilingMultiplier`, which the original reads as a whole
/// number; 8, its default and every MXU track's, where the scene has none.
pub fn detail_multiplier(scene: &str) -> f32 {
    crate::ini::parse(scene)
        .iter()
        .filter(|s| s.name.eq_ignore_ascii_case("Environment"))
        .find_map(|s| s.number("DetailMapTilingMultiplier"))
        .map_or(8.0, |m| m.trunc().max(1.0))
}

/// The page leaf (`lx`, `lz`) draws, by the original's lookup: its parent's table, at its 4x4 block
/// within the parent, holds twice the page's number. `None` where the terrain has no such leaf.
fn leaf_page(terrain: &game_sim::track::Terrain, lx: u32, lz: u32) -> Option<u16> {
    let mut node = terrain.root()?;
    let (mut cx, mut cz) = (lx, lz);
    let mut shift = (terrain.levels - 1) * 4;
    for _ in 1..terrain.levels {
        if cx >> shift >= LEAF || cz >> shift >= LEAF {
            return None;
        }
        node = terrain.node(terrain.child(&node, cx >> shift, cz >> shift)?)?;
        let mask = (1 << shift) - 1;
        cx &= mask;
        cz &= mask;
        shift -= 4;
    }
    terrain.child(&node, cx, cz)?;
    let word = node.unread.table[((cz / PAGE_LEAVES) * PAGE_LEAVES + cx / PAGE_LEAVES) as usize];
    u16::try_from(word / 2).ok()
}

// ---- a world's ground across its joins (MD9) -------------------------------------------------------
//
// A world (MD8) is several maps' terrains side by side with join ground between them. Each map keeps
// its own look inside the cells it owns; on a join, which has no pages of its own, the ground is
// every near map's paint blended by how near the map is, so the join starts as the map behind it and
// ends as the map ahead, with no step anywhere. A map's paint is read past its edge by mirroring the
// point back inside it: the mirror is continuous at the edge, so the join begins on exactly the
// map's last pixel and carries its texture on, not one leaf's quarter-page repeated. Nothing in the
// original covers several maps; this is by eye.

/// Feet from a map's ground within which it still tints a join: past it a map plays no part.
pub const BLEND_REACH: f32 = 1500.0;

/// How strongly dirt holds where no map is near: a map `BLEND_REACH / 2` away outweighs it about
/// two thousand times, so the dirt shows only in a hole well clear of every map.
pub const DIRT_WEIGHT: f32 = 1.0 / (BLEND_REACH * BLEND_REACH * 2048.0);

/// The world rectangles the maps of a world own, `[x0, z0, x1, z1]` in feet, half-open, in the
/// world's order (a point two claim is the first's), and how each tints the ground and the light
/// where none owns it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Regions {
    pub owned: Vec<[f32; 4]>,
}

impl Regions {
    /// The map that owns a point, if one does.
    pub fn owner(&self, x: f32, z: f32) -> Option<usize> {
        self.owned.iter().position(|r| x >= r[0] && x < r[2] && z >= r[1] && z < r[3])
    }

    /// Each map's weight at a point, not normalised: 1 for the owner and 0 for every other where a
    /// map owns it, and otherwise `(1/d - 1/BLEND_REACH)²` of the distance `d` to each map's ground,
    /// which grows without bound at a map's edge and is 0 from `BLEND_REACH` out.
    pub fn weights(&self, x: f32, z: f32) -> Vec<f32> {
        if let Some(o) = self.owner(x, z) {
            return (0..self.owned.len()).map(|i| if i == o { 1.0 } else { 0.0 }).collect();
        }
        self.owned
            .iter()
            .map(|r| {
                let d = distance_to(r, x, z).max(1e-3);
                (1.0 / d - 1.0 / BLEND_REACH).max(0.0).powi(2)
            })
            .collect()
    }

    /// Each map's share of the light and the sky at a point: [`Regions::weights`] summing to one, or
    /// the nearest map's alone where none is within reach.
    pub fn shares(&self, x: f32, z: f32) -> Vec<f32> {
        let mut w = self.weights(x, z);
        let total: f32 = w.iter().sum();
        if total > 0.0 {
            w.iter_mut().for_each(|w| *w /= total);
        } else if let Some(near) = (0..self.owned.len()).min_by(|&a, &b| {
            distance_to(&self.owned[a], x, z).total_cmp(&distance_to(&self.owned[b], x, z))
        }) {
            w[near] = 1.0;
        }
        w
    }

    /// A point read in map `i`'s rectangle: itself inside it, mirrored back in across the edge past it.
    pub fn mirrored(&self, i: usize, x: f32, z: f32) -> (f32, f32) {
        let r = self.owned[i];
        (mirror(x, r[0], r[2]), mirror(z, r[1], r[3]))
    }
}

/// How far a point is from a rectangle, 0 inside it.
fn distance_to(r: &[f32; 4], x: f32, z: f32) -> f32 {
    let dx = (r[0] - x).max(x - r[2]).max(0.0);
    let dz = (r[1] - z).max(z - r[3]).max(0.0);
    (dx * dx + dz * dz).sqrt()
}

/// `v` reflected into `lo..hi` across the edge it is past, kept a hair inside the far edge.
fn mirror(v: f32, lo: f32, hi: f32) -> f32 {
    let top = (hi - 1e-3).max(lo);
    if v < lo {
        (lo + (lo - v)).min(top)
    } else if v > top {
        (top - (v - top)).max(lo)
    } else {
        v
    }
}

/// One map of a world as its ground is painted: where its terrain's sample (0, 0) is, and its pages.
#[derive(Clone, Debug)]
pub struct PaintTile {
    pub at: [f32; 2],
    /// Its own pages, or `None` for a map painted otherwise (a splat) or not at all: dirt.
    pub pages: Option<Pages>,
}

/// A world's ground paint: each map's pages over the cells it owns, blended across the joins.
#[derive(Clone, Debug)]
pub struct WorldPaint {
    pub regions: Regions,
    pub tiles: Vec<PaintTile>,
    /// The colour (0..255) where no map's paint reaches.
    pub dirt: [f32; 3],
}

impl WorldPaint {
    /// Map `i`'s colour before any light at a world point read in its rectangle, dirt where it has
    /// no page.
    fn tile_paint(&self, i: usize, x: f32, z: f32) -> [f32; 3] {
        let (x, z) = self.regions.mirrored(i, x, z);
        let t = &self.tiles[i];
        t.pages
            .as_ref()
            .and_then(|p| p.paint(x - t.at[0], z - t.at[1]))
            .map_or(self.dirt, |c| c.map(f32::from))
    }

    /// The ground's colour before any light at a point of the world (0..255): the owner's own paint,
    /// or on a join every near map's blended with a trace of dirt.
    pub fn paint(&self, x: f32, z: f32) -> [f32; 3] {
        if let Some(o) = self.regions.owner(x, z) {
            return self.tile_paint(o, x, z);
        }
        let w = self.regions.weights(x, z);
        let total: f32 = w.iter().sum::<f32>() + DIRT_WEIGHT;
        let mut c = self.dirt.map(|d| d * DIRT_WEIGHT / total);
        for (i, &w) in w.iter().enumerate() {
            if w / total > 1.0 / 512.0 {
                let t = self.tile_paint(i, x, z);
                (0..3).for_each(|k| c[k] += t[k] * w / total);
            }
        }
        c
    }

    /// Where each map's pages go on the GPU, inside a device's limits: each map's pages in the array
    /// of their own size (MD31: a disc map's baked pages are smaller than a mod map's own, and one
    /// array the size of the first map's drew every map after it at that size), up to
    /// [`PAGE_ARRAYS`] sizes of `max_layers` layers each; their leaf images stacked in one picture no
    /// taller than `max_rows`; and one detail layer each. A map that does not fit is drawn in dirt.
    pub fn pack(&self, max_layers: u32, max_rows: u32) -> Vec<Option<Packed>> {
        let (mut sizes, mut layers): (Vec<(u32, u32)>, Vec<u32>) = (Vec::new(), Vec::new());
        let (mut rows, mut details) = (0u32, 0u32);
        self.tiles
            .iter()
            .map(|t| {
                let p = t.pages.as_ref()?;
                let n = p.pages.len() as u32;
                let size = page_size(p);
                let array = sizes.iter().position(|&s| s == size).unwrap_or(sizes.len());
                let used = layers.get(array).copied().unwrap_or(0);
                if n == 0 || array >= PAGE_ARRAYS || used + n > max_layers || rows + p.along > max_rows || p.across > max_rows {
                    return None;
                }
                if array == sizes.len() {
                    sizes.push(size);
                    layers.push(0);
                }
                let slot = Packed { array: array as u32, first_page: used, row: rows, detail: p.detail.as_ref().map(|_| details) };
                layers[array] += n;
                rows += p.along.max(1);
                details += u32::from(p.detail.is_some());
                Some(slot)
            })
            .collect()
    }

    /// The page size of each array [`WorldPaint::pack`] made, by the array's number.
    pub fn array_sizes(&self, packed: &[Option<Packed>]) -> Vec<(u32, u32)> {
        let mut sizes = Vec::new();
        for (t, at) in self.tiles.iter().zip(packed) {
            if let (Some(p), Some(at)) = (&t.pages, at) {
                if at.array as usize == sizes.len() {
                    sizes.push(page_size(p));
                }
            }
        }
        sizes
    }
}

/// How many page sizes a world's paint holds at once ([`WorldPaint::pack`]): a disc map's baked
/// pages, a mod map's own, and room for two more.
pub const PAGE_ARRAYS: usize = 4;

/// A map's page size, its first page's: every page of a terrain is one size, and one that is not is
/// resampled to the first's.
pub fn page_size(p: &Pages) -> (u32, u32) {
    p.pages.first().map_or((1, 1), |t| (t.width.max(1) as u32, t.height.max(1) as u32))
}

/// Where one map's pages sit in a world's GPU paint ([`WorldPaint::pack`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packed {
    /// The page array its pages are in: one array a page size.
    pub array: u32,
    /// Its first page's layer in that array.
    pub first_page: u32,
    /// The row its leaf image starts on.
    pub row: u32,
    /// Its detail map's layer, if it has one.
    pub detail: Option<u32>,
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
