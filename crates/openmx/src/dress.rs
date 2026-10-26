//! A rider's name and number on their jersey and their bike's plates (P86), painted as the original
//! paints them.
//!
//! **The texture.** Every rider's jersey back and every bike's number plates (the `250sx_plastics`
//! and `250sx_numberplate` parts, under each bike's own paint) are mapped onto one 64x128 texture,
//! `Rider\Tex\NameDefault`, whose `.psm` blends it over the paint. The disc's own copy has
//! `Baumsteiger` and `000` drawn on it. The original copies it per rider (`NameDefault0` … `9`) and
//! draws that rider's name and number onto the copy with fonts out of `BikeText.res`: the name in
//! the jersey's name font centred at (32, 15), the number in its number font at (32, 39), and the
//! number again in the bike's plate font at (31, 79) and (31, 109) — one plate on each side of the
//! bike. The jersey's fonts are its `[JerseyPants]` entry's in `rider\gear.dat` (`name3`, or
//! `name3_black` on a light jersey) and the plate's the bike's `NumberPlateFont` in `bike\bikes.dat`.
//! A gear set's entry, a pro rider's too, is the one its `.sub` dresses it in ([`uniform`]). The
//! journal has where in the original that is.
//!
//! **Here** that is [`paint`]: a fresh transparent 64x128 with the three strings drawn in at those
//! centres, handed to [`game_render::world::Assets::make_texture`] before each rider's body and
//! bike are loaded, so every rider's models pick up their own.
//!
//! **Beside the original** (P91): with a 12-letter name, 888, on a white-digit and a black-digit
//! bike, every texel of [`paint`]'s is painted where the original's own `NameDefault0` in PCSX2's RAM
//! is, and in its colour to within its palette's 5. That includes the long name's overflow: the
//! original draws each glyph at its size and nothing fits or clips it, so a name wider than 64 comes
//! round onto the rows above and below (see [`draw`]). The original then fits the colours into the
//! texture's 256-colour palette; that is not done here.

use crate::roster::{Look, Outfit, Wardrobe};
use game_render::Disc;
use game_render::disc::Res;
use game_render::pst::Texture;
use std::collections::HashMap;

/// The texture file the jersey and the plates are painted with, as their material names it.
pub const NAME_TEXTURE: &str = "NameDefault.pst";

/// The archive the original's name, number and plate fonts are in.
pub const FONT_ARCHIVE: &str = "biketext.res";

/// Its size, the disc's own copy's.
pub const SIZE: (usize, usize) = (64, 128);

/// Where the original centres each string on it.
pub const NAME_AT: (i32, i32) = (32, 15);
pub const NUMBER_AT: (i32, i32) = (32, 39);
pub const PLATES_AT: [(i32, i32); 2] = [(31, 79), (31, 109)];

/// The fonts of `biketext.res`, each parsed the first time it is asked for.
pub struct Fonts {
    res: Res,
    fonts: HashMap<String, Option<Face>>,
}

impl Fonts {
    pub fn open(disc: &mut Disc) -> Result<Fonts, String> {
        let member = disc
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .find(|n| n.eq_ignore_ascii_case(FONT_ARCHIVE))
            .ok_or(format!("the disc has no {FONT_ARCHIVE}"))?;
        Ok(Fonts { res: disc.res(&member)?, fonts: HashMap::new() })
    }

    /// `font\eng\<name>.cel` over its `.pst` atlas.
    pub fn font(&mut self, name: &str) -> Option<&Face> {
        let key = name.to_ascii_lowercase();
        if !self.fonts.contains_key(&key) {
            let cel = self.res.read_suffix(&format!("font\\eng\\{key}.cel")).ok();
            let pst = self.res.read_suffix(&format!("font\\eng\\{key}.pst")).ok();
            let font = cel.zip(pst.and_then(|b| Texture::parse(&b).ok())).and_then(|(c, t)| Face::parse(&c, t).ok());
            self.fonts.insert(key.clone(), font);
        }
        self.fonts[&key].as_ref()
    }
}

/// Which fonts a rider is drawn in: their jersey's name and number fonts and their bike's plate font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Faces {
    pub name: String,
    pub number: String,
    pub plate: String,
}

impl Faces {
    /// The look's own `[JerseyPants]` pick's fonts; else, for a look that keeps its gear set's
    /// jersey, `uniform`'s, the entry that set's `.sub` dresses it in (see [`uniform`]); else what
    /// [`Wardrobe::fonts`] makes of the set's name.
    pub fn of(wardrobe: &Wardrobe, rider: &str, uniform: Option<&Outfit>, look: &Look, bike: &str) -> Faces {
        let (name, number) = match uniform.filter(|_| look.outfit.is_empty()) {
            Some(o) => (o.name_font.clone(), o.number_font.clone()),
            None => wardrobe.fonts(rider, look),
        };
        Faces { name, number, plate: wardrobe.plate_font(bike) }
    }
}

/// The body's jersey and pants materials, which a gear set's `.sub` renames.
pub const BASE_JERSEY: &str = "THQ_JerseySleeves_01";
pub const BASE_PANTS: &str = "THQ_Pants_01";

/// The `[JerseyPants]` entry a gear set wears, as the original finds it when it reads the
/// `[Uniforms]` list: what the set's `.sub` renames [`BASE_JERSEY`] and [`BASE_PANTS`] to (each
/// itself if not renamed), and the first entry of `rider\gear.dat` with that jersey and those pants,
/// by name in any case. A pro rider's set is a sponsor's entry, not `name1`/`number1`: Parsons1 wears
/// Skin1's, Faisst1 Generic1's, Renner TLD's, Langston1 Thor1's (P91). `jerseys` is every entry, in
/// the file's order, whether the wardrobe archive has its paint or not.
pub fn uniform<'a>(jerseys: &'a [Outfit], suit: &[(String, String)]) -> Option<&'a Outfit> {
    let renamed = |m: &'static str| suit.iter().find(|(from, _)| from.eq_ignore_ascii_case(m)).map_or(m, |(_, to)| to.as_str());
    let (jersey, pants) = (renamed(BASE_JERSEY), renamed(BASE_PANTS));
    jerseys.iter().find(|o| o.jersey.eq_ignore_ascii_case(jersey) && o.pants.eq_ignore_ascii_case(pants))
}

/// One glyph of a [`Face`], in whole texels as the original's painter takes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cel {
    /// The box's top-left texel in the atlas: the `.cel`'s 0..1 corner times the atlas's size,
    /// truncated.
    pub at: (i32, i32),
    /// Columns drawn, which is also the pen's advance before `after`.
    pub width: i32,
    pub before: i32,
    pub after: i32,
    /// Rows drawn, from the box's top.
    pub rows: i32,
    /// The following character, and how much nearer (negative) it sits, in the file's order.
    pub kern: Vec<(u32, i32)>,
}

/// A `.cel` font as the original's text painter reads it: the header's spacing, a glyph per byte
/// (a byte the font lacks draws glyph 0, the missing box), and the atlas.
///
/// ```text
/// 0   f32, u16 point size, s16 spacing (0 in name1/number1/plate1/number3, -1 in name3 and
///     name3_black, -3 in plate3 and plate3_black), u32 glyph count
/// 12  per glyph: u32 code, 4 x f32 its box in the atlas (0..1), u32 code again,
///     s16 width, s16 before, s16 after, u16 line height, u16 kerning pairs,
///     then each pair as u16 next code, s16 adjustment
/// ```
///
/// [`crate::menu::Font`] reads the same files for the menus; this one keeps what the painter uses
/// that the menus do not, the spacing and the line height (P91).
pub struct Face {
    pub spacing: i32,
    pub glyphs: Vec<Option<Cel>>,
    pub atlas: Texture,
}

impl Face {
    pub fn parse(cel: &[u8], atlas: Texture) -> Result<Face, String> {
        let bad = || "a .cel font cut short".to_string();
        let u16_at = |at: usize| cel.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]])).ok_or_else(bad);
        let u32_at = |at: usize| cel.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(bad);
        let spacing = u16_at(6)? as i16 as i32;
        let count = u32_at(8)? as usize;
        let mut glyphs = vec![None; 256];
        let mut at = 12;
        for _ in 0..count {
            let code = u32_at(at)?;
            let f = |i: usize| u32_at(at + 4 + 4 * i).map(f32::from_bits);
            let corner = ((f(0)? * atlas.width as f32) as i32, (f(1)? * atlas.height as f32) as i32);
            let s = |o: usize| u16_at(at + o).map(|v| v as i16 as i32);
            let (width, before, after, rows) = (s(24)?, s(26)?, s(28)?, u16_at(at + 30)? as i32);
            let pairs = u16_at(at + 32)? as usize;
            at += 34;
            let mut kern = Vec::with_capacity(pairs);
            for k in 0..pairs {
                kern.push((u16_at(at + 4 * k)? as u32, u16_at(at + 4 * k + 2)? as i16 as i32));
            }
            at += 4 * pairs;
            if let Some(slot) = glyphs.get_mut(code as usize) {
                *slot = Some(Cel { at: corner, width, before, after, rows, kern });
            }
        }
        if glyphs[0].is_none() {
            return Err("a .cel font with no glyph 0".into());
        }
        Ok(Face { spacing, glyphs, atlas })
    }

    /// The glyph a byte draws: its own, else the missing box.
    pub fn glyph(&self, b: u8) -> &Cel {
        self.glyphs[b as usize].as_ref().or(self.glyphs[0].as_ref()).expect("glyph 0, checked by parse")
    }

    /// How wide the original measures `text`: the glyphs' widths, and between each two the first's
    /// `after`, the second's `before`, the font's spacing and the pair's kerning. The first glyph's
    /// `before` and the last's `after` are not counted.
    pub fn measure(&self, text: &[u8]) -> i32 {
        let mut w = 0;
        for (i, &b) in text.iter().enumerate() {
            let g = self.glyph(b);
            w += g.width;
            if let Some(&n) = text.get(i + 1) {
                // The measure stops at the first pair past the next code, and takes the first match.
                let k = g.kern.iter().take_while(|k| k.0 <= n as u32).find(|k| k.0 == n as u32).map_or(0, |k| k.1);
                w += k + self.glyph(n).before + g.after + self.spacing;
            }
        }
        w
    }
}

/// A string's bytes as the original's 8-bit painter sees them: a character past one byte draws the
/// missing box.
fn bytes(text: &str) -> Vec<u8> {
    text.chars().map(|c| u8::try_from(c as u32).unwrap_or(0)).collect()
}

/// Draw `text` in `face` centred on `at` as the original does: the left edge at `at.0` less half the
/// measured width rounded, the top at `at.1` less half glyph 0's line height rounded, and each glyph
/// at its own size — a string wider than the texture is not fitted. Every texel goes to the row-major
/// index `y * 64 + x` without a check on `x`, so what runs off the right edge comes back on the next
/// row's left and what runs off the left on the row above's right, as on the original's jersey.
pub fn draw(t: &mut Texture, face: &Face, text: &str, at: (i32, i32)) {
    let text = bytes(text);
    if text.is_empty() {
        return;
    }
    let w = (face.measure(&text) as f32 + 0.5) as i32;
    let mut pen = at.0 - (w as f32 / 2.0 + 0.5) as i32;
    let top = at.1 - (face.glyph(0).rows as f32 / 2.0 + 0.5) as i32;
    for (i, &b) in text.iter().enumerate() {
        let g = face.glyph(b);
        if i > 0 {
            pen += g.before;
        }
        blit(t, &face.atlas, g, pen, top);
        let next = text.get(i + 1).map_or(0, |&n| n as u32);
        // Drawing sums every pair for the next code; no font repeats one.
        pen += face.spacing + g.width + g.after + g.kern.iter().filter(|k| k.0 == next).map(|k| k.1).sum::<i32>();
    }
}

/// One glyph's `width` x `rows` texels from the atlas onto `t` at (`x`, `y`) by row-major index. A
/// texel of the font's that is clear leaves `t` alone; any other mixes the two colours by their
/// alphas and takes the font's alpha: the original's own blend.
fn blit(t: &mut Texture, atlas: &Texture, g: &Cel, x: i32, y: i32) {
    let len = (t.width * t.height) as i64;
    for row in 0..g.rows {
        for col in 0..g.width {
            let (ax, ay) = (g.at.0 + col, g.at.1 + row);
            if ax < 0 || ay < 0 || ax as usize >= atlas.width || ay as usize >= atlas.height {
                continue;
            }
            let at = (y + row) as i64 * t.width as i64 + (x + col) as i64;
            if at < 0 || at >= len {
                continue;
            }
            let s = &atlas.rgba[(ay as usize * atlas.width + ax as usize) * 4..][..4];
            let d = &mut t.rgba[at as usize * 4..][..4];
            let (sa, da) = (s[3] as u32, d[3] as u32);
            if sa == 0 {
                continue;
            }
            for c in 0..3 {
                d[c] = ((d[c] as u32 * da + s[c] as u32 * sa) / (da + sa)) as u8;
            }
            d[3] = s[3];
        }
    }
}

/// A rider's own `NameDefault`: their name and number on a clear 64x128, where the original draws
/// them. `None` for a rider with neither, who keeps the disc's own texture.
pub fn paint(fonts: &mut Fonts, faces: &Faces, look: &Look) -> Option<Texture> {
    if look.name.is_empty() && look.number.is_empty() {
        return None;
    }
    let (w, h) = SIZE;
    let mut t = Texture { width: w, height: h, rgba: vec![0; w * h * 4] };
    if let Some(f) = fonts.font(&faces.name) {
        draw(&mut t, f, &look.name, NAME_AT);
    }
    if let Some(f) = fonts.font(&faces.number) {
        draw(&mut t, f, &look.number, NUMBER_AT);
    }
    if let Some(f) = fonts.font(&faces.plate) {
        for at in PLATES_AT {
            draw(&mut t, f, &look.number, at);
        }
    }
    Some(t)
}

/// What a ride dresses its riders from: the wardrobe's lists, the fonts, and the archive the pieces'
/// paint is in.
pub struct Kit {
    pub wardrobe: Wardrobe,
    pub fonts: Option<Fonts>,
    /// The wardrobe archive's disc member, `Rider\custom.res`.
    pub custom: Option<String>,
    /// Every `[JerseyPants]` entry, for [`uniform`].
    pub jerseys: Vec<Outfit>,
    /// Each gear set's [`uniform`] once it is read, by archive name lowercased.
    uniforms: HashMap<String, Option<Outfit>>,
}

impl Kit {
    /// Everything that is there, saying what is not.
    pub fn open(disc: &mut Disc) -> Kit {
        let wardrobe = Wardrobe::read(disc).unwrap_or_else(|e| {
            println!("dress: {e}");
            Wardrobe::default()
        });
        let fonts = Fonts::open(disc).map_err(|e| println!("dress: {e}")).ok();
        let custom = disc
            .entries()
            .iter()
            .map(|e| e.name.clone())
            .find(|n| n.to_ascii_lowercase().ends_with(&format!("\\{}", crate::roster::WARDROBE_ARCHIVE)));
        let jerseys = disc
            .res("appstart.res")
            .and_then(|app| app.read_suffix("rider\\gear.dat"))
            .map(|gear| Wardrobe::parse(&game_render::ini::text(&gear), "", &|_| true).outfits)
            .unwrap_or_default();
        Kit { wardrobe, fonts, custom, jerseys, uniforms: HashMap::new() }
    }

    /// The `[JerseyPants]` entry gear set `rider` wears, its `Rider\<rider>.res`'s `.sub` read the
    /// first time.
    pub fn uniform(&mut self, disc: &mut Disc, rider: &str) -> Option<&Outfit> {
        let key = rider.to_ascii_lowercase();
        if !self.uniforms.contains_key(&key) {
            let member = disc.entries().iter().map(|e| e.name.clone()).find(|n| n.eq_ignore_ascii_case(&format!("rider\\{key}.res")));
            let suit = member
                .and_then(|m| disc.res(&m).ok())
                .and_then(|res| res.read_suffix(&format!("{key}.sub")).ok())
                .map(|sub| game_render::rider::Suit::parse(&game_render::ini::text(&sub)).materials);
            let found = suit.and_then(|s| uniform(&self.jerseys, &s).cloned());
            self.uniforms.insert(key.clone(), found);
        }
        self.uniforms[&key].as_ref()
    }

    /// A rider's own `NameDefault`, or `None` to keep the disc's.
    pub fn texture(&mut self, disc: &mut Disc, rider: &str, look: &Look, bike: &str) -> Option<Texture> {
        let worn = self.uniform(disc, rider).cloned();
        let faces = Faces::of(&self.wardrobe, rider, worn.as_ref(), look, bike);
        paint(self.fonts.as_mut()?, &faces, look)
    }

    /// The whole repaint of the shared body for one rider: every base material back to itself, so
    /// nothing another rider's look renamed carries over, then the gear set's `.sub`, then the look.
    pub fn repaint(&self, suit: &game_render::rider::Suit, look: &Look) -> Vec<(String, String)> {
        let mut base: Vec<String> = suit.base_materials.clone();
        for (from, _) in &suit.materials {
            if !base.iter().any(|b| b.eq_ignore_ascii_case(from)) {
                base.push(from.clone());
            }
        }
        let mut out: Vec<(String, String)> = base.iter().map(|b| (b.clone(), b.clone())).collect();
        out.extend(suit.materials.iter().cloned());
        out.extend(self.wardrobe.materials(look, &base));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `.cel` of solid boxes on a white 16x16 atlas: per glyph its code, width, before, after,
    /// line height and kerning pairs.
    fn face(spacing: i16, glyphs: &[(u32, i16, i16, i16, u16, &[(u16, i16)])]) -> Face {
        let mut cel = Vec::new();
        cel.extend(2.71f32.to_le_bytes());
        cel.extend(10u16.to_le_bytes());
        cel.extend(spacing.to_le_bytes());
        cel.extend((glyphs.len() as u32).to_le_bytes());
        for &(code, width, before, after, rows, kern) in glyphs {
            cel.extend(code.to_le_bytes());
            for f in [0.0f32, 0.0, width as f32 / 16.0, rows as f32 / 16.0] {
                cel.extend(f.to_le_bytes());
            }
            cel.extend(code.to_le_bytes());
            for v in [width, before, after] {
                cel.extend(v.to_le_bytes());
            }
            cel.extend(rows.to_le_bytes());
            cel.extend((kern.len() as u16).to_le_bytes());
            for &(next, by) in kern {
                cel.extend(next.to_le_bytes());
                cel.extend(by.to_le_bytes());
            }
        }
        Face::parse(&cel, Texture { width: 16, height: 16, rgba: vec![255; 16 * 16 * 4] }).unwrap()
    }

    fn clear() -> Texture {
        Texture { width: 64, height: 128, rgba: vec![0; 64 * 128 * 4] }
    }

    fn painted(t: &Texture, y: usize) -> Vec<usize> {
        (0..64).filter(|x| t.rgba[(y * 64 + x) * 4 + 3] > 0).collect()
    }

    #[test]
    fn the_measure_leaves_out_the_outer_before_and_after_and_counts_the_spacing() {
        let f = face(-1, &[(0, 4, 0, 0, 6, &[]), (u32::from(b'A'), 5, 2, 3, 6, &[(u16::from(b'B'), -2)]), (u32::from(b'B'), 4, 1, 1, 6, &[])]);
        assert_eq!(f.measure(b"A"), 5);
        // 5 + (A's after 3 + B's before 1 - 1 spacing - 2 kerning) + 4
        assert_eq!(f.measure(b"AB"), 10);
        // A byte the font lacks is the missing box, glyph 0.
        assert_eq!(f.measure(b"z"), 4);
    }

    #[test]
    fn a_string_is_centred_on_whole_texels_and_its_first_glyph_drawn_without_its_before() {
        let f = face(0, &[(0, 4, 0, 0, 6, &[]), (u32::from(b'A'), 5, 7, 2, 4, &[])]);
        let mut t = clear();
        draw(&mut t, &f, "AA", (32, 15));
        // width 5 + 2 + 7 + 5 = 19: the left edge at 32 - 10, the top at 15 - 3 (glyph 0's 6 rows);
        // the second A at 22 + 5 + 2 + 7.
        assert_eq!(painted(&t, 12), [22, 23, 24, 25, 26, 36, 37, 38, 39, 40]);
        assert!(painted(&t, 11).is_empty() && painted(&t, 16).is_empty(), "each glyph its own 4 rows");
    }

    #[test]
    fn what_runs_off_the_right_edge_comes_back_on_the_next_row() {
        let f = face(0, &[(0, 10, 0, 0, 2, &[])]);
        let mut t = clear();
        // Seven 10-wide boxes, 70 wide, centred on 32: x from -3 to 66 on rows 14 and 15.
        draw(&mut t, &f, "\0\0\0\0\0\0\0", (32, 15));
        assert_eq!(painted(&t, 13), [61, 62, 63], "the first three columns, at the row above's end");
        assert_eq!(painted(&t, 14).len(), 64);
        assert_eq!(painted(&t, 15).len(), 64);
        assert_eq!(painted(&t, 16), [0, 1, 2], "the last three, at the row below's start");
    }

    #[test]
    fn a_glyph_over_paint_mixes_by_the_two_alphas_and_keeps_its_own() {
        let mut f = face(0, &[(0, 1, 0, 0, 1, &[])]);
        f.atlas.rgba[..4].copy_from_slice(&[200, 100, 0, 100]);
        let mut t = clear();
        // A 1x1 glyph centred on (32, 15) lands at (31, 14).
        t.rgba[(14 * 64 + 31) * 4..][..4].copy_from_slice(&[0, 0, 200, 50]);
        draw(&mut t, &f, "\0", (32, 15));
        // (0 * 50 + 200 * 100) / 150 and so on; the alpha is the font's.
        assert_eq!(&t.rgba[(14 * 64 + 31) * 4..][..4], [133, 66, 66, 100]);
    }

    #[test]
    fn a_gear_set_wears_the_first_entry_with_its_jersey_and_pants() {
        let entry = |id: &str, jersey: &str, pants: &str, font: &str| Outfit {
            id: id.into(),
            label: id.into(),
            jersey: jersey.into(),
            pants: pants.into(),
            name_font: font.into(),
            number_font: font.into(),
        };
        let jerseys = [
            entry("thq1", "THQ_JerseySleeves_01", "THQ_Pants_01", "name3"),
            entry("skin1", "Skin_JerseySleeves_01", "Skin_Pants_01", "name3_black"),
            entry("skin1b", "Skin_JerseySleeves_01", "Skin_Pants_01", "name1"),
        ];
        let pro = [("thq_jerseysleeves_01".to_string(), "SKIN_JerseySleeves_01".to_string()), ("THQ_Pants_01".into(), "Skin_Pants_01".into())];
        assert_eq!(uniform(&jerseys, &pro).map(|o| o.id.as_str()), Some("skin1"));
        assert_eq!(uniform(&jerseys, &[]).map(|o| o.id.as_str()), Some("thq1"), "a .sub that renames neither");
        assert_eq!(uniform(&jerseys, &pro[..1]), None, "a Skin jersey over THQ pants is no entry");
    }
}
