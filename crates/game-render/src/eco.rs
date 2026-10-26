//! `.esb` ecosystems: the trees and bushes a scene's `[Environment]` scatters over its ground (V48).
//!
//! A free or national scene names one, `EcosystemFile=free09.esb`, with the two distances that
//! decide how each plant is drawn: nearer than `EcoDistance3D` it is its own model (`eco\
//! springtime2_treeb.psg`), farther than `EcoDistance2D` it is a flat picture of itself cut out of
//! one shared atlas (`eco\tex\springtime2billboard.psm`), and between the two it is both, the
//! picture coming in over the model. Nothing in it moves, so it needs no clock.
//!
//! The file, little-endian and packed, as the original's loader reads it:
//!
//! ```text
//! u32 version                      2 on every scene of this disc
//! 256 species slots, each:
//!   u8 present                     0: the slot is empty and nothing else follows
//!   u8 n, n bytes                  the model's name, with its NUL ("SpringTime2_TreeB")
//!   u8 n, n bytes                  the billboard's texture ("SpringTime2Billboard.tga")
//!   f32 height, min, max           feet: what an instance's height byte spreads around
//!   f32 width, min, max            feet, half the picture's width
//!   f32 u0, u1, v_bottom, v_top    the species' rectangle in the billboard atlas
//!   f32 u_trunk                    where across that rectangle the trunk stands
//!   u32 x3                         not understood (1, 1, 0x5a5b0b on every species of free09)
//!   version 2: f32 x9, u8 flag (+ u8 n, n bytes)   not understood
//!   u32 count, count x 48 bytes    solid shapes: u32 kind, vec3 at, 12 bytes, f32 radius, f32 height
//! u8 flag (+ u8 n, n bytes)        a shrubbery file; empty on this disc's free tracks
//! version 2: u32, u8 flag (+ u8 n, n bytes)   a probability map's name ("Free09_TreeProbability.tga")
//! u32 count, count x 9 bytes       the plants: u8 species, u16 x, y, z, i8 height, i8 width
//! ```
//!
//! A plant's species byte is its species' slot; its top bit asks the loader to stand it on the
//! ground, and the loader does ([`Ecosystem::stand`]): every plant of free06 asks, none of free09's.
//! Its position is in 65536ths of the map's larger side, `samples x spacing` of the terrain: on
//! free09 6912 feet, the same number the original keeps beside the plants. The two signed bytes
//! spread the species' size, height down towards its minimum and width up towards its maximum, the
//! way the original reads them: `h = H + c·(H − Hmin)/128`, `w = W + c·(Wmax − W)/128`.
//!
//! The solid shapes are an upright cylinder per tree (radius 1.5, 30 to 50 high), which the sim does
//! not have yet; drawing does not need them.

use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::psg::{Mesh, Model, Node, Strip, Vertex};
use crate::world::{Assets, Camera, Lighting, Loaded, Surface};
use std::sync::Mutex;

/// The `[Environment]` distances when a scene leaves them out, as the original's reader has them.
pub const DISTANCE_2D: f32 = 180.0;
pub const DISTANCE_3D: f32 = 120.0;
pub const SHRUBBERY_LIMIT: f32 = 400.0;

/// One kind of plant.
#[derive(Clone, Debug, PartialEq)]
pub struct Species {
    /// The model's name as the file has it, `SpringTime2_TreeB`; [`Species::model_name`] is the one
    /// [`Assets`] loads it by.
    pub model: String,
    pub billboard: String,
    /// Feet: the usual, the least and the most.
    pub height: [f32; 3],
    pub width: [f32; 3],
    /// The billboard's rectangle in its atlas: left, right, bottom, top.
    pub uv: [f32; 4],
    /// Where the trunk stands across that rectangle, as a `u`.
    pub trunk_u: f32,
    pub solids: Vec<Solid>,
}

/// A solid shape a species stands with: an upright cylinder.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solid {
    pub kind: u32,
    pub at: V3,
    pub radius: f32,
    pub height: f32,
}

/// One plant, where it stands and how big it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plant {
    /// Into [`Ecosystem::species`].
    pub species: usize,
    pub pos: V3,
    pub height: f32,
    pub width: f32,
    /// The bit that asks to be stood on the ground ([`Ecosystem::stand`]).
    pub snap: bool,
}

/// A parsed `.esb`.
#[derive(Clone, Debug, PartialEq)]
pub struct Ecosystem {
    pub version: u32,
    pub species: Vec<Species>,
    pub shrubbery: Option<String>,
    pub probability: Option<String>,
    pub plants: Vec<Plant>,
}

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.p.checked_add(n).filter(|&e| e <= self.d.len());
        let end = end.ok_or_else(|| format!("the .esb ends at {} of {} bytes it needs", self.d.len(), self.p + n))?;
        let b = &self.d[self.p..end];
        self.p = end;
        Ok(b)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn string(&mut self) -> Result<String, String> {
        let n = self.u8()? as usize;
        let b = self.take(n)?;
        let b = b.split(|&c| c == 0).next().unwrap_or(b);
        Ok(String::from_utf8_lossy(b).into_owned())
    }
    /// A flag byte and, if it is set, a string.
    fn maybe_string(&mut self) -> Result<Option<String>, String> {
        if self.u8()? != 0 { self.string().map(Some) } else { Ok(None) }
    }
}

impl Ecosystem {
    /// Read an `.esb`; `extent` is the map's larger side in feet ([`extent`]), which the plants'
    /// positions are fractions of.
    pub fn parse(d: &[u8], extent: f32) -> Result<Ecosystem, String> {
        let mut r = Reader { d, p: 0 };
        let version = r.u32()?;
        let mut species = Vec::new();
        let mut of_slot = [None; 256];
        for slot in of_slot.iter_mut() {
            if r.u8()? == 0 {
                continue;
            }
            *slot = Some(species.len());
            let model = r.string()?;
            let billboard = r.string()?;
            let mut six = [0.0; 6];
            for v in &mut six {
                *v = r.f32()?;
            }
            let (u0, u1, v_bottom, v_top, trunk_u) = (r.f32()?, r.f32()?, r.f32()?, r.f32()?, r.f32()?);
            r.take(12)?;
            if version == 2 {
                r.take(36)?;
                r.maybe_string()?;
            }
            let count = r.u32()? as usize;
            let mut solids = Vec::with_capacity(count.min(256));
            for _ in 0..count {
                let kind = r.u32()?;
                let at = v3(r.f32()?, r.f32()?, r.f32()?);
                r.take(12)?;
                solids.push(Solid { kind, at, radius: r.f32()?, height: r.f32()? });
            }
            species.push(Species {
                model,
                billboard,
                height: [six[0], six[1], six[2]],
                width: [six[3], six[4], six[5]],
                uv: [u0, u1, v_bottom, v_top],
                trunk_u,
                solids,
            });
        }
        let shrubbery = r.maybe_string()?;
        let mut probability = None;
        if version == 2 {
            r.u32()?;
            probability = r.maybe_string()?;
        }
        let count = r.u32()? as usize;
        let scale = extent / 65536.0;
        let mut plants = Vec::with_capacity(count.min(1 << 20));
        for _ in 0..count {
            let kind = r.u8()?;
            let (x, y, z) = (r.u16()?, r.u16()?, r.u16()?);
            let (ch, cw) = (r.u8()? as i8 as f32, r.u8()? as i8 as f32);
            let slot = (kind & 0x7f) as usize;
            let s = of_slot[slot].ok_or_else(|| format!("a plant of species slot {slot}, which is empty"))?;
            let sp = &species[s];
            let [h, h_min, _] = sp.height;
            let [w, _, w_max] = sp.width;
            plants.push(Plant {
                species: s,
                pos: v3(x as f32 * scale, y as f32 * scale, z as f32 * scale),
                height: h + ch * (h - h_min) / 128.0,
                width: w + cw * (w_max - w) / 128.0,
                snap: kind & 0x80 != 0,
            });
        }
        if r.p != d.len() {
            return Err(format!("{} bytes after the last plant", d.len() - r.p));
        }
        Ok(Ecosystem { version, species, shrubbery, probability, plants })
    }

    /// Stand every plant whose species byte asks for it on the ground, as the original's plant reader
    /// does while it reads them: `ground` answers the height of the terrain under a point (the point's
    /// own height where there is no terrain), and the reader keeps it the way the file keeps a height,
    /// as a whole number of 65536ths of `extent` cut towards zero, so a stood plant sinks into the
    /// ground by less than one of them. The others keep the file's height, which on free06 is up to
    /// fifty feet off the ground under it.
    pub fn stand(&mut self, extent: f32, mut ground: impl FnMut(V3) -> f32) {
        let (scale, per) = (extent / 65536.0, 65536.0 / extent);
        for p in self.plants.iter_mut().filter(|p| p.snap) {
            p.pos.y = ((ground(p.pos) * per) as i32 as u16) as f32 * scale;
        }
    }
}

impl Species {
    /// The name [`Assets::load`] finds the model by: `eco\springtime2_treeb.psg` as `springtime2_treeb`.
    pub fn model_name(&self) -> String {
        self.model.to_ascii_lowercase()
    }

    /// The name its billboard is kept under in [`Assets`], made up: the disc has no such model.
    pub fn billboard_name(&self) -> String {
        format!("{} billboard", self.model_name())
    }

    /// How far the picture reaches left and right of the trunk, in feet: its rectangle is two widths
    /// across, split where the trunk is.
    pub fn billboard_span(&self) -> (f32, f32) {
        let [u0, u1, _, _] = self.uv;
        let w = self.width[0];
        (2.0 * w * (self.trunk_u - u0) / (u1 - u0), 2.0 * w * (u1 - self.trunk_u) / (u1 - u0))
    }

    /// The billboard as a model: one quad of the usual size, standing on its origin in the xy plane,
    /// seen along +z, painted from the atlas with `material`.
    pub fn billboard_model(&self, material: &str) -> Model {
        self.billboard_painted(material, [0x80; 3], &self.billboard_name())
    }

    /// The billboard's texture's name less its `.tga`, as a material: `springtime2billboard.psm`.
    fn material(&self) -> String {
        self.billboard.split('.').next().unwrap_or("").to_ascii_lowercase()
    }

    /// The name of the billboard [`Grove::match_billboards`] paints to look like its model.
    pub fn matched_name(&self) -> String {
        format!("{} billboard, matched", self.model_name())
    }

    /// The billboard with its vertices painted `paint` (0x80 is the texture's own colour), as `name`.
    pub fn billboard_painted(&self, material: &str, paint: [u8; 3], name: &str) -> Model {
        let (left, right) = self.billboard_span();
        let [u0, u1, v_bottom, v_top] = self.uv;
        let h = self.height[0];
        // The picture is lit as a crown is from above, so it keeps the 3D trees' light (by eye).
        let vert = |x: f32, y: f32, u: f32, v: f32| Vertex {
            pos: v3(x, y, 0.0),
            normal: v3(0.0, 1.0, 0.0),
            uv: [u, v],
            color: [paint[0], paint[1], paint[2], 0x80],
            node: 0,
            adc: false,
        };
        let verts = vec![
            vert(-left, h, u0, v_top),
            vert(-left, 0.0, u0, v_bottom),
            vert(right, h, u1, v_top),
            vert(right, 0.0, u1, v_bottom),
        ];
        let half = v3((left + right) / 2.0, h / 2.0, 0.0);
        Model {
            nodes: vec![Node {
                name: name.to_string(),
                local: M4::IDENTITY,
                parent: None,
                center: v3((right - left) / 2.0, h / 2.0, 0.0),
                half_extents: half,
                radius: half.len(),
            }],
            materials: vec![material.to_string()],
            meshes: vec![Mesh { material: 0, group: 0, strips: vec![Strip { verts }] }],
            groups: 1,
            group_distance: vec![f32::MAX],
        }
    }
}

/// The map's larger side in feet, which an `.esb`'s positions are fractions of: the terrain's samples
/// times its spacing (not the last sample's place, one spacing short).
pub fn extent(layout: &game_sim::track::TerrainLayout, terrain: &game_sim::track::Terrain) -> f32 {
    let per = game_sim::world::SAMPLES_PER_WIDTH as f32;
    layout.width.max(layout.breadth) as f32 * per * terrain.scale.0
}

/// The terrain's height under a point, by the original's own point query on it (the one its plant
/// reader stands a plant with); the point's own height off the terrain.
pub fn ground_under(layout: &game_sim::track::TerrainLayout, terrain: &game_sim::track::Terrain, p: V3) -> f32 {
    let ground = game_sim::world::Ground::new(terrain, layout);
    game_sim::contact_search::ground(&ground.field(), ps2_float::Vf::new(p.x, p.y, p.z, 1.0)).point.y()
}

/// How a plant is drawn from where the camera is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Look {
    /// Near: its model only.
    Model,
    /// Between: its model, and its picture over it this far in (1..=127 of 128).
    Both(u8),
    /// Far: its picture only.
    Billboard,
}

/// The distance the original's rule measures: across the ground, stretched by how wide the camera
/// sees (`fov`, horizontal degrees, against the 77 it was tuned at).
pub fn seen_distance(eye: V3, pos: V3, fov: f32) -> f32 {
    let (dx, dz) = (pos.x - eye.x, pos.z - eye.z);
    ((dx * dx + dz * dz) * fov / 77.0).sqrt()
}

/// The original's rule for one plant: [`seen_distance`] against the scene's `EcoDistance3D`
/// (`near`) and `EcoDistance2D` (`far`).
pub fn look(eye: V3, pos: V3, fov: f32, near: f32, far: f32) -> Look {
    let d = seen_distance(eye, pos, fov);
    if d > far {
        Look::Billboard
    } else if d < near {
        Look::Model
    } else {
        match (128.0 * (d - near) / (far - near)) as u8 {
            0 => Look::Model,
            f => Look::Both(f.min(127)),
        }
    }
}

/// The billboard's opacity at a fade: the original moves a picture that far in to its solid list
/// at 0x61, and keeps the nearer ones in a list of their own, drawn faded here (by eye).
fn fade_alpha(f: u8) -> f32 {
    (f as f32 / 97.0).min(1.0)
}

/// How a grove hands a plant over from its model to its picture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lod {
    /// The original's ([`look`]): every plant at the same two distances, the picture as the atlas
    /// paints it faded in over the model, and the model dropped at the far one.
    Original,
    /// Ours (V55, presentation only): the model dissolves into the picture over a wider band, by
    /// a fixed screen pattern so neither has to be sorted, the band's middle moved per plant
    /// ([`jitter`]) so no ring of plants changes on one frame, the picture painted and lit to
    /// look like its model ([`Grove::match_billboards`]), and the farthest pictures dissolving
    /// in at the reach rather than appearing there.
    #[default]
    Smooth,
}

/// How a plant's leaves are laid over what is behind them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Foliage {
    /// The original's materials: every texel that passes their alpha test (greater than 80 since
    /// V54, not any alpha at all) mixed in by its alpha and writing depth. Filtered and mipped, a
    /// crown's edges and its far levels are part-alpha, so a leaf card can hide whatever is drawn
    /// behind it after it, where it is see-through as well as where it is not: the trees behind it,
    /// and the tree's own cards behind it, which show as pale ghosts of the cards inside the crown.
    Original,
    /// Ours (V56, presentation only): the leaves cut out at [`FOLIAGE_CUTOFF`] and drawn solid,
    /// so a texel is either all leaf or not there, and the order plants are drawn in no longer
    /// matters ([`cut_out`]).
    #[default]
    Cutout,
}

/// The alpha (0..=255) under which a [`Foliage::Cutout`] texel is not drawn: half. The leaves'
/// textures are near enough all or nothing at their own size, so this is where their edges are.
pub const FOLIAGE_CUTOFF: u8 = 128;

/// A plant material's surface under [`Foliage::Cutout`]: one mixed over the frame by alpha that
/// writes depth is instead solid where its alpha reaches [`FOLIAGE_CUTOFF`] and absent elsewhere.
/// Any other surface is left as it is.
pub fn cut_out(s: Surface) -> Surface {
    if s.blend && !s.additive && s.depth_write {
        Surface { blend: false, cutoff: s.cutoff.max(FOLIAGE_CUTOFF), depth_cutoff: 0, keep_coverage: true, ..s }
    } else {
        s
    }
}

/// The name a part's [`cut_out`] copy is kept under in [`Assets`].
pub fn cut_name(name: &str) -> String {
    format!("{name}, cut")
}

/// Keep a [`cut_out`] copy of the loaded model `name` beside it, replacing any copy made before.
fn load_cut(assets: &mut Assets, name: &str) {
    let Some(loaded) = assets.models.get(name) else { return };
    let mut cut = loaded.clone();
    cut.surfaces = cut.surfaces.into_iter().map(cut_out).collect();
    assets.models.insert(cut_name(name), cut);
}

impl Lod {
    pub const ALL: [Lod; 2] = [Lod::Smooth, Lod::Original];

    /// The settings file's word for it (P126).
    pub fn word(self) -> &'static str {
        match self {
            Lod::Original => "original",
            Lod::Smooth => "smooth",
        }
    }

    pub fn from_word(w: &str) -> Option<Lod> {
        Lod::ALL.into_iter().find(|l| l.word() == w)
    }
}

impl Foliage {
    pub const ALL: [Foliage; 2] = [Foliage::Cutout, Foliage::Original];

    /// The settings file's word for it (V57).
    pub fn word(self) -> &'static str {
        match self {
            Foliage::Original => "original",
            Foliage::Cutout => "cutout",
        }
    }

    pub fn from_word(w: &str) -> Option<Foliage> {
        Foliage::ALL.into_iter().find(|f| f.word() == w)
    }
}

/// The smooth band's width, as a share of the middle of the scene's two distances, when the scene's
/// own band is narrower. It ends where the scene's does, so no model is kept further out than the
/// original keeps it: free09's 300..350 becomes 252.5..350.
pub const BAND_SHARE: f32 = 0.3;
/// How much nearer than the scene's a plant's own band may be, as a share of the middle of the two
/// distances: up to 32 feet on free09. Only nearer, so the jitter costs no models.
pub const JITTER_SHARE: f32 = 0.1;
/// How far past its band a plant has to go before its model forgets the way it was turned, as a
/// share of the middle: the turn does not flip back and forth at the band's far edge.
pub const HOLD_SHARE: f32 = 0.1;
/// Over how much of the reach the farthest pictures dissolve in, at the least.
pub const EDGE_SHARE: f32 = 0.06;

/// A plant's own number in -1..1, the same on every frame and every machine: where in the band it
/// changes, so two plants at one distance do not change together.
pub fn jitter(plant: usize) -> f32 {
    // A 32-bit integer hash (the "lowbias32" mix): neighbours in the file come out unrelated.
    let mut x = plant as u32 ^ 0x9e37_79b9;
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    (x >> 8) as f32 / (1u32 << 23) as f32 - 1.0
}

/// How much of a plant is its picture rather than its model under [`Lod::Smooth`], 0..=1, at
/// [`seen_distance`] `d`: a smoothstep over the plant's own band.
pub fn smooth_share(d: f32, near: f32, far: f32, jitter: f32) -> f32 {
    let (start, width) = smooth_band(near, far, jitter);
    let t = ((d - start) / width).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where a plant's smooth band starts and how wide it is.
fn smooth_band(near: f32, far: f32, jitter: f32) -> (f32, f32) {
    let mid = (near + far) / 2.0;
    let width = (far - near).max(BAND_SHARE * mid).max(1.0);
    (far - width - JITTER_SHARE * mid * (1.0 - jitter) / 2.0, width)
}

/// How much of a plant `away` feet from the eye is drawn at all when nothing is drawn past `reach`:
/// 1 well inside it, dissolving to 0 at it over a stretch that differs a little per plant.
pub fn edge_share(away: f32, reach: f32, jitter: f32) -> f32 {
    if !reach.is_finite() {
        return 1.0;
    }
    let stretch = EDGE_SHARE * reach * (1.25 + 0.25 * jitter);
    ((reach - away) / stretch.max(1e-3)).clamp(0.0, 1.0)
}

/// Whether a screen pattern of `cover` keeps the pixel at `(x, y)`: an ordered 4x4 dither, so a
/// thing drawn at a cover keeps that share of its pixels, spread evenly. 1 or more keeps every one;
/// below 0 the pattern's complement keeps `-cover` of them, so a model at `c` and its picture at
/// `c - 1` share out each pixel to one of the two. The GPU's shader has the same table.
pub fn covers(x: usize, y: usize, cover: f32) -> bool {
    const BAYER: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    if cover >= 1.0 {
        return true;
    }
    let t = (BAYER[(y & 3) * 4 + (x & 3)] as f32 + 0.5) / 16.0;
    if cover >= 0.0 { t < cover } else { t >= 1.0 + cover }
}

/// Which of a species' three models a draw is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Model,
    /// The picture as the atlas paints it, the original's.
    Billboard,
    /// The picture painted to look like the model ([`Grove::match_billboards`]).
    Matched,
}

/// One thing a frame draws of the ecosystem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw<'a> {
    /// The model's name in [`Assets::models`].
    pub model: &'a str,
    pub at: M4,
    /// 1 is solid; below it, see-through.
    pub alpha: f32,
    /// The share of its pixels it draws, by [`covers`]' pattern: 1 all of them, below 0 the
    /// complement's.
    pub cover: f32,
    /// Its species, and which of the species' models: with [`Grove::parts`], a renderer's way to its
    /// own copy of the model without looking the name up.
    pub species: usize,
    pub part: Part,
    /// Its [`cut_out`] copy ([`Foliage::Cutout`]): a model or a matched billboard, never the
    /// original's picture, which [`Lod::Original`] fades in as the original does.
    pub cut: bool,
}

impl Draw<'_> {
    /// Its place in [`Grove::parts`] of a grove of `species` species.
    pub fn part_index(&self, species: usize) -> usize {
        let k = match self.part {
            Part::Model => 0,
            Part::Billboard => 1,
            Part::Matched => 2,
        };
        // The cut copies follow, of the models and the matched billboards only.
        let k = if self.cut { 3 + k / 2 } else { k };
        k * species + self.species
    }
}

/// A scene's ecosystem, loaded and ready to draw.
#[derive(Debug)]
pub struct Grove {
    pub ecosystem: Ecosystem,
    /// Per species: the model and the billboard, by their names in [`Assets`].
    pub models: Vec<String>,
    pub billboards: Vec<String>,
    /// Per species: the billboard painted to look like the model.
    pub matched: Vec<String>,
    /// The [`cut_out`] copies' names: of the models, then of the matched billboards.
    pub cut: Vec<String>,
    /// Per species: the model's half height and half width, which its matrix scales to the plant's.
    half: Vec<(f32, f32)>,
    /// `EcoDistance3D` and `EcoDistance2D`.
    pub near: f32,
    pub far: f32,
    /// How plants change from model to picture; the original's way is this one setting away.
    pub lod: Lod,
    /// How their leaves are laid over what is behind them; the original's way is this one setting
    /// away too.
    pub foliage: Foliage,
    /// Per plant, the way its model turned when it last came near: (cos, sin) about the up axis.
    /// Behind a lock so a frame that only borrows the scene can still turn what it brings near.
    turned: Mutex<Vec<Option<(f32, f32)>>>,
    /// Per plant, [`jitter`].
    jitter: Vec<f32>,
}

impl Clone for Grove {
    fn clone(&self) -> Grove {
        Grove {
            ecosystem: self.ecosystem.clone(),
            models: self.models.clone(),
            billboards: self.billboards.clone(),
            matched: self.matched.clone(),
            cut: self.cut.clone(),
            half: self.half.clone(),
            near: self.near,
            far: self.far,
            lod: self.lod,
            foliage: self.foliage,
            turned: Mutex::new(self.turned.lock().map(|t| t.clone()).unwrap_or_default()),
            jitter: self.jitter.clone(),
        }
    }
}

/// A camera's horizontal field of view in degrees, the measure the original's rule takes, from its
/// vertical one and the shape of the frame.
pub fn horizontal_fov(camera: &Camera, aspect: f32) -> f32 {
    (2.0 * ((camera.fov / 2.0).tan() * aspect).atan()).to_degrees()
}

/// What a loaded model looks like on average: the mean colour its texels are drawn in, each by its
/// share of the picture (its area and its alpha), lit by `light` if there is one — turned every way
/// about the up axis, as a plant's model may be — or by its vertex colours alone.
fn appearance(loaded: &Loaded, assets: &Assets, light: Option<&Lighting>) -> [f32; 3] {
    const TURNS: usize = 8;
    // A fixed spread of points over a triangle, in barycentric weights.
    const POINTS: [(f32, f32); 10] = [
        (1.0 / 3.0, 1.0 / 3.0),
        (0.1, 0.1),
        (0.8, 0.1),
        (0.1, 0.8),
        (0.45, 0.1),
        (0.1, 0.45),
        (0.45, 0.45),
        (0.6, 0.25),
        (0.25, 0.6),
        (0.2, 0.2),
    ];
    let (mut sum, mut weight) = ([0.0f64; 3], 0.0f64);
    for t in &loaded.tris {
        let area = t.v[1].pos.sub(t.v[0].pos).cross(t.v[2].pos.sub(t.v[0].pos)).len() as f64;
        if area <= 0.0 {
            continue;
        }
        let surface = loaded.surfaces.get(t.material).copied().unwrap_or_default();
        let texture = loaded.textures.get(t.material).copied().flatten().and_then(|i| assets.textures.get(i));
        // The triangle's light: its vertices', each over every turn.
        let mut lit = [0.0f32; 3];
        for v in &t.v {
            match light {
                Some(light) => {
                    for k in 0..TURNS {
                        let (s, c) = (k as f32 * std::f32::consts::TAU / TURNS as f32).sin_cos();
                        let n = v3(c * v.normal.x + s * v.normal.z, v.normal.y, -s * v.normal.x + c * v.normal.z);
                        let l = crate::world::vertex_light(surface.shading, light, n, v.color);
                        for (o, l) in lit.iter_mut().zip(l) {
                            *o += l / (3 * TURNS) as f32;
                        }
                    }
                }
                None => {
                    for (o, c) in lit.iter_mut().zip(v.color) {
                        *o += c as f32 / 128.0 / 3.0;
                    }
                }
            }
        }
        for &(b1, b2) in &POINTS {
            let b0 = 1.0 - b1 - b2;
            let uv = |k: usize| t.v[0].uv[k] * b0 + t.v[1].uv[k] * b1 + t.v[2].uv[k] * b2;
            let texel = texture.map_or([255; 4], |tx| tx.sample(uv(0), uv(1)));
            if texel[3] < surface.cutoff {
                continue;
            }
            let w = area * texel[3] as f64;
            for k in 0..3 {
                sum[k] += w * (texel[k] as f32 * lit[k]) as f64;
            }
            weight += w;
        }
    }
    if weight <= 0.0 {
        return [128.0; 3];
    }
    sum.map(|s| (s / weight) as f32)
}

impl Grove {
    /// The grove from an ecosystem, its models and billboards put into `assets` from `archives`.
    pub fn new(ecosystem: Ecosystem, near: f32, far: f32, archives: &[Res], assets: &mut Assets) -> Grove {
        let mut models = Vec::new();
        let mut billboards = Vec::new();
        let mut half = Vec::new();
        for s in &ecosystem.species {
            let name = s.model_name();
            assets.load(archives, &name);
            let h = assets
                .models
                .get(&name)
                .and_then(|l| l.model.nodes.first())
                .map_or((1.0, 1.0), |n| (n.half_extents.y, n.half_extents.x));
            half.push(h);
            load_cut(assets, &name);
            models.push(name);
            let board = s.billboard_name();
            assets.load_model(archives, &board, s.billboard_model(&s.material()));
            billboards.push(board);
        }
        let matched: Vec<String> = ecosystem.species.iter().map(Species::matched_name).collect();
        let cut = models.iter().chain(&matched).map(|n| cut_name(n)).collect();
        let turned = Mutex::new(vec![None; ecosystem.plants.len()]);
        let jitter = (0..ecosystem.plants.len()).map(jitter).collect();
        let mut grove = Grove { ecosystem, models, billboards, matched, cut, half, near, far, lod: Lod::default(), foliage: Foliage::default(), turned, jitter };
        grove.match_billboards(None, archives, assets);
        grove
    }

    /// The scene's grove, if it names an ecosystem the archives have, its plants placed over the
    /// track's terrain (`layout` the scene's statement of its size) and stood on it where they ask.
    pub fn load(
        archives: &[Res],
        scene: &crate::scene::Scene,
        layout: &game_sim::track::TerrainLayout,
        terrain: &game_sim::track::Terrain,
        assets: &mut Assets,
    ) -> Result<Option<Grove>, String> {
        let Some(file) = scene.ecosystem_file.as_deref() else { return Ok(None) };
        let bytes = archives
            .iter()
            .find_map(|a| a.read_suffix(file).ok())
            .ok_or_else(|| format!("no {file} in the track's archives"))?;
        let extent = extent(layout, terrain);
        let mut eco = Ecosystem::parse(&bytes, extent).map_err(|e| format!("{file}: {e}"))?;
        eco.stand(extent, |p| ground_under(layout, terrain, p));
        let mut grove = Grove::new(eco, scene.eco_distance_3d, scene.eco_distance_2d, archives, assets);
        grove.match_billboards(Some(&Lighting::of(scene)), archives, assets);
        Ok(Some(grove))
    }

    /// Every model the grove draws, by name: the species' models, then their billboards, then the
    /// matched billboards, then the [`cut_out`] copies of the models and of the matched billboards
    /// ([`Draw::part_index`]).
    pub fn parts(&self) -> impl Iterator<Item = &String> {
        self.models.iter().chain(&self.billboards).chain(&self.matched).chain(&self.cut)
    }

    /// Paint each species' matched billboard so that, on average, it is drawn in its model's
    /// colours: the atlas's picture of a tree is lighter and yellower than the tree's own
    /// textures, and lit as a crown from above it takes far more of the sun than the model's
    /// upright leaves do (V55). The paint is the model's mean drawn colour over the picture's under
    /// `light` (the scene's, every way the model may be turned), or under their vertex colours
    /// alone with none. Presentation only, and by eye: a picture cannot have its model's shading.
    pub fn match_billboards(&mut self, light: Option<&Lighting>, archives: &[Res], assets: &mut Assets) {
        for (s, sp) in self.ecosystem.species.iter().enumerate() {
            let (Some(model), Some(board)) = (assets.models.get(&self.models[s]), assets.models.get(&self.billboards[s])) else {
                continue;
            };
            let (want, got) = (appearance(model, assets, light), appearance(board, assets, light));
            let paint = std::array::from_fn(|k| (128.0 * want[k] / got[k].max(1.0)).clamp(32.0, 255.0).round() as u8);
            assets.models.remove(&self.matched[s]);
            assets.load_model(archives, &self.matched[s], sp.billboard_painted(&sp.material(), paint, &self.matched[s]));
            load_cut(assets, &self.matched[s]);
        }
    }

    /// The matrix of a plant's model, turned `(cos, sin)` about the up axis and scaled from the
    /// species' bounds to the plant's height and width, as the original builds it.
    pub fn model_at(&self, plant: &Plant, (c, s): (f32, f32)) -> M4 {
        let (half_h, half_w) = self.half[plant.species];
        let sw = plant.width / half_w;
        let sh = plant.height * 0.5 / half_h;
        let p = plant.pos;
        M4([[c * sw, 0.0, s * sw, 0.0], [0.0, sh, 0.0, 0.0], [-s * sw, 0.0, c * sw, 0.0], [p.x, p.y, p.z, 1.0]])
    }

    /// The matrix of a plant's billboard, standing on its spot and facing the camera across the
    /// ground, scaled from the species' size to the plant's.
    pub fn billboard_at(&self, plant: &Plant, camera: &Camera) -> M4 {
        let sp = &self.ecosystem.species[plant.species];
        let (sw, sh) = (plant.width / sp.width[0], plant.height / sp.height[0]);
        let f = camera.at.sub(camera.eye);
        let flat = v3(f.x, 0.0, f.z);
        let flat = if flat.len() > 1e-6 { flat.norm() } else { v3(0.0, 0.0, 1.0) };
        // The screen's right, as `M4::look_at` has it, and the picture's +z along the view.
        let right = v3(0.0, 1.0, 0.0).cross(flat).norm();
        let p = plant.pos;
        M4([
            [right.x * sw, right.y * sw, right.z * sw, 0.0],
            [0.0, sh, 0.0, 0.0],
            [flat.x, flat.y, flat.z, 0.0],
            [p.x, p.y, p.z, 1.0],
        ])
    }

    /// Which way a model faces when it is turned for `camera`: the original's two shorts out of the
    /// camera's heading, `(z, -x)` of its forward across the ground.
    fn heading(camera: &Camera) -> (f32, f32) {
        let f = camera.at.sub(camera.eye);
        let l = (f.x * f.x + f.z * f.z).sqrt();
        if f.z == 0.0 || l == 0.0 { (1.0, 0.0) } else { (f.z / l, -f.x / l) }
    }

    /// What one view draws of the grove from `camera` (`fov` its horizontal degrees), nearest
    /// first, out to `reach`, by [`Grove::lod`]'s rule. A plant whose model comes into view turns it
    /// to the camera's heading and keeps it turned that way while it stays near, as the original
    /// does; one seen near for the first time turns now. Plants outside the camera's view cone
    /// are left out (their turns are kept as if they were drawn).
    pub fn draws<'a>(&'a self, camera: &Camera, fov: f32, reach: f32, out: &mut Vec<Draw<'a>>) {
        let heading = Grove::heading(camera);
        let cone = ViewCone::of(camera, fov);
        let mut turned = self.turned.lock().unwrap_or_else(|e| e.into_inner());
        let mid = (self.near + self.far) / 2.0;
        for (i, plant) in self.ecosystem.plants.iter().enumerate() {
            let size = plant.height.max(plant.width * 2.0);
            let away = plant.pos.sub(camera.eye);
            let away_len = away.len();
            if away_len > reach + size {
                continue;
            }
            let d = seen_distance(camera.eye, plant.pos, fov);
            // How much of it is its picture, and whether its model is drawn at all.
            let (share, model) = match self.lod {
                Lod::Original => match look(camera.eye, plant.pos, fov, self.near, self.far) {
                    Look::Model => (0.0, true),
                    Look::Both(f) => (fade_alpha(f), true),
                    Look::Billboard => (1.0, false),
                },
                Lod::Smooth => {
                    let share = smooth_share(d, self.near, self.far, self.jitter[i]);
                    (share, share < 1.0)
                }
            };
            if model {
                turned[i].get_or_insert(heading);
            } else if self.lod == Lod::Original {
                turned[i] = None;
            } else {
                // Held a little past the band, so the turn does not flip at its edge.
                let (start, width) = smooth_band(self.near, self.far, self.jitter[i]);
                if d > start + width + HOLD_SHARE * mid {
                    turned[i] = None;
                }
            }
            if !cone.sees(away, away_len, size) {
                continue;
            }
            let draw = |part: Part, at: M4, alpha: f32, cover: f32| {
                let cut = self.foliage == Foliage::Cutout && part != Part::Billboard;
                let species = self.ecosystem.species.len();
                let model = match part {
                    Part::Model if cut => &self.cut[plant.species],
                    Part::Matched if cut => &self.cut[species + plant.species],
                    Part::Model => &self.models[plant.species],
                    Part::Billboard => &self.billboards[plant.species],
                    Part::Matched => &self.matched[plant.species],
                };
                Draw { model, at, alpha, cover, species: plant.species, part, cut }
            };
            match self.lod {
                Lod::Original => {
                    if model {
                        let turn = turned[i].unwrap_or(heading);
                        out.push(draw(Part::Model, self.model_at(plant, turn), 1.0, 1.0));
                    }
                    if share > 0.0 {
                        out.push(draw(Part::Billboard, self.billboard_at(plant, camera), share, 1.0));
                    }
                }
                Lod::Smooth => {
                    let edge = edge_share(away_len, reach, self.jitter[i]);
                    if edge <= 0.0 {
                        continue;
                    }
                    // The model keeps the pattern's first `1 - share`, the picture the rest.
                    let keep = (1.0 - share) * edge;
                    if model && keep > 0.0 {
                        let turn = turned[i].unwrap_or(heading);
                        let cover = if keep >= 1.0 { 1.0 } else { keep };
                        out.push(draw(Part::Model, self.model_at(plant, turn), 1.0, cover));
                    }
                    let shown = share * edge;
                    if shown > 0.0 {
                        let cover = if shown >= 1.0 { 1.0 } else { -shown };
                        out.push(draw(Part::Matched, self.billboard_at(plant, camera), 1.0, cover));
                    }
                }
            }
        }
    }
}

/// A camera's view as a cone about its forward, wide enough for the frame's corners: what
/// [`Grove::draws`] leaves out without building a matrix.
struct ViewCone {
    forward: V3,
    cos: f32,
    sin: f32,
}

impl ViewCone {
    fn of(camera: &Camera, fov: f32) -> ViewCone {
        let f = camera.at.sub(camera.eye);
        let forward = if f.len() > 1e-6 { f.norm() } else { v3(0.0, 0.0, 1.0) };
        let (h, v) = ((fov.to_radians() / 2.0).tan(), (camera.fov / 2.0).tan());
        let half = (h * h + v * v).sqrt().atan().min(std::f32::consts::FRAC_PI_2);
        ViewCone { forward, cos: half.cos(), sin: half.sin() }
    }

    /// Whether a sphere `radius` round, `away` from the eye (`len` long), reaches into the cone.
    fn sees(&self, away: V3, len: f32, radius: f32) -> bool {
        if len <= radius {
            return true;
        }
        let along = away.dot(self.forward);
        let across = (len * len - along * along).max(0.0).sqrt();
        across * self.cos - along * self.sin <= radius
    }
}

impl Assets {
    /// Keep a model made here rather than read from the disc under `name`, its materials resolved
    /// from `archives` as a loaded one's are. Returns false if the name is taken.
    pub fn load_model(&mut self, archives: &[Res], name: &str, model: Model) -> bool {
        if self.models.contains_key(name) {
            return false;
        }
        let read = |file: &str| archives.iter().find_map(|a| a.read_suffix(file).ok());
        let (textures, surfaces) = model.materials.iter().map(|m| self.resolve(&read, m)).unzip();
        let tris = model.triangles(0);
        let radius = tris.iter().flat_map(|t| t.v.iter()).map(|v| v.pos.len()).fold(0.0f32, f32::max);
        self.models.insert(name.to_string(), Loaded { model, tris, textures, surfaces, radius });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(b: &mut Vec<u8>, v: f32) {
        b.extend_from_slice(&v.to_le_bytes());
    }
    fn u(b: &mut Vec<u8>, v: u32) {
        b.extend_from_slice(&v.to_le_bytes());
    }
    fn s(b: &mut Vec<u8>, t: &str) {
        b.push(t.len() as u8 + 1);
        b.extend_from_slice(t.as_bytes());
        b.push(0);
    }

    /// A version-2 file with one species in slot 3 and two plants of it.
    fn made_up() -> Vec<u8> {
        let mut b = Vec::new();
        u(&mut b, 2);
        for slot in 0..256 {
            if slot != 3 {
                b.push(0);
                continue;
            }
            b.push(1);
            s(&mut b, "SpringTime2_TreeB");
            s(&mut b, "SpringTime2Billboard.tga");
            for v in [50.0, 30.0, 60.0, 20.0, 15.0, 28.0, 0.25, 0.5, 1.0, 0.0, 0.3125] {
                f(&mut b, v);
            }
            for v in [1, 1, 0x5a5b0b] {
                u(&mut b, v);
            }
            for _ in 0..9 {
                f(&mut b, 0.5);
            }
            b.push(0);
            u(&mut b, 1);
            u(&mut b, 1);
            for v in [0.0, 0.0, 0.0] {
                f(&mut b, v);
            }
            b.extend_from_slice(&[0xcd; 12]);
            f(&mut b, 1.5);
            f(&mut b, 40.0);
        }
        b.push(0);
        u(&mut b, 0);
        b.push(1);
        s(&mut b, "Made_TreeProbability.tga");
        u(&mut b, 2);
        b.push(3);
        for v in [32768u16, 1000, 16384] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[64, 0x80]);
        b.push(0x83);
        for v in [0u16, 0, 65535] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        b.extend_from_slice(&[0x80, 127]);
        b
    }

    #[test]
    fn a_made_up_esb_reads_back_its_species_and_plants() {
        let e = Ecosystem::parse(&made_up(), 6553.6).unwrap();
        assert_eq!(e.species.len(), 1);
        let sp = &e.species[0];
        assert_eq!((sp.model.as_str(), sp.billboard.as_str()), ("SpringTime2_TreeB", "SpringTime2Billboard.tga"));
        assert_eq!(sp.solids, vec![Solid { kind: 1, at: v3(0.0, 0.0, 0.0), radius: 1.5, height: 40.0 }]);
        assert_eq!(e.probability.as_deref(), Some("Made_TreeProbability.tga"));
        let [a, b] = [e.plants[0], e.plants[1]];
        assert_eq!(a.species, 0);
        assert!((a.pos.x - 3276.8).abs() < 1e-3 && (a.pos.y - 100.0).abs() < 1e-3 && (a.pos.z - 1638.4).abs() < 1e-3);
        // Height down towards the least, width up towards the most, by the signed bytes over 128.
        assert_eq!((a.height, a.width), (50.0 + 64.0 * 20.0 / 128.0, 20.0 - 128.0 * 8.0 / 128.0));
        assert_eq!((b.height, b.width), (50.0 - 128.0 * 20.0 / 128.0, 20.0 + 127.0 * 8.0 / 128.0));
        assert!(b.snap && !a.snap);
        // The rectangle is two widths across, split at the trunk.
        assert_eq!(sp.billboard_span(), (10.0, 30.0));
        let mut short = made_up();
        short.pop();
        assert!(Ecosystem::parse(&short, 1.0).is_err());
        // A plant of an empty slot.
        let mut stray = made_up();
        let at = stray.len() - 18;
        stray[at] = 2;
        assert!(Ecosystem::parse(&stray, 1.0).is_err());
    }

    #[test]
    fn the_distance_rule_is_the_originals() {
        // free06's eco distances in the biplane recording's state, at its 85 degrees: the fade bytes
        // the original had written for two of its trees.
        let at = |d: f32| look(v3(0.0, 0.0, 0.0), v3(d, 0.0, 0.0), 85.0, 300.0, 350.0);
        assert_eq!(at(312.1 / (85.0f32 / 77.0).sqrt()), Look::Both(30));
        assert_eq!(at(342.5 / (85.0f32 / 77.0).sqrt()), Look::Both(108));
        assert_eq!(at(100.0), Look::Model);
        assert_eq!(at(340.0), Look::Billboard);
        assert_eq!(fade_alpha(30) < 1.0, fade_alpha(108) == 1.0);
    }
}
