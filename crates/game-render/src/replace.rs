//! Replaced textures (V84): a texture the world decodes is swapped for a picture the shell found for
//! its key, at any size, before anything draws it.
//!
//! opentennis's lookup (texture packs, then `mods/texture-replacements/`, then a mod's own upscales,
//! then the disc) is the shell's, which reads the files; this crate only asks it. It is one process-wide
//! source, set by [`set`], because the loaders that decode a texture are many and deep (a model's
//! material, the ground's layers, its detail map, a road's paint) and none of them should need a new
//! argument for it. With no source set nothing is hashed and every texture is the disc's, byte for
//! byte: the Original switch's, and every replay and picture test's.
//!
//! The key is [`key`]: a hash of the decoded picture, so it is the same on every run and every build
//! and does not move when an archive's members do. `--dump-textures` names each file it writes by it.
//!
//! What is swapped: the 3D world's textures, whose coordinates run 0 to 1 across the picture, so a
//! replacement four times the size keeps every tiling and every UV (opentennis B222). What is not: the
//! HUD's and the menus' sprites and fonts, the particle sheets and the ground's shadow tiles, which are
//! cut by pixel rectangles or joined pixel row to pixel row.

use crate::pst::Texture;
use std::sync::{Arc, RwLock};

/// Where the replacements come from: a texture's key, and its PCSX2 names when the source asks for them
/// (V87, [`crate::texhash`]; none for a texture not read from a `.pst`), to its replacement, `None` for
/// the disc's.
pub type Source = dyn Fn(&str, &dyn Fn() -> Vec<String>) -> Option<Texture> + Send + Sync;

static SOURCE: RwLock<Option<Arc<Source>>> = RwLock::new(None);

/// A texture's key: FNV-1a 64 of its width, its height and its RGBA, as 16 lowercase hex digits (the
/// shape of opentennis's keys, so its file names and its `<name>-<key>.png` matching carry over).
pub fn key(t: &Texture) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    for b in (t.width as u32).to_le_bytes().into_iter().chain((t.height as u32).to_le_bytes()) {
        eat(b);
    }
    for &b in &t.rgba {
        eat(b);
    }
    format!("{h:016x}")
}

/// Replace the source; `None` draws the disc's textures alone. It applies to what is decoded from then
/// on: a ride already loaded keeps its textures.
pub fn set(source: Option<Arc<Source>>) {
    *SOURCE.write().unwrap_or_else(|e| e.into_inner()) = source;
}

/// Whether a source is set.
pub fn on() -> bool {
    SOURCE.read().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// `t`, or its replacement if the source has one. A replacement that is not a whole picture is left
/// out (the disc's is kept).
pub fn swap(t: Texture) -> Texture {
    swap_named(t, &Vec::new)
}

/// [`swap`] of the texture decoded from `file`, a `.pst` or an `.atm`: a PCSX2 pack's picture for it
/// is found too.
pub fn swap_file(t: Texture, file: &[u8]) -> Texture {
    swap_named(t, &|| crate::texhash::pcsx2_names(file))
}

/// A `.pst` or `.atm` decoded as [`crate::world::decode_disc_texture`] does by its contents, and
/// [`swap_file`]d.
pub fn parse(file: &[u8]) -> Option<Texture> {
    let t = if file.len() >= 4 && file[..4] == [1, 0, 0, 0] { Texture::parse_animation(file) } else { Texture::parse(file) };
    t.ok().map(|t| swap_file(t, file))
}

fn swap_named(t: Texture, names: &dyn Fn() -> Vec<String>) -> Texture {
    let source = SOURCE.read().unwrap_or_else(|e| e.into_inner()).clone();
    let Some(source) = source else { return t };
    match source(&key(&t), names) {
        Some(r) if r.width > 0 && r.height > 0 && r.rgba.len() == r.width * r.height * 4 => r,
        _ => t,
    }
}

/// [`swap`] for a set that must stay one size (a terrain's pages, which share one texture array):
/// each is swapped, and if that leaves them of more than one size every one is resampled, bilinear,
/// to the largest, so a pack that covers only some of them still draws them at its size.
pub fn swap_all(set: Vec<Texture>) -> Vec<Texture> {
    if !on() {
        return set;
    }
    let set: Vec<Texture> = set.into_iter().map(swap).collect();
    let (w, h) = set.iter().map(|t| (t.width, t.height)).max_by_key(|&(w, h)| w * h).unwrap_or((0, 0));
    set.into_iter().map(|t| if (t.width, t.height) == (w, h) { t } else { resample(&t, w, h) }).collect()
}

/// `t` at `w` x `h`, each texel mixed from the four nearest.
pub fn resample(t: &Texture, w: usize, h: usize) -> Texture {
    let rgba = (0..w * h).flat_map(|i| t.sample_smooth(((i % w) as f32 + 0.5) / w as f32, ((i / w) as f32 + 0.5) / h as f32)).collect();
    Texture { width: w, height: h, rgba }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, v: u8) -> Texture {
        Texture { width: w, height: h, rgba: vec![v; w * h * 4] }
    }

    /// The key is the picture's: the same pixels the same key, one pixel or the shape changed another.
    #[test]
    fn the_key_is_the_pictures() {
        let a = solid(2, 2, 7);
        assert_eq!(key(&a), key(&a.clone()));
        assert_eq!(key(&a).len(), 16);
        assert!(key(&a).bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        let mut b = a.clone();
        b.rgba[5] = 8;
        assert_ne!(key(&a), key(&b));
        assert_ne!(key(&solid(4, 1, 7)), key(&a), "the same bytes in another shape");
    }

    /// The source is process-wide, so this one test sets and clears it: off, nothing changes; on, a
    /// texture with a replacement is swapped whole and one without is the disc's; a broken one is
    /// refused; a set of pages comes out one size.
    #[test]
    fn swaps_only_what_the_source_has() {
        let disc = solid(2, 2, 7);
        let other = solid(2, 1, 9);
        assert!(!on());
        assert_eq!(swap(disc.clone()).rgba, disc.rgba);
        let k = key(&disc);
        set(Some(Arc::new(move |asked: &str, _: &dyn Fn() -> Vec<String>| (asked == k).then(|| solid(8, 8, 200)))));
        let r = swap(disc.clone());
        assert_eq!((r.width, r.height, r.rgba[0]), (8, 8, 200));
        assert_eq!(swap(other.clone()).rgba, other.rgba);
        let pages = swap_all(vec![disc.clone(), other.clone()]);
        assert_eq!(pages.iter().map(|t| (t.width, t.height)).collect::<Vec<_>>(), [(8, 8), (8, 8)]);
        assert_eq!(pages[1].rgba[0], 9, "resampled, not replaced");
        set(Some(Arc::new(|_: &str, _: &dyn Fn() -> Vec<String>| Some(Texture { width: 4, height: 4, rgba: vec![1; 3] }))));
        assert_eq!(swap(disc.clone()).rgba, disc.rgba);
        set(None);
        assert!(!on());
    }
}
