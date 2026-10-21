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
//! The journal has where in the original that is.
//!
//! **Here** that is [`paint`]: a fresh transparent 64x128 with the three strings drawn in at those
//! centres, handed to [`game_render::world::Assets::make_texture`] before each rider's body and
//! bike are loaded, so every rider's models pick up their own.

use crate::menu::Font;
use crate::roster::{Look, Wardrobe};
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
pub const NAME_AT: (f32, f32) = (32.0, 15.0);
pub const NUMBER_AT: (f32, f32) = (32.0, 39.0);
pub const PLATES_AT: [(f32, f32); 2] = [(31.0, 79.0), (31.0, 109.0)];

/// The fonts of `biketext.res`, each parsed the first time it is asked for.
pub struct Fonts {
    res: Res,
    fonts: HashMap<String, Option<Font>>,
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
    pub fn font(&mut self, name: &str) -> Option<&Font> {
        let key = name.to_ascii_lowercase();
        if !self.fonts.contains_key(&key) {
            let cel = self.res.read_suffix(&format!("font\\eng\\{key}.cel")).ok();
            let pst = self.res.read_suffix(&format!("font\\eng\\{key}.pst")).ok();
            let font = cel.zip(pst.and_then(|b| Texture::parse(&b).ok())).and_then(|(c, t)| Font::parse(&c, t).ok());
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
    pub fn of(wardrobe: &Wardrobe, rider: &str, look: &Look, bike: &str) -> Faces {
        let (name, number) = wardrobe.fonts(rider, look);
        Faces { name, number, plate: wardrobe.plate_font(bike) }
    }
}

/// Draw `text` in `font` centred on `at`, alpha over what is there. A string wider than the texture
/// is squeezed across to fit (the original's name is a profile name of any length; how it fits a
/// long one is not known).
fn draw(t: &mut Texture, font: &Font, text: &str, at: (f32, f32)) {
    if text.is_empty() {
        return;
    }
    let w = font.measure(text);
    let h = text
        .chars()
        .map(|c| font.glyph(c))
        .map(|g| g.uv[3] - g.uv[1])
        .fold(0.0f32, f32::max);
    let room = t.width as f32 - 2.0;
    let sx = if w > room { room / w } else { 1.0 };
    let mut pen = at.0 - w * sx / 2.0;
    let top = (at.1 - h / 2.0).round();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        let g = font.glyph(c);
        pen += g.before * sx;
        if c != ' ' || font.has(' ') {
            blit(t, &font.atlas, g.uv, pen, top, sx);
        }
        pen += (g.width + g.after) * sx;
        if let Some(&n) = chars.peek() {
            pen += g.kern.iter().find(|k| k.0 == n as u32).map_or(0.0, |k| k.1) * sx;
        }
    }
}

/// One glyph's box of the atlas onto `t` with its left edge at `x`, top at `y`, squeezed across by
/// `sx`: the nearest texel, alpha over.
fn blit(t: &mut Texture, atlas: &Texture, uv: [f32; 4], x: f32, y: f32, sx: f32) {
    let (bw, bh) = (uv[2] - uv[0], uv[3] - uv[1]);
    let x0 = x.round() as i64;
    let cols = (bw * sx).round() as i64;
    for row in 0..bh.round() as i64 {
        let ty = y as i64 + row;
        let ay = (uv[1] + row as f32) as usize;
        if ty < 0 || ty >= t.height as i64 || ay >= atlas.height {
            continue;
        }
        for col in 0..cols {
            let tx = x0 + col;
            let ax = (uv[0] + (col as f32 + 0.5) / sx) as usize;
            if tx < 0 || tx >= t.width as i64 || ax >= atlas.width {
                continue;
            }
            let s = &atlas.rgba[(ay * atlas.width + ax) * 4..][..4];
            let d = &mut t.rgba[(ty as usize * t.width + tx as usize) * 4..][..4];
            let a = s[3] as u32;
            for c in 0..3 {
                d[c] = ((s[c] as u32 * a + d[c] as u32 * (255 - a)) / 255) as u8;
            }
            d[3] = (a + d[3] as u32 * (255 - a) / 255).min(255) as u8;
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
        Kit { wardrobe, fonts, custom }
    }

    /// A rider's own `NameDefault`, or `None` to keep the disc's.
    pub fn texture(&mut self, rider: &str, look: &Look, bike: &str) -> Option<Texture> {
        let faces = Faces::of(&self.wardrobe, rider, look, bike);
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
