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
//! are [`Splat::meshes`] ([`crate::ground_mesh`]) — all but free06's erosion bands, which lay the
//! program's own erosion layer again and which the original shows no trace of (V77,
//! [`crate::ground_mesh::repeats_a_layer`]).
//!
//! The layers and the detail map are read at the mip level the original's own paint camera reads
//! them at ([`Mip`], [`paint_depth`]), which is what makes its ground soft (V97), and blurred
//! again by the target's own texels as the ground reads it back ([`paint_texel`], V101). Both are
//! blurs on the ground, the same every way, so the GPU adds them to the screen's own footprint
//! rather than stretching it. The original's finest grain past that is not the ground's: its frame
//! is 16-bit, dithered 4x4, which the port does not copy.
//!
//! None of this is 1:1: the mip levels are box-filtered halvings, not the disc's own, and the paint
//! camera's pitch and field of view are fits to live readings; the target is a blur here, not a
//! picture drawn and then read; the direction the `Angle` turns is chosen by eye; and the mesh's
//! ground-fog parameters are not modelled.

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
    /// How far the pass's texture is softened with distance ([`Mip`]); `None` for a pass that names
    /// no mips (the masks), which reads its full-size texture at every distance.
    pub mip: Option<Mip>,
}

/// A pass's mip levels as the GS picks them (V97): `MipMap_1` levels below the full size, and the
/// level at a pixel `log2(depth) + K / 16` — the GS's `LCM = 0` LOD, `Q` being one over the depth
/// in feet — kept between 0 and the last level. The original draws the paint through a camera of
/// its own, high over the ground ([`paint_depth`]), so no ground is nearer than about 55 feet to it
/// and the layers read about their quarter-size level everywhere on `free09` (`K = -65`; past 17
/// feet a layer reads its half-size level, past 67 its quarter). That is most of why the
/// original's ground is so soft. The level is a floor: where the screen squeezes a texture smaller
/// still, the renderer's own filtering goes on from there.
#[derive(Clone, Debug)]
pub struct Mip {
    /// `MipMap_1`.
    pub levels: u32,
    /// `K / 16`: the level at a depth of one foot.
    pub bias: f32,
    /// The levels below the full size, each half the last (box-filtered; the software path's).
    pub chain: std::sync::Arc<Vec<Texture>>,
}

impl PartialEq for Mip {
    fn eq(&self, other: &Mip) -> bool {
        self.levels == other.levels && self.bias == other.bias
    }
}

impl Mip {
    /// The level the original reads at `depth` feet from the eye along its view.
    pub fn level(&self, depth: f32) -> f32 {
        if !(depth > 1.0) {
            return 0.0;
        }
        (depth.log2() + self.bias).clamp(0.0, self.levels as f32)
    }

    /// [`Mip::level`] with the paint target's own texel folded in (V101): the original reads the
    /// layer at its level into the target, and the ground then reads the target back, one texel
    /// [`paint_texel`] feet wide. Both blurs stack, so their spreads add; `texel` is the layer's own
    /// texel in feet. Kept to the last level, which is all a chain holds.
    pub fn level_through_target(&self, depth: f32, texel: f32) -> f32 {
        if !(depth > 1.0) || !(texel > 0.0) {
            return self.level(depth);
        }
        let layer = self.level(depth).exp2();
        let target = paint_texel(depth) / texel;
        (0.5 * (layer * layer + target * target).log2()).clamp(0.0, self.levels as f32)
    }

    /// `levels` box-filtered halvings of `t`, for a pass whose `.psm` names them with `K`.
    pub fn of(t: &Texture, levels: u32, k: f32) -> Mip {
        let mut chain: Vec<Texture> = Vec::new();
        for _ in 0..levels {
            let above = chain.last().unwrap_or(t);
            if above.width < 2 && above.height < 2 {
                break;
            }
            chain.push(half(above));
        }
        Mip { levels: chain.len() as u32, bias: k / 16.0, chain: std::sync::Arc::new(chain) }
    }

    /// The texture at `level` (0 the full size `full`) between its two nearest levels, as a
    /// trilinear filter reads it.
    fn sample(&self, full: &Texture, level: f32, u: f32, v: f32, rgb: bool, alpha: bool) -> [u8; 4] {
        let at = |l: usize| if l == 0 { full } else { &self.chain[l - 1] };
        let lo = level.max(0.0) as usize;
        let lo = lo.min(self.chain.len());
        let f = level - lo as f32;
        let a = at(lo).sample_smooth_parts(u, v, rgb, alpha);
        if f <= 0.0 || lo >= self.chain.len() {
            return a;
        }
        let b = at(lo + 1).sample_smooth_parts(u, v, rgb, alpha);
        std::array::from_fn(|k| (a[k] as f32 + (b[k] as f32 - a[k] as f32) * f + 0.5) as u8)
    }
}

/// How high the original's paint camera stands over the ground at the least, in feet (V97).
pub const PAINT_EYE_LEAST: f32 = 75.0;
/// How far over the player's eye's height above the ground the paint camera stands, if that is higher.
pub const PAINT_EYE_OVER: f32 = 15.0;
/// The paint camera's pitch below level, as its (sine, cosine): read off the running game on
/// `free09` and `free06` (forward's y −0.732 and −0.739, from player pitches of −18° and −27°).
pub const PAINT_PITCH: (f32, f32) = (0.735, 0.678);

/// How far ground point `p` is from the original's paint camera along its view (V97), the depth
/// a pass's [`Mip`] level is read at. The original copies the player's camera into one of its
/// own, lifts it to [`PAINT_EYE_LEAST`] feet over the ground (or [`PAINT_EYE_OVER`] over the
/// player's eye, if higher), and tips it to [`PAINT_PITCH`] whatever the player's pitch; it then
/// draws the paint on a flat plane from there. So a point's depth is its distance ahead along the
/// player's heading, foreshortened by the pitch, plus the height's share; points behind the eye
/// count as under it.
pub fn paint_depth(eye: crate::V3, forward: crate::V3, p: crate::V3) -> f32 {
    let (sin, cos) = PAINT_PITCH;
    let n = (forward.x * forward.x + forward.z * forward.z).sqrt();
    let ahead = if n > 1e-4 { (((p.x - eye.x) * forward.x + (p.z - eye.z) * forward.z) / n).max(0.0) } else { 0.0 };
    let high = PAINT_EYE_LEAST.max(PAINT_EYE_OVER + eye.y - p.y);
    cos * ahead + sin * high
}

/// The nearest any ground is to the original's paint camera, in feet: what a bake that cannot know
/// the camera ([`paint_depth`]) reads the paint at, so it is never softer than the original's.
pub const PAINT_NEAREST: f32 = PAINT_EYE_LEAST * PAINT_PITCH.0;

/// The paint target's size in texels, single player (`terrainscreenspaceprologue.psm`'s
/// `renderTargetWidth` and `renderTargetHeight` in each freeride track's own archive; the `_ss`
/// archives' 384x448 is not the one a ride loads).
pub const PAINT_TARGET: (f32, f32) = (512.0, 832.0);
/// The paint camera's horizontal field of view in degrees, and the width-to-height its projection
/// keeps, read off the running game on `free09` (108.9 and 1.499; V97 saw 106.8 on another frame).
pub const PAINT_FOV: (f32, f32) = (108.9, 1.5);

/// How wide one of the paint target's texels is on the ground, in feet, `depth` feet from the paint
/// camera (V101), as a box filter that spreads as far as the read back's bilinear tent does
/// (√2 texels). Across, the texel is the view's width over 512 columns; along, its height over 832
/// rows stretched by the plane's tilt to the camera; the two are taken as one width, their
/// geometric mean, rather than as an ellipse.
pub fn paint_texel(depth: f32) -> f32 {
    let half = (PAINT_FOV.0.to_radians() / 2.0).tan();
    let across = 2.0 * half / PAINT_TARGET.0;
    let along = 2.0 * half / PAINT_FOV.1 / PAINT_TARGET.1 / PAINT_PITCH.0;
    depth.max(0.0) * (across * along).sqrt() * std::f32::consts::SQRT_2
}

/// [`paint_depth`] in WGSL, `paint_depth(p)` against the shader's `view.eye` and `view.forward`.
pub fn paint_depth_wgsl() -> String {
    let f = |v: f32| format!("{v:?}");
    format!(
        "fn paint_depth(p: vec3f) -> f32 {{
  let n = length(view.forward.xz);
  let ahead = select(0.0, max(dot(p.xz - view.eye.xz, view.forward.xz / max(n, 1e-6)), 0.0), n > 1e-4);
  let high = max({least}, {over} + view.eye.y - p.y);
  return {cos} * ahead + {sin} * high;
}}
// One paint-target texel's width on the ground, in feet, `depth` feet from the paint camera.
fn paint_texel(depth: f32) -> f32 {{
  return max(depth, 0.0) * {texel};
}}
// How wide, in texture coordinates, the original's two reads leave a layer of a `size`-texel side
// read at `level` into the paint target, which is read back over `texel` feet at `scale` per foot.
fn mip_width(size: f32, level: f32, texel: f32, scale: f32) -> f32 {{
  return length(vec2f(exp2(level) / size, texel * scale));
}}
// The screen's gradients `gx` and `gy` with a blur `width` wide every way added (V101): spreads
// add, so the footprint's two axes each grow by it, the long one barely. Not one factor on both,
// which on ground seen edge-on would blur along it four times past the floor.
fn floor_grads(gx: vec2f, gy: vec2f, width: f32) -> mat2x2f {{
  let a = gx.x * gx.x + gy.x * gy.x;
  let b = gx.x * gx.y + gy.x * gy.y;
  let c = gx.y * gx.y + gy.y * gy.y;
  let r = sqrt(0.25 * (a - c) * (a - c) + b * b);
  let long = 0.5 * (a + c) + r;
  let short = max(0.5 * (a + c) - r, 0.0);
  let toward = select(vec2f(b, long - a), vec2f(long - c, b), a >= c);
  let e = select(vec2f(1.0, 0.0), toward / length(toward), length(toward) > 1e-20);
  let w = width * width;
  return mat2x2f(e * sqrt(long + w), vec2f(-e.y, e.x) * sqrt(short + w));
}}
",
        texel = f(paint_texel(1.0)),
        least = f(PAINT_EYE_LEAST),
        over = f(PAINT_EYE_OVER),
        cos = f(PAINT_PITCH.1),
        sin = f(PAINT_PITCH.0),
    )
}

/// `t` at half the size, each texel the mean of the four it covers.
fn half(t: &Texture) -> Texture {
    let (w, h) = ((t.width / 2).max(1), (t.height / 2).max(1));
    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            for k in 0..4 {
                let mut sum = 0u32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let (sx, sy) = ((2 * x + dx).min(t.width - 1), (2 * y + dy).min(t.height - 1));
                    sum += t.rgba[4 * (sy * t.width + sx) + k] as u32;
                }
                rgba[4 * (y * w + x) + k] = ((sum + 2) / 4) as u8;
            }
        }
    }
    Texture { width: w, height: h, rgba }
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
        let mut layers: Vec<String> = Vec::new();
        let mut splat = Splat::parse(&ini::text(&psm), |name| {
            layers.push(name.to_string());
            crate::replace::parse(&read(&format!("{name}.pst"))?)
        });
        let tiles: Option<Vec<Texture>> = (1..=16)
            .map(|i| Texture::parse(&read(&format!("{track}_shadow_{i:02}.pst"))?).ok())
            .collect();
        splat.shadow = tiles.map(|t| Shadow::assemble(t, 4, extent));
        let scene = read(&format!("{track}.scn")).map(|t| crate::scene::Scene::parse(&ini::text(&t)));
        splat.meshes = scene.and_then(|s| GroundMeshes::load(archives, &drawn_meshes(archives, s.ground_meshes, &layers)));
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
            let levels = s.number("MipMap_1").unwrap_or(0.0).max(0.0) as u32;
            let mip = (levels > 0).then(|| Mip::of(&textures[index], levels, s.number("K").unwrap_or(0.0)));
            passes.push(Pass {
                texture: index,
                scale,
                turn: angle.sin_cos(),
                reads: (true, true),
                mip,
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
        self.paint_at(x, z, 0.0)
    }

    /// [`Splat::paint`] as the original's camera sees it `depth` feet ahead (V97): each pass read at
    /// its [`Mip`] level there, so far ground is as soft as the original's. A depth of 0 reads every
    /// texture at its full size, as [`Splat::paint`] does.
    pub fn paint_at(&self, x: f32, z: f32, depth: f32) -> [u8; 3] {
        let mut frame = self.passes_frame(x, z, depth);
        if let Some(m) = &self.meshes {
            frame = m.paint(x, z, frame);
        }
        frame.map(|c| c.clamp(0.0, 255.0) as u8)[..3].try_into().unwrap()
    }

    /// [`Splat::paint`] without the screen-space meshes: the passes alone (MD35, what a world draws
    /// per pixel, its baked pages giving back the roads and the light).
    pub fn paint_passes(&self, x: f32, z: f32) -> [u8; 3] {
        self.paint_passes_at(x, z, 0.0)
    }

    /// [`Splat::paint_passes`] at `depth` feet from the original's paint camera, as [`Splat::paint_at`].
    pub fn paint_passes_at(&self, x: f32, z: f32, depth: f32) -> [u8; 3] {
        self.passes_frame(x, z, depth).map(|c| c.clamp(0.0, 255.0) as u8)[..3].try_into().unwrap()
    }

    /// The frame every pass leaves, in the GS's units.
    fn passes_frame(&self, x: f32, z: f32, depth: f32) -> [f32; 4] {
        let mut frame = [0.0f32; 4];
        for p in &self.passes {
            let (s, c) = p.turn;
            let (u, v) = ((x * c + z * s) * p.scale[0], (z * c - x * s) * p.scale[1]);
            let full = &self.textures[p.texture];
            let t = match &p.mip {
                Some(m) if depth > 0.0 => {
                    let texel = 1.0 / (p.scale[0].abs().max(p.scale[1].abs()) * full.width.max(full.height).max(1) as f32);
                    m.sample(full, m.level_through_target(depth, texel), u, v, p.reads.0, p.reads.1)
                }
                _ => full.sample_smooth_parts(u, v, p.reads.0, p.reads.1),
            };
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
        frame
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

/// The scene's screen-space meshes the ground is painted with: all of them but those that lay one of
/// the program's `layers` again ([`crate::ground_mesh::repeats_a_layer`]), each model asked once.
fn drawn_meshes(archives: &[Res], placements: Vec<crate::scene::Placement>, layers: &[String]) -> Vec<crate::scene::Placement> {
    let mut asked: Vec<(String, bool)> = Vec::new();
    placements
        .into_iter()
        .filter(|p| {
            let key = p.model.to_ascii_lowercase();
            let repeats = match asked.iter().find(|(m, _)| *m == key) {
                Some(&(_, r)) => r,
                None => {
                    let r = crate::ground_mesh::repeats_a_layer(archives, &key, layers);
                    asked.push((key, r));
                    r
                }
            };
            !repeats
        })
        .collect()
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
