//! A track loaded into memory, and the camera that looks at it.
//!
//! The pieces come from three places on the disc and have to be matched up by name: a scene's
//! `SLT=bale_single_02` is `geom\bale_single_02.psg` in the track's archive, whose mesh names a
//! material `bales_01_03`, which is `tex\bales_01_03.psm`, which names the texture `bales_01_03.pst`.
//! [`Assets`] does that matching once per model and then holds everything flat, so drawing can borrow
//! it all at once.

use crate::disc::Res;
use crate::mat::{M4, V3, v3};
use crate::psg::{Model, Tri};
use crate::pst::Texture;
use crate::raster::{Frame, Shade};
use crate::scene::{Material, Preset, Scene};
use std::collections::HashMap;

/// A model with its triangles already expanded and its textures already resolved.
pub struct Loaded {
    pub model: Model,
    /// The finest level of detail, in model space.
    pub tris: Vec<Tri>,
    /// One texture index per material of the model; `None` where the material has no texture or the
    /// archive does not hold it.
    pub textures: Vec<Option<usize>>,
    /// How far the model reaches from its own origin, for culling.
    pub radius: f32,
}

/// Every model, material and texture a scene needs, loaded from the archives it is spread across.
#[derive(Default)]
pub struct Assets {
    pub models: HashMap<String, Loaded>,
    pub textures: Vec<Texture>,
    by_name: HashMap<String, usize>,
    /// What to load instead of the material a mesh names, lower-cased: a bike's `.sub` repaints the
    /// chassis it shares with every other bike.
    renames: HashMap<String, String>,
    missing: Vec<String>,
}

impl Assets {
    /// Paint one material with another's texture from now on. Set these before loading the models
    /// that use them; a model already loaded keeps the textures it was given.
    pub fn rename_materials(&mut self, pairs: &[(String, String)]) {
        for (from, to) in pairs {
            self.renames
                .insert(from.to_ascii_lowercase(), to.to_string());
        }
    }

    /// Load one model by the name a scene uses for it, from the first archive that has it. Returns
    /// false if the model is not on the disc or does not parse; the caller draws what it got.
    pub fn load(&mut self, archives: &[Res], name: &str) -> bool {
        if self.models.contains_key(name) {
            return true;
        }
        let file = format!("{name}.psg");
        let Some(bytes) = archives.iter().find_map(|a| a.read_suffix(&file).ok()) else {
            self.missing.push(file);
            return false;
        };
        let model = match Model::parse(&bytes) {
            Ok(m) => m,
            Err(e) => {
                self.missing.push(format!("{file}: {e}"));
                return false;
            }
        };
        let tris = model.triangles(0);
        let textures = model
            .materials
            .iter()
            .map(|m| self.texture_of(archives, m))
            .collect();
        let radius = tris
            .iter()
            .flat_map(|t| t.v.iter())
            .map(|v| v.pos.len())
            .fold(0.0f32, f32::max);
        self.models.insert(
            name.to_string(),
            Loaded {
                model,
                tris,
                textures,
                radius,
            },
        );
        true
    }

    /// The texture a material paints with, loading it if this is the first mesh to ask.
    fn texture_of(&mut self, archives: &[Res], material: &str) -> Option<usize> {
        if let Some(&i) = self.by_name.get(material) {
            return Some(i);
        }
        let asked = material;
        let material = match self.renames.get(&material.to_ascii_lowercase()) {
            Some(to) => to.clone(),
            None => material.to_string(),
        };
        let material = material.as_str();
        let psm = archives
            .iter()
            .find_map(|a| a.read_suffix(&format!("{material}.psm")).ok())?;
        let name = Material::parse(&crate::ini::text(&psm)).texture?;
        if let Some(&i) = self.by_name.get(&name) {
            self.by_name.insert(asked.to_string(), i);
            return Some(i);
        }
        let pst = archives
            .iter()
            .find_map(|a| a.read_suffix(&format!("{name}.pst")).ok())?;
        let texture = Texture::parse(&pst).ok()?;
        let i = self.textures.len();
        self.textures.push(texture);
        self.by_name.insert(name, i);
        self.by_name.insert(asked.to_string(), i);
        Some(i)
    }

    /// What could not be found, for a report at the end of a draw.
    pub fn missing(&self) -> &[String] {
        &self.missing
    }
}

/// Where the frame is looked at from.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub eye: V3,
    pub at: V3,
    /// Vertical field of view, in radians.
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Camera {
    /// The chase camera the game starts a race with: behind the rider, a little above, looking just
    /// ahead of them. The distances are in the disc's own unit, which is a foot — a bike's model is
    /// seven and a half of them long and a terrain quad is three.
    pub fn behind(pos: V3, dir: V3) -> Camera {
        let dir = dir.norm();
        Camera {
            eye: pos.sub(dir.scale(22.0)).add(v3(0.0, 8.0, 0.0)),
            at: pos.add(dir.scale(8.0)).add(v3(0.0, 3.5, 0.0)),
            fov: 1.0,
            near: 0.05,
            far: 4000.0,
        }
    }

    /// The camera the game itself follows a rider with, from a preset of the track's `cpresets.ini`.
    /// `AngleY` turns it around the bike (180 is straight behind), `AngleX` is measured away from
    /// straight up, and the field of view in the file is the horizontal one — `aspect` is the shape of
    /// the frame it will be drawn into, which is what turns it into the vertical one the projection
    /// wants. The original reads that term out of its viewport too, so a frame that is not 4:3 is not
    /// the disc's 4:3 picture stretched: it keeps the horizontal sweep and takes the vertical one from
    /// its own shape.
    pub fn chase(pos: V3, dir: V3, preset: &Preset, aspect: f32) -> Camera {
        let dir = dir.norm();
        let yaw = preset.angle_y.to_radians();
        // `dir` turned about the up axis, so yaw 180 points from the bike back towards the camera.
        let (sin, cos) = (yaw.sin(), yaw.cos());
        let away = v3(dir.x * cos + dir.z * sin, 0.0, dir.z * cos - dir.x * sin).norm();
        let up_from_x = (90.0 - preset.angle_x).to_radians();
        let eye = pos
            .add(away.scale(preset.distance * up_from_x.cos()))
            .add(v3(0.0, preset.distance * up_from_x.sin(), 0.0));
        Camera {
            eye,
            at: pos.add(dir.scale(preset.look_ahead)),
            fov: vertical_fov(preset.fov, aspect),
            near: 0.05,
            far: 4000.0,
        }
    }

    /// The camera's focal length in pixels, for a frame this many pixels tall: the distance at which
    /// something one pixel across subtends the same angle as the frame's own height does at 1.
    ///
    /// This is the term the original keeps beside its projection and the only camera term its level
    /// of detail test reads ([`Model::group_for`]) — which is why that test follows the window: the
    /// same model in the same place is drawn finer into a taller frame, and coarser through a wider
    /// field of view.
    pub fn focal(&self, height: usize) -> f32 {
        (height as f32 / 2.0) / (self.fov / 2.0).tan()
    }

    /// World to clip, for a frame of the given shape.
    pub fn view_projection(&self, width: usize, height: usize) -> M4 {
        let aspect = width as f32 / height as f32;
        M4::look_at(self.eye, self.at, v3(0.0, 1.0, 0.0))
            .mul(M4::perspective(self.fov, aspect, self.near, self.far))
    }
}

/// The disc's field of view — horizontal, in degrees — as the vertical one in radians that [`Camera`]
/// and [`M4::perspective`] want, for a viewport of the given aspect.
///
/// This is the original's own conversion, and `aspect` is a term it reads out of its viewport rather
/// than a constant: the engine builds its projection from the horizontal angle alone, takes the half
/// width at the near plane as `near * tan(fov / 2)`, and divides *that* by the viewport's aspect for
/// the half height. So the angle in `cpresets.ini` is horizontal on every shape of window and the
/// vertical angle follows the window — which is also what that projection does to a split screen's
/// half-height viewport: the full horizontal sweep, and about half the vertical one.
///
/// At 4:3 it answers the 68.997 degrees down that the frame in the human's save state was drawn with
/// (V2), from the 85 across that `Preset1` asks for.
pub fn vertical_fov(horizontal_degrees: f32, aspect: f32) -> f32 {
    let horizontal = horizontal_degrees.to_radians();
    2.0 * (horizontal / 2.0).tan().atan2(aspect)
}

/// How a scene's lights and fog are applied, pulled out of the `.scn` once.
pub struct Lighting {
    pub sun: V3,
    pub sun_color: [f32; 3],
    pub ambient: [f32; 3],
    pub fog_color: [u8; 3],
    pub fog_near: f32,
    pub fog_far: f32,
}

impl Lighting {
    pub fn of(scene: &Scene) -> Lighting {
        Lighting {
            sun: scene.sun(),
            sun_color: scene.sun_color(),
            ambient: scene.ambient(),
            fog_color: scene.fog_color,
            fog_near: scene.fog_near,
            fog_far: scene.fog_far,
        }
    }
}

/// Draw one model, placed by `transform`, into `frame`.
///
/// Returns how many triangles it put through the rasteriser, which is the only progress report a batch
/// render gets.
pub fn draw_model(
    frame: &mut Frame,
    loaded: &Loaded,
    assets: &Assets,
    transform: M4,
    view_projection: M4,
    eye: V3,
    light: &Lighting,
) -> usize {
    let mvp = transform.mul(view_projection);
    let mut drawn = 0;
    for tri in &loaded.tris {
        let texture = loaded
            .textures
            .get(tri.material)
            .copied()
            .flatten()
            .and_then(|i| assets.textures.get(i));
        let mut shade = [Shade {
            clip: [0.0; 4],
            uv: [0.0; 2],
            light: [1.0; 3],
            fog: 0.0,
        }; 3];
        let mut behind = 0;
        for (k, v) in tri.v.iter().enumerate() {
            let world = transform.point(v.pos);
            let normal = transform.dir(v.normal).norm();
            // Lambert against the sun, plus the scene's ambient fill; the vertex's own colour is a
            // multiplier the artists painted, with 0x80 meaning "unchanged".
            let lambert = (-normal.dot(light.sun)).max(0.0);
            let mut lit = [0.0f32; 3];
            for c in 0..3 {
                lit[c] = (light.ambient[c] + light.sun_color[c] * lambert)
                    * (v.color[c] as f32 / 128.0);
            }
            let distance = world.sub(eye).len();
            let fog = ((distance - light.fog_near) / (light.fog_far - light.fog_near).max(1.0))
                .clamp(0.0, 1.0);
            let clip = mvp.clip(v.pos);
            if clip[3] <= 0.0 {
                behind += 1;
            }
            shade[k] = Shade {
                clip,
                uv: v.uv,
                light: lit,
                fog,
            };
        }
        if behind == 3 {
            continue;
        }
        frame.triangle(shade, texture, light.fog_color);
        drawn += 1;
    }
    drawn
}

/// Draw a whole scene: every prop the `.scn` places, then whatever extra models the caller wants
/// (the rider's bike, say) at their own transforms.
pub fn draw_scene(
    frame: &mut Frame,
    scene: &Scene,
    assets: &Assets,
    extra: &[(&str, M4)],
    camera: &Camera,
) -> usize {
    let view_projection = camera.view_projection(frame.width, frame.height);
    let light = Lighting::of(scene);
    let mut drawn = 0;
    for placement in &scene.models {
        let Some(loaded) = assets.models.get(&placement.model) else {
            continue;
        };
        // Cheap cull: a prop whose bounding sphere is beyond the fog is not worth transforming.
        let distance = placement.pos.sub(camera.eye).len() - loaded.radius;
        if distance > light.fog_far {
            continue;
        }
        drawn += draw_model(
            frame,
            loaded,
            assets,
            placement.transform(),
            view_projection,
            camera.eye,
            &light,
        );
    }
    for (name, transform) in extra {
        if let Some(loaded) = assets.models.get(*name) {
            drawn += draw_model(
                frame,
                loaded,
                assets,
                *transform,
                view_projection,
                camera.eye,
                &light,
            );
        }
    }
    drawn
}
