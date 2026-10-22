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
use crate::raster::{Batch, Frame, Mode, Shade};
use crate::scene::{Material, Preset, Scene, Shading};
use std::collections::HashMap;

/// A model with its triangles already expanded and its textures already resolved.
///
/// Cloned for a split screen's second rider and on (V22): a skinned model's triangles are its
/// pose, so each rider on the screen needs a copy of its own to be posed into.
#[derive(Clone)]
pub struct Loaded {
    pub model: Model,
    /// The finest level of detail, in model space.
    pub tris: Vec<Tri>,
    /// One texture index per material of the model; `None` where the material has no texture or the
    /// archive does not hold it.
    pub textures: Vec<Option<usize>>,
    /// One surface per material of the model, in the same order: how it is lit and laid over the
    /// frame (V25).
    pub surfaces: Vec<Surface>,
    /// How far the model reaches from its own origin, for culling.
    pub radius: f32,
}

/// What a material's `.psm` says about drawing it, beyond which texture: the light, the alpha test
/// and the blend of its first shader pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub shading: Shading,
    /// Texels under this alpha (0..=255) are not drawn.
    pub cutoff: u8,
    /// Mixed over the frame by alpha; `additive` adds instead.
    pub blend: bool,
    pub additive: bool,
    pub depth_write: bool,
    /// The material's texture is clamped at its edges along u and along v, not repeated.
    pub clamp: [bool; 2],
}

impl Default for Surface {
    /// A material with no `.psm`, or one that says nothing: lit by the sun, opaque, and with the
    /// hole-punching cut-out every texture had before materials were read.
    fn default() -> Surface {
        Surface {
            shading: Shading::Diffuse,
            cutoff: 32,
            blend: false,
            additive: false,
            depth_write: true,
            clamp: [false; 2],
        }
    }
}

impl Surface {
    pub fn of(m: &Material) -> Surface {
        let blend = m.blend.is_some();
        // `(Cs - 0) * As + Cd`: the source is added rather than mixed.
        let additive = matches!(m.blend, Some([0, 2, 1]));
        let cutoff = match m.alpha_test {
            // The reference is in the GS's units, where 128 is opaque; the decoder doubles alpha.
            Some(Some(r)) => (r * 2.0).clamp(0.0, 255.0) as u8,
            Some(None) => 1,
            // A blended material with no test draws all but the empty texels, faintly.
            None if blend => 1,
            None => 32,
        };
        Surface {
            shading: m.shading(),
            cutoff,
            blend,
            additive,
            // A texel that fails the test but still lands in colour must not hide what is behind it:
            // the port has no "colour only" write, so the whole material skips depth instead.
            depth_write: !m.no_depth_write && !(blend && m.fail_keeps_color && cutoff > 1),
            clamp: m.clamp,
        }
    }
}

/// The files a material's pixels may be in, in the order they are tried: its texture as a `.pst`,
/// then as an `.atm`, then the `.atm` its `TextureAnimation` names (`unique` meaning its own name).
pub fn texture_files(material: &str, m: &Material) -> Vec<String> {
    let mut files = Vec::new();
    if let Some(t) = &m.texture {
        files.push(format!("{t}.pst"));
        files.push(format!("{t}.atm"));
    }
    match m.animation.as_deref() {
        Some(a) if a.eq_ignore_ascii_case("unique") => files.push(format!("{material}.atm")),
        Some(a) => files.push(format!("{a}.atm")),
        None => {}
    }
    files
}

/// A texture file decoded by its kind: a `.pst` whole, an `.atm` by its first frame.
pub fn decode_texture(file: &str, bytes: &[u8]) -> Option<Texture> {
    if file.to_ascii_lowercase().ends_with(".atm") {
        Texture::parse_animation(bytes).ok()
    } else {
        Texture::parse(bytes).ok()
    }
}

/// How one vertex is lit by its material's class: the light the rasteriser multiplies its texel by.
///
/// A diffuse material takes the scene's sun on its normal plus the ambient fill, times the vertex
/// colour the artists painted (0x80 meaning "unchanged"); a prelit one has its light baked into that
/// colour and ignores the sun; a constant-colour one is one colour all over.
pub fn vertex_light(shading: Shading, light: &Lighting, normal: V3, color: [u8; 4]) -> [f32; 3] {
    let mut lit = [0.0f32; 3];
    let lambert = (-normal.dot(light.sun)).max(0.0);
    for c in 0..3 {
        let paint = color[c] as f32 / 128.0;
        lit[c] = match shading {
            Shading::Diffuse => (light.ambient[c] + light.sun_color[c] * lambert) * paint,
            Shading::Prelit => paint,
            Shading::Constant(rgba) => rgba[c] / 128.0,
        };
    }
    lit
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
    /// Textures made rather than read, by file name lower-cased: a rider's own name and number
    /// painted over `NameDefault.pst` (P86). Answers before the archives do.
    made: HashMap<String, Texture>,
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

    /// Answer a texture file with `texture` from now on instead of reading it, or read it again with
    /// `None`. Like [`Assets::rename_materials`] it is for models loaded after it.
    pub fn make_texture(&mut self, file: &str, texture: Option<Texture>) {
        match texture {
            Some(t) => self.made.insert(file.to_ascii_lowercase(), t),
            None => self.made.remove(&file.to_ascii_lowercase()),
        };
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
        let (textures, surfaces) = model
            .materials
            .iter()
            .map(|m| self.texture_of(archives, m))
            .unzip();
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
                surfaces,
                radius,
            },
        );
        true
    }

    /// Load a model the scene places, [`Assets::load`] but with the placement standing in for the
    /// model's root node: the original draws a scene prop's vertices with the prop's own matrix and
    /// nothing above it. The exporter left one instance's rotation in the root of many of these files
    /// (free09's and free10's telephone wires, free06's pylons, bridges, fences, a runway), so that
    /// root turned on top of the scene's look and up laid a wire's sag sideways and stood a pylon
    /// off the line its wires hang from (V41).
    ///
    /// The path cars a track's `.toy` drives are loaded this way too: the original's car makes the
    /// same model object, and free06's train_car_03/04/05 carry a half turn in the root (V46).
    pub fn load_placed(&mut self, archives: &[Res], name: &str) -> bool {
        let fresh = !self.models.contains_key(name);
        if !self.load(archives, name) {
            return false;
        }
        if let Some(loaded) = self.models.get_mut(name).filter(|_| fresh) {
            for node in loaded.model.nodes.iter_mut().filter(|n| n.parent.is_none()) {
                node.local = M4::IDENTITY;
            }
            loaded.tris = loaded.model.triangles(0);
        }
        true
    }

    /// Load `name` a second time as `own`, painted with `pairs` over the renames already set: another
    /// rider on the one shared body in gear of their own (P37). The materials' earlier answers are not
    /// reused for it, and the renames and answers the first load left are back afterwards.
    pub fn load_repainted(&mut self, archives: &[Res], name: &str, own: &str, pairs: &[(String, String)]) -> bool {
        let saved = (std::mem::take(&mut self.by_name), self.renames.clone());
        self.rename_materials(pairs);
        let first = self.models.remove(name);
        let loaded = self.load(archives, name);
        if let Some(m) = self.models.remove(name) {
            self.models.insert(own.to_string(), m);
        }
        if let Some(f) = first {
            self.models.insert(name.to_string(), f);
        }
        (self.by_name, self.renames) = saved;
        loaded
    }

    /// The texture a material paints with, loading it if this is the first mesh to ask, and how the
    /// material is drawn.
    fn texture_of(&mut self, archives: &[Res], material: &str) -> (Option<usize>, Surface) {
        self.resolve(
            &|file: &str| archives.iter().find_map(|a| a.read_suffix(file).ok()),
            material,
        )
    }

    /// [`Assets::texture_of`] against any reader of archive members, so it can be tried on bytes
    /// made up for a test.
    pub fn resolve(
        &mut self,
        read: &dyn Fn(&str) -> Option<Vec<u8>>,
        material: &str,
    ) -> (Option<usize>, Surface) {
        let asked = material;
        let material = match self.renames.get(&material.to_ascii_lowercase()) {
            Some(to) => to.clone(),
            None => material.to_string(),
        };
        let material = material.as_str();
        let Some(psm) = read(&format!("{material}.psm")) else {
            return (self.by_name.get(asked).copied(), Surface::default());
        };
        let m = Material::parse(&crate::ini::text(&psm));
        let surface = Surface::of(&m);
        if let Some(&i) = self.by_name.get(asked) {
            return (Some(i), surface);
        }
        for file in texture_files(material, &m) {
            if let Some(&i) = self.by_name.get(&file) {
                self.by_name.insert(asked.to_string(), i);
                return (Some(i), surface);
            }
            let made = self.made.get(&file.to_ascii_lowercase()).cloned();
            let Some(texture) = made.or_else(|| read(&file).and_then(|b| decode_texture(&file, &b))) else {
                continue;
            };
            let i = self.textures.len();
            self.textures.push(texture);
            self.by_name.insert(file, i);
            self.by_name.insert(asked.to_string(), i);
            return (Some(i), surface);
        }
        (None, surface)
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
            fov: view_fov(preset.fov, aspect),
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

/// The narrowest view the original ever draws: the side-by-side split's half of a 4:3 television,
/// two wide and three tall once the PS2's pixels are put back square.
pub const NARROWEST_VIEW: f32 = 2.0 / 3.0;
/// The widest: the stacked split's strip of a 16:9 one, which its widescreen mode rides on the full
/// screen's own presets.
pub const WIDEST_VIEW: f32 = 32.0 / 9.0;

/// [`vertical_fov`] for a view of any shape (V24): the original's own rule over every shape it ever
/// drew, and the angle along the view's short side held at that range's edge beyond it.
///
/// The disc has a widescreen mode of its own (its 480p widescreen, 720p and 1080i video modes), and
/// that mode is the same rule as the television's: the viewport's shape term is its pixel shape times
/// the display's aspect, 16:9 instead of 4:3, and the projection keeps the preset's horizontal angle.
/// So a wider screen sees no further sideways and less up and down. Between a 2:3 view and a 32:9 one
/// that is exactly what is drawn here. Past them the original has nothing to say, and the port widens
/// rather than crops: a view wider than 32:9 keeps 32:9's vertical angle and grows sideways (Hor+), and
/// one narrower than 2:3 — a portrait window — keeps 2:3's vertical angle and narrows sideways, rather
/// than the vertical sweep opening towards a fish-eye.
pub fn view_fov(horizontal_degrees: f32, aspect: f32) -> f32 {
    vertical_fov(horizontal_degrees, aspect.clamp(NARROWEST_VIEW, WIDEST_VIEW))
}

/// How a scene's lights and fog are applied, pulled out of the `.scn` once.
pub struct Lighting {
    pub sun: V3,
    pub sun_color: [f32; 3],
    pub ambient: [f32; 3],
    pub fog_color: [u8; 3],
    /// Where the original's fog starts and ends ([`Scene::fog_range`]), not the scene's minimum and
    /// maximum.
    pub fog_near: f32,
    pub fog_far: f32,
}

impl Lighting {
    pub fn of(scene: &Scene) -> Lighting {
        let (fog_near, fog_far) = scene.fog_range();
        Lighting {
            sun: scene.sun(),
            sun_color: scene.sun_color(),
            ambient: scene.ambient(),
            fog_color: scene.fog_color,
            fog_near,
            fog_far,
        }
    }
}

/// How far the world is drawn (V32): the original's fog and far clip, or the whole map.
///
/// The original fogs every track over [`Scene::fog_range`] — on every scene but nat08 from
/// `1 - Haziness` of the way to 64 ft short of `MaximumFogDistance`, to `MaximumFogDistance` — and its
/// race viewport's far clip plane is that same end (the fog object hands its end to the viewport's
/// far setter; the human's freeride save state reads near 2.4, far 1000), so nothing past the fog's
/// end is drawn at all. `Full` draws the whole map from anywhere, with a light
/// haze past the original's fog end for depth that never hides more than [`HAZE_MAX`] of a colour.
/// Only the picture changes: nothing here is read by the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Graphics {
    /// The original's look: its fog colour, fog start and end, and its far clip.
    Original,
    /// No fog and no draw distance: the whole map, everywhere.
    #[default]
    Full,
}

/// The most of the fog's colour `Graphics::Full`'s haze mixes into anything, however far.
pub const HAZE_MAX: f32 = 0.3;

/// How a view's fog is applied: the factor is `clamp((d - near) / (far - near), 0, max)` of the fog's
/// colour at `d` feet from the eye, and nothing past `reach` feet is drawn at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub near: f32,
    pub far: f32,
    pub max: f32,
    pub reach: f32,
}

impl Graphics {
    pub const ALL: [Graphics; 2] = [Graphics::Full, Graphics::Original];

    /// `--graphics`' word, and the settings file's.
    pub fn word(self) -> &'static str {
        match self {
            Graphics::Original => "original",
            Graphics::Full => "full",
        }
    }

    pub fn from_word(w: &str) -> Option<Graphics> {
        Graphics::ALL.into_iter().find(|g| g.word() == w)
    }

    /// The settings screen's name for it.
    pub fn label(self) -> &'static str {
        match self {
            Graphics::Original => "Original",
            Graphics::Full => "Whole map",
        }
    }

    /// The fog a view is drawn with, for a map whose farthest two points are `span` feet apart.
    ///
    /// `Original` is the disc's: the scene's fog, and the far clip at its end. `Full` reaches past the
    /// whole map and starts its haze where the original's fog ends, at its most `span` further on.
    pub fn fog(self, light: &Lighting, span: f32) -> Fog {
        match self {
            Graphics::Original => Fog {
                near: light.fog_near,
                far: light.fog_far,
                max: 1.0,
                reach: light.fog_far,
            },
            Graphics::Full => Fog {
                near: light.fog_far,
                far: light.fog_far + span.max(1.0),
                max: HAZE_MAX,
                reach: f32::INFINITY,
            },
        }
    }

    /// Where the projection's far plane goes: the original's far clip, or past anything on a map
    /// `span` feet across, seen from anywhere on it.
    pub fn far_clip(self, light: &Lighting, span: f32) -> f32 {
        match self {
            Graphics::Original => light.fog_far,
            Graphics::Full => (2.0 * span).max(light.fog_far).max(10_000.0),
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
    let mut batch = Batch::new(frame.width, frame.height);
    let drawn = queue_model(&mut batch, loaded, assets, transform, view_projection, eye, light);
    frame.draw(&batch, &texel_of(assets), light.fog_color);
    drawn
}

/// A queued model triangle's texel: its tag is its texture's index in [`Assets::textures`] plus one,
/// and 0 is untextured, flat white. The tag's top two bits are its material's clamp along u and v
/// ([`tag_of`]).
fn texel_of(assets: &Assets) -> impl Fn(u32, f32, f32) -> [u8; 4] + Sync + '_ {
    |tag, u, v| match tag & !(CLAMP_U | CLAMP_V) {
        0 => [255, 255, 255, 255],
        t => {
            // Just short of 1, which the wrap would take back round to the first texel.
            let clamp = |c: f32, on: bool| if on { c.clamp(0.0, 0.999_999) } else { c };
            assets.textures[t as usize - 1].sample(clamp(u, tag & CLAMP_U != 0), clamp(v, tag & CLAMP_V != 0))
        }
    }
}

/// The tag bits that say a triangle's texture is clamped along u and along v.
const CLAMP_U: u32 = 1 << 31;
const CLAMP_V: u32 = 1 << 30;

/// The tag [`texel_of`] reads: the texture's index plus one (0 for none), with the clamp bits.
fn tag_of(texture: Option<usize>, surface: &Surface) -> u32 {
    let Some(i) = texture else { return 0 };
    let mut tag = i as u32 + 1;
    if surface.clamp[0] {
        tag |= CLAMP_U;
    }
    if surface.clamp[1] {
        tag |= CLAMP_V;
    }
    tag
}

/// [`draw_model`]'s triangles, queued into `batch` rather than drawn, in the order it would have
/// drawn them (P47).
fn queue_model(
    batch: &mut Batch,
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
            .filter(|&i| i < assets.textures.len());
        let mut shade = [Shade {
            clip: [0.0; 4],
            uv: [0.0; 2],
            light: [1.0; 3],
            fog: 0.0,
        }; 3];
        let surface = loaded
            .surfaces
            .get(tri.material)
            .copied()
            .unwrap_or_default();
        let mut behind = 0;
        for (k, v) in tri.v.iter().enumerate() {
            let world = transform.point(v.pos);
            let normal = transform.dir(v.normal).norm();
            let lit = vertex_light(surface.shading, light, normal, v.color);
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
        let mode = Mode {
            cutoff: surface.cutoff,
            blend: surface.blend,
            additive: surface.additive,
            depth_write: surface.depth_write,
        };
        batch.triangle(shade, tag_of(texture, &surface), mode);
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
    // Every prop and every extra, in order: what is to be queued into the one batch.
    let mut jobs: Vec<(&Loaded, M4)> = Vec::new();
    for placement in &scene.models {
        let Some(loaded) = assets.models.get(&placement.model) else {
            continue;
        };
        // Cheap cull: a prop whose bounding sphere is beyond the fog is not worth transforming.
        let distance = placement.pos.sub(camera.eye).len() - loaded.radius;
        if distance > light.fog_far {
            continue;
        }
        jobs.push((loaded, placement.transform()));
    }
    for (name, transform) in extra {
        if let Some(loaded) = assets.models.get(*name) {
            jobs.push((loaded, *transform));
        }
    }
    // Queued on every core, a run of models each, and joined back in order: the same batch as one
    // core queueing them all, and in a four-way split the vertex work is most of the scene (P47).
    let queue = |batch: &mut Batch, jobs: &[(&Loaded, M4)]| {
        jobs.iter()
            .map(|&(loaded, at)| queue_model(batch, loaded, assets, at, view_projection, camera.eye, &light))
            .sum::<usize>()
    };
    let mut batch = Batch::new(frame.width, frame.height);
    let threads = crate::raster::threads();
    let drawn = if threads == 1 || jobs.len() < 2 {
        queue(&mut batch, &jobs)
    } else {
        // Runs of about equal triangle counts, so one core does not get every rider.
        let total: usize = jobs.iter().map(|(l, _)| l.tris.len()).sum();
        let share = total.div_ceil(threads).max(1);
        let mut runs: Vec<&[(&Loaded, M4)]> = Vec::new();
        let (mut from, mut held) = (0, 0);
        for (i, (l, _)) in jobs.iter().enumerate() {
            held += l.tris.len();
            if held >= share {
                runs.push(&jobs[from..=i]);
                (from, held) = (i + 1, 0);
            }
        }
        if from < jobs.len() {
            runs.push(&jobs[from..]);
        }
        let (width, height) = (frame.width, frame.height);
        let queued: Vec<(Batch, usize)> = std::thread::scope(|s| {
            let handles: Vec<_> = runs
                .iter()
                .map(|run| {
                    let queue = &queue;
                    s.spawn(move || {
                        let mut part = Batch::new(width, height);
                        let drawn = queue(&mut part, run);
                        (part, drawn)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("a queueing thread panicked")).collect()
        });
        let mut drawn = 0;
        for (part, n) in queued {
            batch.append(part);
            drawn += n;
        }
        drawn
    };
    frame.draw(&batch, &texel_of(assets), light.fog_color);
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::Look;

    /// A 1x1 sixteen-colour `.pst` of one colour, alpha in the GS's units.
    fn one_texel(rgba: [u8; 4]) -> Vec<u8> {
        let mut d = vec![0u8; 92 + 1];
        d[0..4].copy_from_slice(&0x0002_0002u32.to_le_bytes());
        d[4..8].copy_from_slice(&crate::pst::FOUR_BIT.to_le_bytes());
        d[8..12].copy_from_slice(&rgba);
        for (at, v) in [(72, 1u32), (76, 1), (80, 1), (84, 1)] {
            d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        d
    }

    #[test]
    fn an_animated_material_that_came_out_untextured_now_resolves() {
        // `seats_red.psm`: its `TextureMap_1` names no `.pst` at all, only `tex\seats_red.atm`, a
        // header and two frames. Before V25 the stands' seats were drawn plain white.
        let psm = b"[ShaderPass_1]\nmicroProgramName = Vu1StripPrelit.vuo\nTextureMap_1 = seats_red\n\
                    alphaBlendingOnOff = 1\nalphaTestOnOff = 1\nalphaReferenceValue = 80\n\
                    TextureAnimation = unique\n";
        let mut atm = vec![];
        for v in [1u32, 2, 0] {
            atm.extend_from_slice(&v.to_le_bytes());
        }
        atm.extend(one_texel([200, 10, 20, 0x80]));
        atm.extend(one_texel([0, 0, 255, 0x80]));
        let read = |file: &str| match file {
            "seats_red.psm" => Some(psm.to_vec()),
            "seats_red.atm" => Some(atm.clone()),
            _ => None,
        };
        let mut assets = Assets::default();
        let (texture, surface) = assets.resolve(&read, "seats_red");
        let t = &assets.textures[texture.expect("the .atm's first frame")];
        assert_eq!(&t.rgba[0..4], &[200, 10, 20, 255]);
        assert_eq!(surface.shading, Shading::Prelit);
        assert_eq!((surface.cutoff, surface.blend), (160, true));
        // Asked again, the same texture comes back rather than a second copy.
        assert_eq!(assets.resolve(&read, "seats_red").0, texture);
        assert_eq!(assets.textures.len(), 1);
    }

    fn light() -> Lighting {
        Lighting {
            // Straight down, so a normal pointing up takes all of it.
            sun: v3(0.0, -1.0, 0.0),
            sun_color: [0.5, 0.5, 0.5],
            ambient: [0.25, 0.25, 0.25],
            fog_color: [0, 0, 0],
            fog_near: 1.0e6,
            fog_far: 2.0e6,
        }
    }

    #[test]
    fn a_lit_triangle_comes_out_the_colour_worked_by_hand() {
        // A grey texel of 200, on a face tilted 60 degrees from the sun: lambert is cos 60 = 0.5.
        // Diffuse: (0.25 + 0.5 * 0.5) * 64/128 = 0.25, and 200 * 0.25 = 50. Prelit ignores the sun
        // and takes the vertex colour alone: 200 * 64/128 = 100. Constant colour 32: 200 / 4 = 50.
        let normal = v3(0.0, 0.5, 3.0f32.sqrt() / 2.0);
        let color = [64, 64, 64, 128];
        let texel = |_: f32, _: f32| [200u8, 200, 200, 255];
        for (shading, want) in [
            (Shading::Diffuse, 50),
            (Shading::Prelit, 100),
            (Shading::Constant([32.0, 32.0, 32.0, 128.0]), 50),
        ] {
            let lit = vertex_light(shading, &light(), normal, color);
            let mut frame = Frame::new(4, 4, [0, 0, 0]);
            // One triangle over the whole frame, in clip space already.
            let at = |x: f32, y: f32| Shade {
                clip: [x, y, 0.5, 1.0],
                uv: [0.0, 0.0],
                light: lit,
                fog: 0.0,
            };
            frame.triangle_with(
                [at(-3.0, -3.0), at(9.0, -3.0), at(-3.0, 9.0)],
                &Look::cut_out(&texel),
                [0, 0, 0],
            );
            let px = &frame.color[(2 * 4 + 2) * 3..(2 * 4 + 2) * 3 + 3];
            assert_eq!(px, &[want; 3], "{shading:?}");
        }
    }

    #[test]
    fn a_blended_texel_mixes_over_the_frame_by_its_alpha() {
        // Half alpha (the GS's 64, decoded 128): 200 over 100 is 100 + (200 - 100) * 128/255 = 150.
        let texel = |_: f32, _: f32| [200u8, 200, 200, 128];
        let mut frame = Frame::new(2, 2, [100, 100, 100]);
        let at = |x: f32, y: f32| Shade {
            clip: [x, y, 0.5, 1.0],
            uv: [0.0, 0.0],
            light: [1.0; 3],
            fog: 0.0,
        };
        let look = Look {
            paint: &texel,
            cutoff: 1,
            blend: true,
            additive: false,
            depth_write: false,
        };
        frame.triangle_with([at(-3.0, -3.0), at(9.0, -3.0), at(-3.0, 9.0)], &look, [0, 0, 0]);
        assert_eq!(&frame.color[0..3], &[150, 150, 150]);
        assert_eq!(frame.depth[0], 0.0);
    }
}
