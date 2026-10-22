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
use crate::world::{Assets, Camera, Loaded};
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
        let (left, right) = self.billboard_span();
        let [u0, u1, v_bottom, v_top] = self.uv;
        let h = self.height[0];
        // The picture is lit as a crown is from above, so it keeps the 3D trees' light (by eye).
        let vert = |x: f32, y: f32, u: f32, v: f32| Vertex {
            pos: v3(x, y, 0.0),
            normal: v3(0.0, 1.0, 0.0),
            uv: [u, v],
            color: [0x80, 0x80, 0x80, 0x80],
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
                name: self.billboard_name(),
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
    let per = 16u32.pow(terrain.levels) as f32;
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

/// The original's rule for one plant: the distance across the ground, stretched by how wide the
/// camera sees (`fov`, horizontal degrees, against the 77 it was tuned at), against the scene's
/// `EcoDistance3D` (`near`) and `EcoDistance2D` (`far`).
pub fn look(eye: V3, pos: V3, fov: f32, near: f32, far: f32) -> Look {
    let (dx, dz) = (pos.x - eye.x, pos.z - eye.z);
    let d = ((dx * dx + dz * dz) * fov / 77.0).sqrt();
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

/// One thing a frame draws of the ecosystem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Draw<'a> {
    /// The model's name in [`Assets::models`].
    pub model: &'a str,
    pub at: M4,
    /// 1 is solid; below it, see-through.
    pub alpha: f32,
}

/// A scene's ecosystem, loaded and ready to draw.
#[derive(Debug)]
pub struct Grove {
    pub ecosystem: Ecosystem,
    /// Per species: the model and the billboard, by their names in [`Assets`].
    pub models: Vec<String>,
    pub billboards: Vec<String>,
    /// Per species: the model's half height and half width, which its matrix scales to the plant's.
    half: Vec<(f32, f32)>,
    /// `EcoDistance3D` and `EcoDistance2D`.
    pub near: f32,
    pub far: f32,
    /// Per plant, the way its model turned when it last came near: (cos, sin) about the up axis.
    /// Behind a lock so a frame that only borrows the scene can still turn what it brings near.
    turned: Mutex<Vec<Option<(f32, f32)>>>,
}

impl Clone for Grove {
    fn clone(&self) -> Grove {
        Grove {
            ecosystem: self.ecosystem.clone(),
            models: self.models.clone(),
            billboards: self.billboards.clone(),
            half: self.half.clone(),
            near: self.near,
            far: self.far,
            turned: Mutex::new(self.turned.lock().map(|t| t.clone()).unwrap_or_default()),
        }
    }
}

/// A camera's horizontal field of view in degrees, the measure the original's rule takes, from its
/// vertical one and the shape of the frame.
pub fn horizontal_fov(camera: &Camera, aspect: f32) -> f32 {
    (2.0 * ((camera.fov / 2.0).tan() * aspect).atan()).to_degrees()
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
            models.push(name);
            let board = s.billboard_name();
            // The texture's name less its `.tga`, as a material: `springtime2billboard.psm`.
            let material = s.billboard.split('.').next().unwrap_or("").to_ascii_lowercase();
            assets.load_model(archives, &board, s.billboard_model(&material));
            billboards.push(board);
        }
        let turned = Mutex::new(vec![None; ecosystem.plants.len()]);
        Grove { ecosystem, models, billboards, half, near, far, turned }
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
        Ok(Some(Grove::new(eco, scene.eco_distance_3d, scene.eco_distance_2d, archives, assets)))
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
    /// first, out to `reach`. A plant whose model comes into view turns it to the camera's heading
    /// and keeps it turned that way while it stays near, as the original does; one seen near for the
    /// first time turns now.
    pub fn draws<'a>(&'a self, camera: &Camera, fov: f32, reach: f32, out: &mut Vec<Draw<'a>>) {
        let heading = Grove::heading(camera);
        let mut turned = self.turned.lock().unwrap_or_else(|e| e.into_inner());
        for (i, plant) in self.ecosystem.plants.iter().enumerate() {
            let size = plant.height.max(plant.width * 2.0);
            if plant.pos.sub(camera.eye).len() > reach + size {
                continue;
            }
            let seen = look(camera.eye, plant.pos, fov, self.near, self.far);
            if seen == Look::Billboard {
                turned[i] = None;
            } else {
                let turn = *turned[i].get_or_insert(heading);
                out.push(Draw { model: &self.models[plant.species], at: self.model_at(plant, turn), alpha: 1.0 });
            }
            let alpha = match seen {
                Look::Model => continue,
                Look::Both(f) => fade_alpha(f),
                Look::Billboard => 1.0,
            };
            out.push(Draw { model: &self.billboards[plant.species], at: self.billboard_at(plant, camera), alpha });
        }
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
