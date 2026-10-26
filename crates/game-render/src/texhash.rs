//! PCSX2's texture-replacement names (V87), so a texture pack made for the original MX Unleashed in
//! PCSX2 (hash-named PNGs, Tools > Texture Replacement) is drawn as it is. opentennis's
//! `hst-data/src/texhash.rs`, its GS half unchanged and a `.pst` reader in place of its MTL/TIM2 ones.
//!
//! PCSX2 names a texture `<tex>-<clut>-<bits>.png` (`<tex>-<bits>.png` without a palette): `tex` is
//! XXH3-64 of the texture as it sits in GS memory, `clut` XXH3-64 of the palette as 16 or 256 RGBA32
//! entries (raw PS2 alpha, logical order), `bits` PSM | TW << 6 | TH << 10 (| TEXA for 16/24-bit). How
//! the texture bytes are hashed:
//! - 32-bit, 8-bit and 4-bit textures of at least one GS block: every block (256 bytes, in its swizzled
//!   layout), blocks in raster order;
//! - anything smaller than a block, and 16/24-bit: the texture expanded row by row (8-bit indices for
//!   palettes, RGBA32 otherwise).
//!
//! With mipmapping on, each level of the drawn LOD range is hashed after the base into the same digest,
//! and PCSX2 dumps the levels after the base beside it as `<name>-mip<N>.png`.
//!
//! This game's `.pst`s (checked against a PCSX2 2.9 dump of `free06`, V87's journal): 8-bit textures
//! are PSMT8 with the palette's logical order the one [`crate::pst`] decodes by (the file holds it in
//! the GS's CSM1 order), 16-colour ones PSMT4 with the palette in file order, the alpha as the file
//! has it (0x80 opaque), TW/TH the next power of two. A far model is drawn from a smaller level as
//! its base, which PCSX2 names by the run of levels from there ([`Pst::pcsx2_names`]).

use std::path::{Path, PathBuf};
use twox_hash::XxHash3_64;

/// One disc texture as the GS sees it.
pub struct GsTex<'a> {
    pub psm: u8,
    /// log2 of the GS width/height of level 0.
    pub tw: u32,
    pub th: u32,
    /// Raw texel data per mip level, linear rows (4-bit: low nibble first).
    pub levels: Vec<&'a [u8]>,
    /// Logical palette, raw PS2 alpha; empty for direct colour.
    pub clut: &'a [[u8; 4]],
}

const PSMCT32: u8 = 0x00;
const PSMCT24: u8 = 0x01;
const PSMCT16: u8 = 0x02;
const PSMT8: u8 = 0x13;
const PSMT4: u8 = 0x14;

/// Block size in pixels and bits per pixel of the formats the game's textures use.
fn format(psm: u8) -> Option<((u32, u32), u32)> {
    Some(match psm {
        PSMCT32 => ((8, 8), 32),
        PSMCT24 => ((8, 8), 24),
        PSMCT16 => ((16, 8), 16),
        PSMT8 => ((16, 16), 8),
        PSMT4 => ((32, 16), 4),
        _ => return None,
    })
}

/// Position of pixel (x, y) inside its 256-byte GS block, in units of the format's pixel (u32, byte or
/// nibble).
fn in_block(psm: u8, x: u32, y: u32) -> u32 {
    if psm == PSMCT32 {
        return (y >> 1) << 4 | (x >> 1) << 2 | (y & 1) << 1 | (x & 1);
    }
    // 8- and 4-bit blocks are 4 columns of 4 rows; in every other column the half rows swap 4 pixels
    let (c, r) = (y >> 2, y & 3);
    let x8 = if (r >> 1) ^ (c & 1) != 0 { (x + 4) & 7 } else { x & 7 };
    if psm == PSMT8 {
        c * 64 + (x8 >> 1) * 16 + (x8 & 1) * 4 + ((x >> 3) & 1) * 2 + (r & 1) * 8 + (r >> 1)
    } else {
        c * 128 + (x8 >> 1) * 32 + (x8 & 1) * 8 + ((x >> 3) & 3) * 2 + (r & 1) * 16 + (r >> 1)
    }
}

fn texel(psm: u8, d: &[u8], i: usize) -> u32 {
    let at = |o: usize, n: usize| (0..n).fold(0u32, |v, k| v | (d.get(o + k).copied().unwrap_or(0) as u32) << (8 * k));
    match psm {
        PSMT4 => at(i / 2, 1) >> (4 * (i & 1)) & 0xf,
        PSMT8 => at(i, 1),
        PSMCT16 => at(i * 2, 2),
        PSMCT24 => at(i * 3, 3),
        _ => at(i * 4, 4),
    }
}

/// The bytes PCSX2 hashes for one level of `w`×`h` pixels (the data's own width is `dw`).
fn level_bytes(psm: u8, d: &[u8], w: u32, h: u32, dw: u32, texa: (u8, bool, u8)) -> Vec<u8> {
    let ((bw, bh), bpp) = format(psm).unwrap();
    let get = |x: u32, y: u32| if x < dw { texel(psm, d, (y * dw + x) as usize) } else { 0 };
    if w < bw || h < bh || psm == PSMCT24 || psm == PSMCT16 {
        let (ta0, aem, ta1) = texa;
        return (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .flat_map(|(x, y)| {
                let c = get(x, y);
                match psm {
                    PSMT8 | PSMT4 => vec![c as u8],
                    PSMCT32 => c.to_le_bytes().to_vec(),
                    PSMCT24 => vec![c as u8, (c >> 8) as u8, (c >> 16) as u8, if aem && c == 0 { 0 } else { ta0 }],
                    _ => {
                        let ch = |s: u32| ((c >> s & 31) << 3) as u8;
                        let a = if c & 0x8000 != 0 {
                            ta1
                        } else if aem && c & 0x7fff == 0 {
                            0
                        } else {
                            ta0
                        };
                        vec![ch(0), ch(5), ch(10), a]
                    }
                }
            })
            .collect();
    }
    let mut out = vec![0u8; (w * h * bpp / 8) as usize];
    for by in 0..h / bh {
        for bx in 0..w / bw {
            let block = &mut out[((by * (w / bw) + bx) * 256) as usize..][..256];
            for y in 0..bh {
                for x in 0..bw {
                    let (c, o) = (get(bx * bw + x, by * bh + y), in_block(psm, x, y) as usize);
                    match psm {
                        PSMT4 => block[o / 2] |= (c as u8) << (4 * (o & 1)),
                        PSMT8 => block[o] = c as u8,
                        _ => block[o * 4..o * 4 + 4].copy_from_slice(&c.to_le_bytes()),
                    }
                }
            }
        }
    }
    out
}

impl GsTex<'_> {
    /// PCSX2's file name (without `.png`) when it draws levels `first..=last`; `None` for formats it
    /// can't be. `texa` = (TA0, AEM, TA1), used by 16/24-bit textures only.
    pub fn pcsx2_name(&self, first: usize, last: usize, data_width: u32, texa: (u8, bool, u8)) -> Option<String> {
        format(self.psm)?;
        let mut bytes = Vec::new();
        for k in first..=last {
            let (w, h) = (1 << self.tw.saturating_sub(k as u32), 1 << self.th.saturating_sub(k as u32));
            bytes.extend(level_bytes(self.psm, self.levels.get(k)?, w, h, (data_width >> k).max(1), texa));
        }
        let (tw, th) = (self.tw.saturating_sub(first as u32), self.th.saturating_sub(first as u32));
        let mut bits = self.psm as u32 | tw << 6 | th << 10;
        if matches!(self.psm, PSMCT24 | PSMCT16) {
            bits |= (texa.0 as u32) << 15 | (texa.1 as u32) << 23 | (texa.2 as u32) << 24;
        }
        let tex = XxHash3_64::oneshot(&bytes);
        Some(if matches!(self.psm, PSMT8 | PSMT4) {
            let n = if self.psm == PSMT8 { 256 } else { 16 };
            let clut: Vec<u8> = (0..n).flat_map(|i| self.clut.get(i).copied().unwrap_or([0; 4])).collect();
            format!("{tex:x}-{:x}-{bits:08x}", XxHash3_64::oneshot(&clut))
        } else {
            format!("{tex:x}-{bits:08x}")
        })
    }

    /// Every name PCSX2 may give this texture: the base level alone and each mip range it may draw.
    pub fn pcsx2_names(&self, data_width: u32, texa: (u8, bool, u8)) -> Vec<String> {
        let n = self.levels.len();
        (0..n).flat_map(|a| (a..n).map(move |b| (a, b))).filter_map(|(a, b)| self.pcsx2_name(a, b, data_width, texa)).collect()
    }
}

/// log2 of the GS size the game gives a texture `n` pixels wide/high (the next power of two).
pub fn log2_size(n: u32) -> u32 {
    n.max(1).next_power_of_two().trailing_zeros()
}

/// The TEXA PCSX2's default leaves (TA0 0, AEM off, TA1 0x80); a `.pst` is palettised, so it is not
/// part of its names.
pub const TEXA: (u8, bool, u8) = (0, false, 0x80);

/// A `.pst` as the GS gets it: its format, its levels largest first, its width and its palette in the
/// order the GS reads it.
pub struct Pst {
    pub psm: u8,
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<u8>>,
    pub clut: Vec<[u8; 4]>,
}

impl Pst {
    /// The `.pst` in `d` (the layouts [`crate::pst`] documents), or the first frame of an `.atm`;
    /// `None` for anything else.
    pub fn parse(d: &[u8]) -> Option<Pst> {
        let le = |at: usize| Some(u32::from_le_bytes(d.get(at..at + 4)?.try_into().ok()?));
        if le(0)? == 1 && le(8)? == 0 && le(4)? > 0 && le(4)? <= 1024 && d.get(12..16) == Some(&[2, 0, 2, 0][..]) {
            // An `.atm`: its first frame.
            let frames = le(4)? as usize;
            return Pst::parse(&d[12..12 + (d.len() - 12) / frames]);
        }
        let four = le(4)? == crate::pst::FOUR_BIT;
        let (entries, count_at) = if four { (16, 72) } else { (256, 1032) };
        let clut = (0..entries)
            .map(|i| {
                let at = 8 + 4 * if four { i } else { crate::pst::unswizzle(i) };
                d.get(at..at + 4).map(|c| [c[0], c[1], c[2], c[3]])
            })
            .collect::<Option<Vec<_>>>()?;
        let mips = le(count_at)? as usize;
        if !(1..=12).contains(&mips) {
            return None;
        }
        let mut at = count_at + 4;
        let mut levels = Vec::new();
        let (mut width, mut height) = (0, 0);
        for _ in 0..mips {
            let (h, w, n) = (le(at)?, le(at + 4)?, le(at + 8)? as usize);
            if w == 0 || h == 0 || n != ((w * h) as usize * if four { 1 } else { 2 }).div_ceil(2) {
                return None;
            }
            levels.push(d.get(at + 16..at + 16 + n)?.to_vec());
            (width, height) = (w, h);
            at += 16 + n;
        }
        levels.reverse();
        Some(Pst { psm: if four { PSMT4 } else { PSMT8 }, width, height, levels, clut })
    }

    /// The GS's view of it.
    pub fn gs(&self) -> GsTex<'_> {
        GsTex { psm: self.psm, tw: log2_size(self.width), th: log2_size(self.height), levels: self.levels.iter().map(Vec::as_slice).collect(), clut: &self.clut }
    }

    /// The names PCSX2 gives it, the ones that start at the picture first: each run of levels the GS
    /// may draw, the base with its whole chain first. A far-off model draws from a smaller level as
    /// its base (MX Unleashed sets TEX0 to one of the chain's levels), and PCSX2 names that upload by
    /// the run from there down, so a pack holds those as pictures of their own.
    pub fn pcsx2_names(&self) -> Vec<String> {
        let g = self.gs();
        let n = self.levels.len();
        (0..n).flat_map(|a| (a..n).rev().map(move |b| (a, b))).filter_map(|(a, b)| g.pcsx2_name(a, b, self.width, TEXA)).collect()
    }
}

/// The PCSX2 names of a texture file's bytes ([`Pst::pcsx2_names`]); none for what is not a `.pst`.
pub fn pcsx2_names(file: &[u8]) -> Vec<String> {
    Pst::parse(file).map(|p| p.pcsx2_names()).unwrap_or_default()
}

/// Whether `stem` (a file name without `.png`) is a PCSX2 replacement's name: `<hex>-<hex>-<8 hex>` or
/// `<hex>-<8 hex>`. Not its `-mipN` levels, which come with the base level.
pub fn is_pcsx2_name(stem: &str) -> bool {
    let parts: Vec<&str> = stem.split('-').collect();
    let hex = |s: &str, max: usize| !s.is_empty() && s.len() <= max && s.bytes().all(|b| b.is_ascii_hexdigit());
    matches!(parts.len(), 2 | 3) && parts[..parts.len() - 1].iter().all(|p| hex(p, 16)) && parts[parts.len() - 1].len() == 8 && hex(parts[parts.len() - 1], 8)
}

/// A PCSX2 pack's files at the top of `dir`, by name (without `.png`).
pub fn pack(dir: &Path) -> std::collections::HashMap<String, PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?;
            let stem = name.len().checked_sub(4).filter(|&n| name.is_char_boundary(n) && name[n..].eq_ignore_ascii_case(".png")).map(|n| &name[..n])?;
            is_pcsx2_name(stem).then(|| (stem.to_ascii_lowercase(), p.clone()))
        })
        .collect()
}

/// A pack's picture is in the GS's alpha (0x80 opaque): straight 0..255, as the rest of the renderer
/// draws.
pub fn expand_alpha(rgba: &mut [u8]) {
    rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = (*a as u32 * 255 / 128).min(255) as u8);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swizzle_columns() {
        // first rows of the GS manual's PSMT8/PSMT4 column layouts
        assert_eq!((0..8).map(|x| in_block(PSMT8, x, 2)).collect::<Vec<_>>(), [33, 37, 49, 53, 1, 5, 17, 21]);
        assert_eq!((0..8).map(|x| in_block(PSMT8, x, 4)).collect::<Vec<_>>(), [96, 100, 112, 116, 64, 68, 80, 84]);
        assert_eq!((24..32).map(|x| in_block(PSMT4, x, 3)).collect::<Vec<_>>(), [87, 95, 119, 127, 23, 31, 55, 63]);
        assert_eq!((0..8).map(|x| in_block(PSMCT32, x, 1)).collect::<Vec<_>>(), [2, 3, 6, 7, 10, 11, 14, 15]);
    }

    #[test]
    fn pcsx2_names_are_told_from_others() {
        assert!(is_pcsx2_name("1ccd2192c3c0484c-48d8164186c1d734-00001553"));
        assert!(is_pcsx2_name("2dc5991659e33d9-56c76b19a6b072f9-000015d4"));
        assert!(is_pcsx2_name("2dc5991659e33d9-00002a80"));
        assert!(!is_pcsx2_name("1ccd2192c3c0484c-48d8164186c1d734-00001553-mip1"));
        assert!(!is_pcsx2_name("bush-0123456789abcdef"));
        assert!(!is_pcsx2_name("x-y-00001553"));
    }

    #[test]
    fn the_alpha_doubles_to_straight() {
        let mut p = [1, 2, 3, 0x80, 1, 2, 3, 0x40, 0, 0, 0, 0xff];
        expand_alpha(&mut p);
        assert_eq!([p[3], p[7], p[11]], [255, 127, 255]);
    }
}
