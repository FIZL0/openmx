//! The freeride ground's paint: tiled layers blended through whole-terrain masks, a detail map over
//! the top, and a baked light map — read from the track's own `terrainscreenspacemesh.psm`.
//!
//! The original paints an off-road track's ground in two steps. It first draws the terrain from
//! above into an off-screen target, pass after pass, each pass a `.psm` shader pass with a texture,
//! a texture-coordinate transform and a GS blend; then the ground mesh samples that target. The
//! passes are a little program, and `free09`'s reads:
//!
//! ```text
//! 1  Free09_Grass       no blend                    tiled, 100 ft a repeat, turned 30 degrees
//! 2  Free09_Sand_Mask   (0 - 0) * As + Cd           colour kept, the mask's alpha into the frame's
//! 3  Free09_Sand        (Cs - Cd) * Ad + Cd         sand over grass by that alpha
//! 4  Free09_Rock_Mask, 5 Free09_Rock, 6 Free09_Dirt_Mask, 7 Free09_Dirt   the same again, twice
//! 8  Free09_Detail      (Cd - 0) * As + 0           the frame times the detail map's alpha
//! ```
//!
//! The masks' transform is `1/6912` of a foot, which is the whole 768-sample field at 9 feet a sample:
//! one mask covers the track. The layers' are a repeat every 25 to 100 feet. Every freeride track
//! has the file, with its own layers in its own order (`free06` adds an erosion layer, `free07` ash
//! and embers, and has no detail pass), so [`Splat`] runs whatever passes the file lists, as the GS
//! would: the blend inputs are read, not recognised. Here the program runs per pixel of the ground at
//! the pixel's world position, rather than into a target that is then sampled.
//!
//! The light comes from sixteen 512x512 grey tiles, `<track>_shadow_01..16.pst`, which are not named
//! by any `.psm` and are found by name. Assembled four by four they are the ground's lighting baked,
//! shadows of the hills included: against the port's own Lambert term from the scene's sun they
//! correlate at 0.87 on `free09` and 0.86 on `free10`, at one placement only — covering the middle
//! 640 of the 768 samples, with the image's rows running against the world's z. Where the map does not
//! reach, the caller lights the ground itself. On `free09` one row of tiles is stored right to left,
//! which [`Shadow::assemble`] finds by how well each row's top edge continues the row above it.
//!
//! After the passes the original draws the scene's `Type=TerrainScreenSpace` models into the same
//! target — the roads, ruts, runways and dirt loops, each flat mesh with its own `.psm` — and those
//! are [`Splat::meshes`] ([`crate::ground_mesh`]).
//!
//! None of this is 1:1: textures are sampled nearest, without the original's mip chain; the
//! direction the `Angle` turns is chosen by eye; the screen-space target's own resolution and the
//! mesh's ground-fog parameters are not modelled.

use crate::disc::Res;
use crate::ground_mesh::GroundMeshes;
use crate::ini;
use crate::pst::Texture;

/// One shader pass: a texture, where on the ground it lands, and how it lands on what is already there.
#[derive(Clone, Debug, PartialEq)]
pub struct Pass {
    /// Index into [`Splat::textures`].
    pub texture: usize,
    /// Texture coordinates per foot, `UScale` and `VScale`, after turning the ground by `angle`.
    pub scale: [f32; 2],
    /// The `UVAnimation`'s `Angle`, in radians.
    pub angle: f32,
    /// `ShaderRGBA`, with 128 meaning "unchanged" as the GS has it.
    pub rgba: [f32; 4],
    /// `inputColorA`, `inputColorB`, `inputAlphaC`, `inputColorD` when `alphaBlendingOnOff` is set:
    /// the GS's `(A - B) * C + D`, where A, B and D pick the source colour (0), the frame's (1) or
    /// zero (2), and C the source alpha (0), the frame's (1) or a fixed one (2).
    pub blend: Option<[u8; 4]>,
    /// `fixedAlphaValue`, for a blend whose C is 2.
    pub fixed: f32,
    /// `angle`'s sine and cosine, worked out once rather than at every pixel.
    pub turn: (f32, f32),
    /// Which of the texel's colour and alpha anything reads ([`Splat::reads`]): a mask's colour
    /// and a layer's alpha are never looked at, so they are never mixed.
    pub reads: (bool, bool),
}

/// A freeride track's ground paint.
pub struct Splat {
    pub textures: Vec<Texture>,
    pub passes: Vec<Pass>,
    pub shadow: Option<Shadow>,
    /// The roads, ruts and runways the scene paints in after the passes (V67).
    pub meshes: Option<GroundMeshes>,
}

/// The baked light map, assembled, and the part of the world it covers.
pub struct Shadow {
    /// The tiles in the order they are laid out, row by row from the image's top.
    pub tiles: Vec<Texture>,
    pub across: usize,
    /// The world rectangle the whole image covers: x from `x0` to `x1`, z from `z0` to `z1`, with the
    /// image's top row at `z1`.
    pub x0: f32,
    pub x1: f32,
    pub z0: f32,
    pub z1: f32,
}

/// How many of a field's samples the light map leaves out on each side, as a fraction of the field:
/// 64 of 768, fitted on `free09` and `free10` (see the module docs).
const SHADOW_MARGIN: f32 = 64.0 / 768.0;

impl Splat {
    /// The ground paint of the track whose archives these are, or `None` if it has no screen-space
    /// terrain (the supercross tracks, which paint their ground from the `.trn`).
    ///
    /// `extent` is the field's size in feet, for where the light map lands.
    pub fn load(archives: &[Res], track: &str, extent: (f32, f32)) -> Option<Splat> {
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        let psm = read("terrainscreenspacemesh.psm")?;
        let mut splat = Splat::parse(&ini::text(&psm), |name| {
            Texture::parse(&read(&format!("{name}.pst"))?).ok()
        });
        let tiles: Option<Vec<Texture>> = (1..=16)
            .map(|i| Texture::parse(&read(&format!("{track}_shadow_{i:02}.pst"))?).ok())
            .collect();
        splat.shadow = tiles.map(|t| Shadow::assemble(t, 4, extent));
        let scene = read(&format!("{track}.scn")).map(|t| crate::scene::Scene::parse(&ini::text(&t)));
        splat.meshes = scene.and_then(|s| GroundMeshes::load(archives, &s.ground_meshes));
        (!splat.passes.is_empty()).then_some(splat)
    }

    /// The passes a `.psm` lists, with `texture` to load each one's texture by name. A pass whose
    /// texture does not load is dropped, which leaves the ones under it showing.
    pub fn parse(text: &str, mut texture: impl FnMut(&str) -> Option<Texture>) -> Splat {
        let sections = ini::parse(text);
        let mut textures: Vec<Texture> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut passes = Vec::new();
        for s in &sections {
            let name = s.name.to_ascii_lowercase();
            let Some(n) = name.strip_prefix("shaderpass_") else {
                continue;
            };
            if n.contains('_') {
                continue; // an animator; read below with its pass
            }
            let Some(map) = s.get("TextureMap_1").map(|t| t.to_ascii_lowercase()) else {
                continue;
            };
            let index = match names.iter().position(|m| *m == map) {
                Some(i) => i,
                None => {
                    let Some(t) = texture(&map) else { continue };
                    textures.push(t);
                    names.push(map);
                    textures.len() - 1
                }
            };
            let mut scale = [1.0, 1.0];
            let mut angle = 0.0;
            for a in &sections {
                let an = a.name.to_ascii_lowercase();
                let mine = an.strip_prefix(&name).is_some_and(|r| r.starts_with("_animator_"));
                let uv = a.get("Algorithm").is_some_and(|v| v.trim().eq_ignore_ascii_case("UVAnimation"));
                if mine && uv {
                    scale = [a.number("UScale").unwrap_or(1.0), a.number("VScale").unwrap_or(1.0)];
                    angle = a.number("Angle").unwrap_or(0.0).to_radians();
                }
            }
            let rgba = match s.numbers("ShaderRGBA") {
                Some(v) if v.len() >= 4 => [v[0], v[1], v[2], v[3]],
                _ => [128.0; 4],
            };
            let input = |k: &str| s.number(k).unwrap_or(0.0) as u8;
            let blend = (s.number("alphaBlendingOnOff").unwrap_or(0.0) != 0.0).then(|| {
                [input("inputColorA"), input("inputColorB"), input("inputAlphaC"), input("inputColorD")]
            });
            passes.push(Pass {
                texture: index,
                scale,
                turn: angle.sin_cos(),
                reads: (true, true),
                angle,
                rgba,
                blend,
                fixed: s.number("fixedAlphaValue").unwrap_or(128.0),
            });
        }
        Splat::reads(&mut passes);
        Splat {
            textures,
            passes,
            shadow: None,
            meshes: None,
        }
    }

    /// Which half of each pass's texel the program reads. The colour, where the blend picks the
    /// source's colour (or there is no blend and it replaces the frame's). The alpha, where the
    /// blend scales by the source's alpha — or where the next pass scales by the frame's, which is
    /// this pass's alpha, since every pass writes its source alpha into the frame. The last pass's
    /// alpha goes nowhere: [`Splat::paint`] keeps the colour.
    fn reads(passes: &mut [Pass]) {
        for i in 0..passes.len() {
            let rgb = match passes[i].blend {
                None => true,
                Some([a, b, _, d]) => a == 0 || b == 0 || d == 0,
            };
            let next_reads_mine = passes.get(i + 1).is_some_and(|n| matches!(n.blend, Some([_, _, 1, _])));
            let alpha = matches!(passes[i].blend, Some([_, _, 0, _])) || next_reads_mine;
            passes[i].reads = (rgb, alpha);
        }
    }

    /// The ground's colour at a point of the world, before any light: every pass in order, blended
    /// the GS's way in its own units (alpha 128 = one), then the scene's screen-space meshes over
    /// them ([`GroundMeshes::paint`]).
    pub fn paint(&self, x: f32, z: f32) -> [u8; 3] {
        let mut frame = [0.0f32; 4];
        for p in &self.passes {
            let (s, c) = p.turn;
            let (u, v) = ((x * c + z * s) * p.scale[0], (z * c - x * s) * p.scale[1]);
            let t = self.textures[p.texture].sample_smooth_parts(u, v, p.reads.0, p.reads.1);
            // Modulate by the pass's constant colour; the `.pst` decoder doubled the alpha, so halve it
            // back to the GS's scale.
            let src = [
                t[0] as f32 * p.rgba[0] / 128.0,
                t[1] as f32 * p.rgba[1] / 128.0,
                t[2] as f32 * p.rgba[2] / 128.0,
                (t[3] as f32 * 0.5 * p.rgba[3] / 128.0).min(255.0),
            ];
            frame = blend(src, frame, p.blend, p.fixed);
        }
        if let Some(m) = &self.meshes {
            frame = m.paint(x, z, frame);
        }
        frame.map(|c| c.clamp(0.0, 255.0) as u8)[..3].try_into().unwrap()
    }

    /// The baked light at a point, 1 being "as painted"; `None` outside the light map.
    pub fn light(&self, x: f32, z: f32) -> Option<f32> {
        self.shadow.as_ref()?.at(x, z)
    }
}

/// One GS blend: `(A - B) * C / 128 + D`, writing the source's alpha into the frame's. Without a
/// blend the source replaces the frame.
#[inline]
pub fn blend(src: [f32; 4], dst: [f32; 4], inputs: Option<[u8; 4]>, fixed: f32) -> [f32; 4] {
    let Some([a, b, c, d]) = inputs else {
        return src;
    };
    let pick = |which: u8, k: usize| match which {
        0 => src[k],
        1 => dst[k],
        _ => 0.0,
    };
    let alpha = match c {
        0 => src[3],
        1 => dst[3],
        _ => fixed,
    };
    let mut out = [0.0; 4];
    for k in 0..3 {
        out[k] = ((pick(a, k) - pick(b, k)) * alpha / 128.0 + pick(d, k)).clamp(0.0, 255.0);
    }
    out[3] = src[3];
    out
}

impl Shadow {
    /// Lay `tiles` out `across` to a row, over the middle of a field `extent` feet in size.
    ///
    /// A row stored right to left is put back: for each row, both orders are tried against the
    /// bottom edge of the row above, and the one whose seam is smoother wins.
    pub fn assemble(mut tiles: Vec<Texture>, across: usize, extent: (f32, f32)) -> Shadow {
        let rows = tiles.len() / across.max(1);
        for r in 1..rows {
            let seam = |tiles: &[Texture]| -> f64 {
                let mut cost = 0.0;
                for c in 0..across {
                    let (up, down) = (&tiles[(r - 1) * across + c], &tiles[r * across + c]);
                    let w = up.width.min(down.width);
                    for x in 0..w {
                        let a = up.rgba[4 * ((up.height - 1) * up.width + x)] as f64;
                        let b = down.rgba[4 * x] as f64;
                        cost += (a - b).abs();
                    }
                }
                cost
            };
            let kept = seam(&tiles);
            tiles[r * across..(r + 1) * across].reverse();
            if seam(&tiles) >= kept {
                tiles[r * across..(r + 1) * across].reverse();
            }
        }
        let (mx, mz) = (extent.0 * SHADOW_MARGIN, extent.1 * SHADOW_MARGIN);
        Shadow {
            tiles,
            across,
            x0: mx,
            x1: extent.0 - mx,
            z0: mz,
            z1: extent.1 - mz,
        }
    }

    /// The light at a point of the world, as a multiplier: the map's grey over 128.
    pub fn at(&self, x: f32, z: f32) -> Option<f32> {
        let u = (x - self.x0) / (self.x1 - self.x0);
        let v = (self.z1 - z) / (self.z1 - self.z0);
        if !(0.0..1.0).contains(&u) || !(0.0..1.0).contains(&v) {
            return None;
        }
        let rows = self.tiles.len() / self.across;
        let (fu, fv) = (u * self.across as f32, v * rows as f32);
        let tile = &self.tiles[fv as usize * self.across + fu as usize];
        // Both are at least 0 here, where the fraction is what is left above the floor.
        let (fu, fv) = (fu - crate::pst::floor(fu), fv - crate::pst::floor(fv));
        let t = tile.sample_smooth_parts(fu, fv, true, false);
        Some(t[0] as f32 / 128.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(rgba: [u8; 4]) -> Texture {
        Texture {
            width: 2,
            height: 2,
            rgba: rgba.repeat(4),
        }
    }

    const PSM: &str = "[General]\n\
        TotalShaderPasses = 3\n\
        [ShaderPass_1]\n\
        TextureMap_1 = Grass\n\
        alphaBlendingOnOff = 0\n\
        [ShaderPass_1_Animator_1]\n\
        Algorithm = UVAnimation\n\
        UScale = -0.01\n\
        VScale = 0.01\n\
        Angle = 30\n\
        [ShaderPass_2]\n\
        TextureMap_1 = Sand_Mask\n\
        inputColorA = 2\ninputColorB = 2\ninputAlphaC = 0\ninputColorD = 1\n\
        alphaBlendingOnOff = 1\n\
        [ShaderPass_3]\n\
        TextureMap_1 = Sand\n\
        inputColorA = 0\ninputColorB = 1\ninputAlphaC = 1\ninputColorD = 1\n\
        alphaBlendingOnOff = 1\n\
        [ShaderPass_4]\n\
        TextureMap_1 = Detail\n\
        ShaderRGBA = 128.0, 128.0, 128.0, 255\n\
        inputColorA = 1\ninputColorB = 2\ninputAlphaC = 0\ninputColorD = 2\n\
        alphaBlendingOnOff = 1\n";

    fn layers(name: &str) -> Option<Texture> {
        match name {
            "grass" => Some(flat([200, 0, 0, 255])),
            // A mask the `.pst` decoder has doubled: 128 here is 64 to the GS, a half.
            "sand_mask" => Some(flat([0, 0, 0, 128])),
            "sand" => Some(flat([0, 0, 200, 255])),
            // 192 decoded, 96 to the GS, 96 * 255 / 128 = 191.25 after the pass's constant: x1.494.
            "detail" => Some(flat([0, 0, 0, 192])),
            _ => None,
        }
    }

    #[test]
    fn the_terrain_program_is_read_pass_by_pass() {
        let s = Splat::parse(PSM, layers);
        assert_eq!(s.passes.len(), 4);
        assert_eq!(s.passes[0].blend, None);
        assert_eq!(s.passes[0].scale, [-0.01, 0.01]);
        assert!((s.passes[0].angle - 30f32.to_radians()).abs() < 1e-6);
        assert_eq!(s.passes[1].blend, Some([2, 2, 0, 1]));
        assert_eq!(s.passes[3].rgba[3], 255.0);
    }

    #[test]
    fn a_layer_shows_through_its_mask_and_the_detail_multiplies() {
        let s = Splat::parse(PSM, layers);
        // Worked by hand. Grass: (200, 0, 0). The mask leaves the colour and sets the frame's alpha
        // to 64. Sand blends (0 - 200) * 64 / 128 + 200 = 100 red, (200 - 0) * 64 / 128 + 0 = 100
        // blue. The detail multiplies by 96 * 255 / 128 / 128 = 1.494: 149.4 -> 149.
        assert_eq!(s.paint(10.0, 20.0), [149, 0, 149]);
    }

    #[test]
    fn a_pass_whose_texture_is_missing_is_dropped() {
        let s = Splat::parse(PSM, |n| (n != "sand").then(|| layers(n)).flatten());
        assert_eq!(s.passes.len(), 3);
        // Grass, the mask, then the detail straight over it: 200 * 1.494.
        assert_eq!(s.paint(0.0, 0.0), [255, 0, 0]);
    }

    #[test]
    fn a_light_map_row_stored_backwards_is_turned_round() {
        // Two rows of two: the top row grey 10 then 200; the bottom row should continue it but is
        // stored right to left.
        let tiles = vec![flat([10, 10, 10, 255]), flat([200, 200, 200, 255]), flat([200; 4]), flat([10; 4])];
        let s = Shadow::assemble(tiles, 2, (1200.0, 1200.0));
        assert_eq!(s.tiles[2].rgba[0], 10);
        assert_eq!(s.tiles[3].rgba[0], 200);
        // Margins of 64/768 of the field, the image's top at the far z.
        assert_eq!((s.x0, s.x1), (100.0, 1100.0));
        assert_eq!(s.at(150.0, 1050.0), Some(10.0 / 128.0));
        assert_eq!(s.at(50.0, 600.0), None);
    }
}
