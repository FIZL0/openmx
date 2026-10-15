//! `.pst` — the disc's texture format: eight-bit pixels into a 256-colour palette, plus a mip chain.
//!
//! ```text
//! 0     u32 0x00020002, u32 0x28     the same in every texture on the disc; unexplained
//! 8     256 x RGBA                   the palette, in the GS's order (see below)
//! 1032  u32 mip count, u32 width of the *smallest* mip, u32 height of it, u32 its byte count, u32 0
//! 1052  the largest mip, width x height bytes, then each smaller one after 16 unexplained bytes
//! ```
//!
//! The full size is `last << (mips - 1)`, which reproduces the exact length of all 143 textures on this
//! disc — the check that the header is understood.
//!
//! The palette order is the one quirk. The GS reads a 256-entry palette as eight blocks of 32, with the
//! middle two blocks of each group of four swapped, so a palette copied straight out of the file comes
//! out in bands of wrong colour. Undoing it is one line, and the result was checked by eye against the
//! texture it belongs to.

/// A decoded texture: the largest mip, as RGBA.
#[derive(Clone, Debug)]
pub struct Texture {
    pub width: usize,
    pub height: usize,
    /// `width * height * 4` bytes.
    pub rgba: Vec<u8>,
}

impl Texture {
    /// Decode the largest mip of a `.pst`.
    pub fn parse(d: &[u8]) -> Result<Texture, String> {
        if d.len() < 1052 {
            return Err(format!("not a .pst: only {} bytes", d.len()));
        }
        let mips = le32(d, 1032) as usize;
        let small_w = le32(d, 1036) as usize;
        let small_h = le32(d, 1040) as usize;
        if !(1..=12).contains(&mips) || small_w == 0 || small_h == 0 {
            return Err(format!(
                "not a .pst: {mips} mips of {small_w}x{small_h} makes no sense"
            ));
        }
        let width = small_w << (mips - 1);
        let height = small_h << (mips - 1);
        let pixels = width * height;
        if 1052 + pixels > d.len() {
            return Err(format!(
                "a {width}x{height} texture needs {} bytes, the file has {}",
                1052 + pixels,
                d.len()
            ));
        }
        let mut palette = [[0u8; 4]; 256];
        for (i, slot) in palette.iter_mut().enumerate() {
            let at = 8 + 4 * unswizzle(i);
            // The GS calls 0x80 "fully opaque"; doubling puts it back in 0..=255.
            let a = (d[at + 3] as u32 * 2).min(255) as u8;
            *slot = [d[at], d[at + 1], d[at + 2], a];
        }
        let mut rgba = vec![0u8; pixels * 4];
        for (i, &p) in d[1052..1052 + pixels].iter().enumerate() {
            rgba[4 * i..4 * i + 4].copy_from_slice(&palette[p as usize]);
        }
        Ok(Texture {
            width,
            height,
            rgba,
        })
    }

    /// The texel at wrapped texture coordinates. Nearest neighbour: close enough for a still frame, and
    /// the hardware's bilinear filter is a separate argument.
    pub fn sample(&self, u: f32, v: f32) -> [u8; 4] {
        if self.width == 0 || self.height == 0 {
            return [255, 0, 255, 255];
        }
        let wrap = |t: f32, n: usize| {
            let n = n as f32;
            let t = t - (t / n).floor() * n;
            (t as usize).min(n as usize - 1)
        };
        let x = wrap(u * self.width as f32, self.width);
        let y = wrap(v * self.height as f32, self.height);
        let at = 4 * (y * self.width + x);
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }
}

/// Where entry `i` of a 256-colour palette really lives: the GS swaps the middle two blocks of 8 in
/// every group of 32. The terrain's own palette is in the same order, so this is shared.
pub(crate) fn unswizzle(i: usize) -> usize {
    (i & 0xE7) | ((i & 0x08) << 1) | ((i & 0x10) >> 1)
}

fn le32(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_palette_order_swaps_two_blocks_and_nothing_else() {
        // The first eight and the last eight of each 32 stay put; the middle two blocks trade.
        assert_eq!(unswizzle(0), 0);
        assert_eq!(unswizzle(7), 7);
        assert_eq!(unswizzle(8), 16);
        assert_eq!(unswizzle(16), 8);
        assert_eq!(unswizzle(24), 24);
        assert_eq!(unswizzle(32), 32);
        // It is its own inverse, so decoding twice is the identity.
        for i in 0..256 {
            assert_eq!(unswizzle(unswizzle(i)), i);
        }
    }

    #[test]
    fn a_texture_smaller_than_its_header_is_refused() {
        assert!(Texture::parse(&[0u8; 64]).is_err());
    }
}
