//! A terrain file written from a height function: the `.trn` layout [`crate::track::Terrain`] reads
//! (`PLN` version 7, no palette, no textures), one tree level, every leaf its own block. The world
//! tests ride generated tiles and joins with it, so no byte of their ground is off a disc.
//!
//! Host arithmetic throughout: it writes a file, and the file is what the port then reads.

use crate::track::{BLOCK_BYTES, BLOCK_SAMPLES, TREE_ARITY};

/// Raw height units a foot of world height is written in.
pub const RAW_PER_FOOT: f64 = 64.0;

/// A terrain of `cells.0 × cells.1` root cells (at most 16 each), one 16-sample leaf a cell, at
/// `spacing` feet a sample, with each sample's world height `height(sx, sz)` in feet (0 to 1000) and
/// surface `surface(sx, sz)`. The normals are the heights' central differences.
pub fn terrain(
    name: &str,
    cells: (u32, u32),
    spacing: f32,
    height: impl Fn(u32, u32) -> f64,
    surface: impl Fn(u32, u32) -> u8,
) -> Vec<u8> {
    terrain_painted(name, cells, spacing, height, surface, None)
}

/// [`terrain`], with a palette and one 256 x 256 texture page (format 39, one byte a pixel) every
/// leaf draws, as MX vs. ATV's terrains carry theirs (MD4): `page(px, py)` is the pixel's colour, at
/// most 255 of them told apart. The world look's tests (MD9) paint generated maps with it.
pub fn terrain_painted(
    name: &str,
    cells: (u32, u32),
    spacing: f32,
    height: impl Fn(u32, u32) -> f64,
    surface: impl Fn(u32, u32) -> u8,
    page: Option<&dyn Fn(u32, u32) -> [u8; 3]>,
) -> Vec<u8> {
    assert!(cells.0 >= 1 && cells.1 >= 1 && cells.0 <= TREE_ARITY && cells.1 <= TREE_ARITY);
    let word = |out: &mut Vec<u8>, w: u32| out.extend_from_slice(&w.to_le_bytes());
    // World height = raw × height_scale × spacing.
    let height_scale = (1.0 / (RAW_PER_FOOT * f64::from(spacing))) as f32;
    let raw = |sx: u32, sz: u32| (height(sx, sz) * RAW_PER_FOOT).round().clamp(0.0, 65535.0) as u16;
    let blocks = cells.0 * cells.1;

    let mut out = Vec::new();
    out.extend_from_slice(b"PLN\0");
    out.push(b'7');
    out.extend_from_slice(b"ps2\0");
    let mut label = [0u8; 32];
    let n = name.len().min(31);
    label[..n].copy_from_slice(&name.as_bytes()[..n]);
    out.extend_from_slice(&label);
    // The page's pixels as palette indices, and the palette. Indices are written where the GS's
    // swizzle of the palette puts them back: bits 3 and 4 swapped.
    let swizzle = |i: usize| (i & !0x18) | ((i & 0x08) << 1) | ((i & 0x10) >> 1);
    let mut colours: Vec<[u8; 3]> = Vec::new();
    let pixels: Vec<u8> = match page {
        Some(page) => (0..65536u32)
            .map(|i| {
                let c = page(i % 256, i / 256);
                let n = colours.iter().position(|&k| k == c).unwrap_or_else(|| {
                    colours.push(c);
                    colours.len() - 1
                });
                assert!(n < 256, "a generated page of more than 256 colours");
                n as u8
            })
            .collect(),
        None => Vec::new(),
    };
    if page.is_some() {
        word(&mut out, 1);
        let mut palette = [0u8; 1024];
        for (i, c) in colours.iter().enumerate() {
            let at = 4 * swizzle(i);
            palette[at..at + 3].copy_from_slice(c);
            palette[at + 3] = 0x80;
        }
        out.extend_from_slice(&palette);
    } else {
        word(&mut out, 0); // no palette
    }
    word(&mut out, spacing.to_bits());
    word(&mut out, 1); // one level
    let tree_slot = out.len();
    word(&mut out, 0);
    word(&mut out, 0); // no words in front of the blocks
    word(&mut out, blocks);
    let pages = u32::from(page.is_some());
    word(&mut out, pages); // the texture table
    word(&mut out, pages); // and the textures stored
    let table_at = out.len();
    if page.is_some() {
        word(&mut out, 0);
    }
    for cz in 0..cells.1 {
        for cx in 0..cells.0 {
            let at = |x: u32, z: u32| (cx * 16 + x, cz * 16 + z);
            let start = out.len();
            word(&mut out, 0); // base
            for z in 0..BLOCK_SAMPLES {
                for x in 0..20 {
                    let h = if x < BLOCK_SAMPLES { let (sx, sz) = at(x, z); raw(sx, sz) } else { 0 };
                    out.extend_from_slice(&h.to_le_bytes());
                }
            }
            for z in 0..BLOCK_SAMPLES {
                for x in 0..BLOCK_SAMPLES {
                    let (sx, sz) = at(x, z);
                    let h = |sx: i64, sz: i64| height(sx.max(0) as u32, sz.max(0) as u32);
                    let (sx, sz) = (i64::from(sx), i64::from(sz));
                    let dx = (h(sx + 1, sz) - h(sx - 1, sz)) / (2.0 * f64::from(spacing));
                    let dz = (h(sx, sz + 1) - h(sx, sz - 1)) / (2.0 * f64::from(spacing));
                    let len = (dx * dx + 1.0 + dz * dz).sqrt();
                    for c in [-dx / len, 1.0 / len, -dz / len] {
                        out.push((c * 127.0).round() as i8 as u8);
                    }
                }
            }
            out.extend(std::iter::repeat_n(0u8, 289));
            for z in 0..BLOCK_SAMPLES {
                for x in 0..BLOCK_SAMPLES {
                    let (sx, sz) = at(x, z);
                    out.push(surface(sx, sz) & 0x7f);
                }
            }
            debug_assert_eq!(out.len() - start, BLOCK_BYTES);
        }
    }
    if page.is_some() {
        let at = out.len() as u32;
        out[table_at..table_at + 4].copy_from_slice(&at.to_le_bytes());
        for w in [39, 1, 256, 256, 65536, 0] {
            word(&mut out, w);
        }
        out.extend_from_slice(&pixels);
    }
    let tree_at = out.len() as u32;
    out[tree_slot..tree_slot + 4].copy_from_slice(&tree_at.to_le_bytes());
    let node = |out: &mut Vec<u8>, block: u16, children_at: u32| {
        for f in [1.0f32, height_scale, 0.0, 1000.0] {
            out.extend_from_slice(&f.to_bits().to_le_bytes());
        }
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&block.to_le_bytes());
        out.extend_from_slice(&[0; 4 + 64 + 4]);
        out.push(1);
        out.extend_from_slice(&children_at.to_le_bytes());
    };
    let children_at = tree_at + 97;
    let leaves_at = children_at + 256 * 4;
    node(&mut out, 0, children_at);
    for cz in 0..TREE_ARITY {
        for cx in 0..TREE_ARITY {
            let w = if cx < cells.0 && cz < cells.1 { leaves_at + (cz * cells.0 + cx) * 97 } else { 0 };
            word(&mut out, w);
        }
    }
    for b in 0..blocks {
        node(&mut out, b as u16, 0);
    }
    out
}
