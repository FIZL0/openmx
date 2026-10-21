//! `.pst` — the disc's texture format: eight-bit pixels into a 256-colour palette, plus a mip chain.
//!
//! ```text
//! 0     u32 0x00020002, u32 0x28     the same in every texture on the disc; unexplained
//! 8     256 x RGBA                   the palette, in the GS's order (see below)
//! 1032  u32 mip count, u32 height of the *smallest* mip, u32 width of it, u32 its byte count, u32 0
//! 1052  the largest mip, width x height bytes, then each smaller one after 16 unexplained bytes
//! ```
//!
//! Height first: the title screen says 448 then 640, and the bushes' billboard on `free06` is a clean
//! strip of plants at 512x256 and the same plants cut into alternating rows at 256x512. Square
//! textures, which are most of them, read the same either way, which is how this stayed hidden (P36).
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
        crate::tally::decode();
        if d.len() >= 8 && le32(d, 4) == FOUR_BIT {
            return Texture::parse_four_bit(d);
        }
        if d.len() < 1052 {
            return Err(format!("not a .pst: only {} bytes", d.len()));
        }
        let mips = le32(d, 1032) as usize;
        let small_h = le32(d, 1036) as usize;
        let small_w = le32(d, 1040) as usize;
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

    /// Decode the first frame of an `.atm`, an animated texture: a 12-byte header (`1`, the frame
    /// count, `0`) and then that many whole `.pst` files of one size, back to back. The frames are
    /// the flag's wave or the crowd's bob; a still picture gets the first.
    pub fn parse_animation(d: &[u8]) -> Result<Texture, String> {
        if d.len() < 12 || le32(d, 0) != 1 {
            return Err("not an .atm: no header".to_string());
        }
        let frames = le32(d, 4) as usize;
        if frames == 0 || (d.len() - 12) % frames != 0 {
            return Err(format!(
                "not an .atm: {} bytes do not split into {frames} frames",
                d.len() - 12
            ));
        }
        Texture::parse(&d[12..12 + (d.len() - 12) / frames])
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

    /// The texel at wrapped texture coordinates, mixed from the four nearest: the hardware's bilinear
    /// filter, for a texture seen up close, where nearest neighbour shows its squares.
    pub fn sample_smooth(&self, u: f32, v: f32) -> [u8; 4] {
        self.sample_smooth_parts(u, v, true, true)
    }

    /// [`Texture::sample_smooth`], mixing only the colour, only the alpha, or both: a channel not
    /// asked for comes back 0, and one asked for is exactly what `sample_smooth` gives. The ground's
    /// masks are read for their alpha alone and its layers for their colour (P47).
    #[inline]
    pub fn sample_smooth_parts(&self, u: f32, v: f32, rgb: bool, alpha: bool) -> [u8; 4] {
        if self.width == 0 || self.height == 0 {
            return [255, 0, 255, 255];
        }
        let (x, y) = (u * self.width as f32 - 0.5, v * self.height as f32 - 0.5);
        let (x_floor, y_floor) = (floor(x), floor(y));
        let (fx, fy) = (x - x_floor, y - y_floor);
        let (x0, y0) = (x_floor as i64, y_floor as i64);
        // Every texture on the disc is a power of two along each side, where the wrap is a mask
        // rather than a division (P47: four divisions a sample, nine samples a ground pixel).
        let (w, h) = (self.width as i64, self.height as i64);
        let wrap = |t: i64, n: i64| if n & (n - 1) == 0 { (t & (n - 1)) as usize } else { t.rem_euclid(n) as usize };
        let (xa, xb, ya, yb) = (wrap(x0, w), wrap(x0 + 1, w), wrap(y0, h), wrap(y0 + 1, h));
        let corners = [
            4 * (ya * self.width + xa),
            4 * (ya * self.width + xb),
            4 * (yb * self.width + xa),
            4 * (yb * self.width + xb),
        ];
        let weights = [(1.0 - fx) * (1.0 - fy), fx * (1.0 - fy), (1.0 - fx) * fy, fx * fy];
        let mut out = [0u8; 4];
        let channels = match (rgb, alpha) {
            (true, true) => 0..4,
            (true, false) => 0..3,
            (false, true) => 3..4,
            (false, false) => 0..0,
        };
        for k in channels {
            let mixed: f32 = corners.iter().zip(weights).map(|(&c, w)| self.rgba[c + k] as f32 * w).sum();
            out[k] = (mixed + 0.5).min(255.0) as u8;
        }
        out
    }
}

/// `x.floor()`, without the library call a target with no rounding instruction makes of it: the
/// same value for every float a texture coordinate can be (an integer-valued float is its own floor).
#[inline]
pub(crate) fn floor(x: f32) -> f32 {
    if x.abs() < 8_388_608.0 {
        let t = x as i32 as f32;
        if t > x { t - 1.0 } else { t }
    } else {
        x.floor()
    }
}

/// The second header word of a sixteen-colour `.pst`, which is most of the menus' art (`ui\tex\*`)
/// and the fonts: every track texture has `0x28` there instead.
pub(crate) const FOUR_BIT: u32 = 0x2e;

impl Texture {
    /// Decode the largest mip of a sixteen-colour `.pst`: the same layout with a 16-entry palette,
    /// which the GS reads in order, and two pixels a byte, the low nibble first.
    ///
    /// ```text
    /// 0     u32 0x00020002, u32 0x2e
    /// 8     16 x RGBA                    the palette
    /// 72    u32 mip count, u32 height of the smallest mip, u32 width of it, u32 its byte count, u32 0
    /// 92    the largest mip, width x height / 2 bytes, a row at a time
    /// ```
    ///
    /// Height before width, as in the 256-colour header: the main menu's highlight bar is 256 wide
    /// and 32 high, as its layout (`ui\mainmenu.lyt`) places it. Checked by eye against the main
    /// menu's own art (P36): the highlight, the arrows, the button icons and the font atlases.
    fn parse_four_bit(d: &[u8]) -> Result<Texture, String> {
        const PIXELS: usize = 92;
        if d.len() < PIXELS {
            return Err(format!("not a sixteen-colour .pst: only {} bytes", d.len()));
        }
        let mips = le32(d, 72) as usize;
        let small_h = le32(d, 76) as usize;
        let small_w = le32(d, 80) as usize;
        if !(1..=12).contains(&mips) || small_w == 0 || small_h == 0 {
            return Err(format!(
                "not a .pst: {mips} mips of {small_w}x{small_h} makes no sense"
            ));
        }
        let width = small_w << (mips - 1);
        let height = small_h << (mips - 1);
        let pixels = width * height;
        if PIXELS + pixels.div_ceil(2) > d.len() {
            return Err(format!(
                "a {width}x{height} sixteen-colour texture needs {} bytes, the file has {}",
                PIXELS + pixels.div_ceil(2),
                d.len()
            ));
        }
        let mut palette = [[0u8; 4]; 16];
        for (i, slot) in palette.iter_mut().enumerate() {
            let at = 8 + 4 * i;
            let a = (d[at + 3] as u32 * 2).min(255) as u8;
            *slot = [d[at], d[at + 1], d[at + 2], a];
        }
        let mut rgba = vec![0u8; pixels * 4];
        for i in 0..pixels {
            let byte = d[PIXELS + i / 2];
            let p = if i % 2 == 0 { byte & 0x0f } else { byte >> 4 };
            rgba[4 * i..4 * i + 4].copy_from_slice(&palette[p as usize]);
        }
        Ok(Texture {
            width,
            height,
            rgba,
        })
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
    fn a_sixteen_colour_texture_reads_two_pixels_a_byte_low_nibble_first() {
        let mut d = vec![0u8; 92 + 2];
        d[0..4].copy_from_slice(&0x0002_0002u32.to_le_bytes());
        d[4..8].copy_from_slice(&FOUR_BIT.to_le_bytes());
        // Entry 1 red, entry 2 green, both opaque at the GS's 0x80.
        d[12..16].copy_from_slice(&[255, 0, 0, 0x80]);
        d[16..20].copy_from_slice(&[0, 255, 0, 0x80]);
        for (at, v) in [(72, 1u32), (76, 1), (80, 4), (84, 2)] {
            d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        d[92] = 0x21;
        d[93] = 0x01;
        let t = Texture::parse(&d).unwrap();
        assert_eq!((t.width, t.height), (4, 1));
        assert_eq!(&t.rgba[0..4], &[255, 0, 0, 255]);
        assert_eq!(&t.rgba[4..8], &[0, 255, 0, 255]);
        assert_eq!(&t.rgba[8..12], &[255, 0, 0, 255]);
        assert_eq!(t.rgba[15], 0);
    }

    #[test]
    fn the_inline_floor_is_the_library_floor() {
        // Equal as numbers: the one difference, -0.0's floor coming back +0.0, mixes the same texels.
        for x in [-2.5f32, -1.0, -0.75, -0.0, 0.0, 0.25, 1.0, 3.999_999_8, 1e7, -1e7, 8_388_607.5, -8_388_607.5, 4e9] {
            assert_eq!(floor(x), x.floor(), "{x}");
        }
        // And over a sweep of the coordinates a ground sample sees.
        let mut x = -30_000.0f32;
        while x < 30_000.0 {
            assert_eq!(floor(x), x.floor(), "{x}");
            x += 0.137;
        }
    }

    #[test]
    fn a_texture_smaller_than_its_header_is_refused() {
        assert!(Texture::parse(&[0u8; 64]).is_err());
    }
}
