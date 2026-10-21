//! `.scn` — a track's scene, and `.psm` — a material. Both are the text format in [`crate::ini`].
//!
//! A scene names where the race starts, which terrain to load, the fog, two lights, and then every
//! prop on the track: a supercross stadium is about fifteen hundred `[ModelN]` sections, each one a
//! model name and three vectors. That list is most of what a spectator sees — the stands, the banners,
//! the bales, the finish gantry — so it is the world the first draw puts the bike in.
//!
//! A material is shorter: the VU1 microprogram to run (`Vu1StripDiffuse.vuo` for nearly everything),
//! the texture, and the filter settings. Only the texture name is used here.
//!
//! A bike is a third one, `.sub`: the parts bolted onto the chassis model and the node each one hangs
//! from, plus the materials this particular bike paints the shared chassis with — a 250 SX is the CR
//! 250's geometry with its own plastics, engine and wheels, so without those renames half its
//! textures are not on the disc under the names its meshes ask for.
//!
//! A fourth, `cpresets.ini`, is the follow camera: eleven presets, of which the first is the one a
//! race starts on. It is loaded with the track rather than with the game, so a track could in
//! principle give itself a camera of its own.

use crate::ini;
use crate::mat::{M4, V3, v3};

/// One light from `[LightN]`. The disc's tracks carry two: an ambient fill and the sun.
#[derive(Clone, Copy, Debug)]
pub struct Light {
    pub directional: bool,
    pub color: [f32; 3],
    /// Where the sun is; with `target`, this gives the direction it shines in.
    pub position: V3,
    pub target: V3,
}

/// What the original's fog object subtracts from the fog's end before `Haziness` scales it, in feet:
/// a constant its constructor and its split-screen reset both pass, not anything in the scene.
const FOG_START_OFFSET: f32 = 64.0;

/// Where the original's fog starts and ends, from a scene's `[Fog]` numbers.
///
/// The fog object ends the fog `Visibility` of the way from `MinimumFogDistance` to
/// `MaximumFogDistance`, and starts it `1 - Haziness` of the way from the eye to 64 ft short of that
/// end. The renderer is handed exactly this pair: nothing nearer than the start is fogged, the fog
/// grows linearly to fully opaque at the end, and the race view's far clip plane is the end too. So
/// with `Visibility` 1 (every scene but nat08) `MinimumFogDistance` plays no part at all: free09's
/// 10..1000 at `Haziness` 0.6 is fog from 374.4 to 1000.
///
/// A split screen outside supercross passes `0.61 x MaximumFogDistance` as `max`.
pub fn fog_range(min: f32, max: f32, visibility: f32, haziness: f32) -> (f32, f32) {
    // The object's clamp: anything not above 0 (NaN too) is 0, anything from 1 up is 1.
    let unit = |x: f32| if x > 0.0 { x.min(1.0) } else { 0.0 };
    let end = min + unit(visibility) * (max - min);
    let start = (1.0 - unit(haziness)) * (end - FOG_START_OFFSET);
    (start, end)
}

/// One prop: a model name and the transform the scene places it with.
#[derive(Clone, Debug)]
pub struct Placement {
    /// The model's name without a directory or extension, as the scene spells it.
    pub model: String,
    pub pos: V3,
    pub look: V3,
    pub up: V3,
}

/// The model a `SLT=` line names, as the original's text reader hands it on: that reader drops every
/// space, tab and line break outside brackets and quotes before it splits a line at `=`, so free06's
/// `Free06_Power Line_A` (its high-tension wires) and free09's `Large Egg` load `free06_powerline_a.psg`
/// and `largeegg.psg`. Kept with the spaces, both were names no file on the disc has, and neither drew.
pub fn slt_name(value: &str) -> String {
    value.chars().filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n')).collect()
}

impl Placement {
    /// Model space to world space.
    pub fn transform(&self) -> M4 {
        M4::placement(self.pos, self.look, self.up)
    }
}

/// A parsed `.scn`.
#[derive(Clone, Debug)]
pub struct Scene {
    pub name: String,
    /// Where the game puts the rider at the start, and which way it faces.
    pub start: V3,
    pub start_dir: V3,
    /// The terrain file this track draws its ground from.
    pub terrain: String,
    pub fog_color: [u8; 3],
    /// `[Fog]`'s `MinimumFogDistance` and `MaximumFogDistance` as the file has them. They are not
    /// where the fog starts and ends: [`Scene::fog_range`] is.
    pub fog_near: f32,
    pub fog_far: f32,
    /// `[Fog]`'s `Visibility` and `Haziness`, fractions the original clamps to 0..1.
    pub fog_visibility: f32,
    pub fog_haziness: f32,
    pub lights: Vec<Light>,
    pub models: Vec<Placement>,
}

impl Scene {
    pub fn parse(text: &str) -> Scene {
        let sections = ini::parse(text);
        let find = |name: &str| sections.iter().find(|s| s.name.eq_ignore_ascii_case(name));
        let vec_of = |s: Option<&ini::Section>, key: &str, fallback: V3| {
            s.and_then(|s| s.numbers(key))
                .filter(|n| n.len() >= 3)
                .map(|n| v3(n[0], n[1], n[2]))
                .unwrap_or(fallback)
        };
        let info = find("SceneInfo");
        let env = find("Environment");
        let fog = find("Fog");
        // The fog's numbers as the original reads them: its `strtod` and then its double-to-float
        // conversion, which truncates, so `Haziness=0.60` is the float just below 0.6.
        let fog_number = |key: &str| fog.and_then(|s| s.get(key)).map(|v| game_sim::phy::number(v).0);
        let rgb = fog
            .and_then(|s| s.numbers("ColorRGB"))
            .filter(|n| n.len() >= 3)
            .map(|n| [n[0] as u8, n[1] as u8, n[2] as u8])
            .unwrap_or([120, 130, 150]);

        let mut lights = Vec::new();
        let mut models = Vec::new();
        for s in &sections {
            let lower = s.name.to_ascii_lowercase();
            if lower.starts_with("light") && s.get("Type").is_some() {
                let color = s
                    .numbers("ColorRGB")
                    .filter(|n| n.len() >= 3)
                    .map(|n| [n[0] / 255.0, n[1] / 255.0, n[2] / 255.0])
                    .unwrap_or([1.0, 1.0, 1.0]);
                lights.push(Light {
                    directional: s
                        .get("Type")
                        .is_some_and(|t| t.eq_ignore_ascii_case("Directional")),
                    color,
                    position: vec_of(Some(s), "Position", v3(0.0, 1.0, 0.0)),
                    target: vec_of(Some(s), "TargetPosition", v3(0.0, 0.0, 0.0)),
                });
            } else if lower.starts_with("model") {
                if let Some(name) = s.get("SLT") {
                    models.push(Placement {
                        model: slt_name(name),
                        pos: vec_of(Some(s), "Position", v3(0.0, 0.0, 0.0)),
                        look: vec_of(Some(s), "LookVector", v3(0.0, 0.0, 1.0)),
                        up: vec_of(Some(s), "UpVector", v3(0.0, 1.0, 0.0)),
                    });
                }
            }
        }

        Scene {
            name: info
                .and_then(|s| s.get("SceneName"))
                .unwrap_or("")
                .to_string(),
            start: vec_of(info, "DefaultStartPosition", v3(0.0, 0.0, 0.0)),
            start_dir: vec_of(info, "DefaultStartDirection", v3(0.0, 0.0, 1.0)),
            terrain: env
                .and_then(|s| s.get("TerrainFile"))
                .unwrap_or("")
                .to_string(),
            fog_color: rgb,
            fog_near: fog_number("MinimumFogDistance").unwrap_or(5.0),
            fog_far: fog_number("MaximumFogDistance").unwrap_or(2000.0),
            // The original's reader defaults these two to 1 and 0.5; every scene on the disc has both.
            fog_visibility: fog_number("Visibility").unwrap_or(1.0),
            fog_haziness: fog_number("Haziness").unwrap_or(0.5),
            lights,
            models,
        }
    }

    /// Where the original's fog starts and ends, in feet from the eye: [`fog_range`] of this scene's
    /// `[Fog]`.
    pub fn fog_range(&self) -> (f32, f32) {
        fog_range(self.fog_near, self.fog_far, self.fog_visibility, self.fog_haziness)
    }

    /// The direction the sun shines in, or a default from above and behind if the scene has no
    /// directional light.
    pub fn sun(&self) -> V3 {
        for l in &self.lights {
            if l.directional {
                return l.target.sub(l.position).norm();
            }
        }
        v3(0.3, -0.9, 0.3).norm()
    }

    /// The ambient colour, or a dim grey.
    pub fn ambient(&self) -> [f32; 3] {
        for l in &self.lights {
            if !l.directional {
                return l.color;
            }
        }
        [0.3, 0.3, 0.3]
    }

    /// The directional light's colour, or white.
    pub fn sun_color(&self) -> [f32; 3] {
        for l in &self.lights {
            if l.directional {
                return l.color;
            }
        }
        [1.0, 1.0, 1.0]
    }
}

/// A parsed `.psm`: what a mesh is painted with. Only the first shader pass is read; the passes
/// after it are specular maps and the like, drawn over the first.
#[derive(Clone, Debug, Default)]
pub struct Material {
    /// The `.pst` this material samples, without its extension.
    pub texture: Option<String>,
    /// The VU1 microprogram the original runs for it, which is what decides how it is lit.
    pub microprogram: Option<String>,
    /// `alphaTestOnOff`, and `alphaReferenceValue` in the GS's units (128 = opaque) where it has one.
    pub alpha_test: Option<Option<f32>>,
    /// `processingMethodWhenFailedInAlphaTest` of 1: a texel that fails the test still lands in the
    /// colour buffer, only the depth buffer skips it.
    pub fail_keeps_color: bool,
    /// `alphaBlendingOnOff`, with `inputColorA`, `inputColorB` and `inputColorD` of the GS's
    /// `(A - B) * C + D` (0 source, 1 frame, 2 zero); a missing one reads as the usual 0, 1, 1.
    pub blend: Option<[u8; 3]>,
    /// `zWriteDisabled`.
    pub no_depth_write: bool,
    /// `ShaderRGBA`, for the constant-colour microprograms, 128 meaning "unchanged".
    pub rgba: Option<[f32; 4]>,
    /// `TextureAnimation`: `unique` for the material's own `.atm`, or the name of another.
    pub animation: Option<String>,
    /// `wrapModeHorizontal` and `wrapModeVertical` of 1: the texture is clamped at its edges along
    /// that axis instead of repeated (the GS's CLAMP against its REPEAT, 0, which is also what a
    /// material that says nothing gets). Palm fronds, bark and the cut-out cowboys clamp.
    pub clamp: [bool; 2],
}

/// How a material's vertices are lit, by the family of microprogram it names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shading {
    /// `Vu1StripDiffuse*`, and anything not recognised: the scene's sun and ambient on the normals,
    /// times the vertex colour.
    Diffuse,
    /// `Vu1StripPrelit*`: the vertex colours are the light, baked; the sun is not applied.
    Prelit,
    /// `Vu1StripConstantColor*`: one colour, `ShaderRGBA`, over the whole mesh.
    Constant([f32; 4]),
}

impl Material {
    pub fn parse(text: &str) -> Material {
        let sections = ini::parse(text);
        let pass = sections
            .iter()
            .find(|s| s.name.to_ascii_lowercase().starts_with("shaderpass"));
        let get = |k: &str| {
            pass.and_then(|s| s.get(k))
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
        };
        let flag = |k: &str| get(k).and_then(|v| v.parse::<f32>().ok()).map(|v| v != 0.0);
        let number = |k: &str| get(k).and_then(|v| v.parse::<f32>().ok());
        let input = |k: &str, usual: u8| number(k).map(|v| v as u8).unwrap_or(usual);
        Material {
            texture: get("TextureMap_1").map(|t| t.to_string()),
            microprogram: get("microProgramName").map(|t| t.to_string()),
            alpha_test: flag("alphaTestOnOff")
                .filter(|&on| on)
                .map(|_| number("alphaReferenceValue")),
            fail_keeps_color: number("processingMethodWhenFailedInAlphaTest") == Some(1.0),
            blend: flag("alphaBlendingOnOff").filter(|&on| on).map(|_| {
                [input("inputColorA", 0), input("inputColorB", 1), input("inputColorD", 1)]
            }),
            no_depth_write: flag("zWriteDisabled") == Some(true),
            rgba: pass
                .and_then(|s| s.numbers("ShaderRGBA"))
                .filter(|v| v.len() >= 4)
                .map(|v| [v[0], v[1], v[2], v[3]]),
            animation: get("TextureAnimation").map(|t| t.to_string()),
            clamp: [number("wrapModeHorizontal") == Some(1.0), number("wrapModeVertical") == Some(1.0)],
        }
    }

    /// How the vertices are lit, from the microprogram's name.
    pub fn shading(&self) -> Shading {
        let program = self.microprogram.as_deref().unwrap_or("").to_ascii_lowercase();
        if program.contains("constantcolor") {
            Shading::Constant(self.rgba.unwrap_or([128.0; 4]))
        } else if program.contains("prelit") && !program.contains("diffuse") {
            Shading::Prelit
        } else {
            Shading::Diffuse
        }
    }
}

/// One follow camera from `cpresets.ini`. The file carries eleven, plus a copy of each for every
/// split-screen layout (`Preset1h`, `Preset1v`, …); only the single-player ones are read here.
#[derive(Clone, Debug)]
pub struct Preset {
    pub name: String,
    /// Degrees around the bike, 180 being straight behind it.
    pub angle_y: f32,
    /// Degrees away from straight up, so 65 puts the camera 25 degrees above the rider.
    pub angle_x: f32,
    /// How far behind, and how far ahead of the bike the camera looks, in the disc's feet.
    pub distance: f32,
    pub look_ahead: f32,
    /// Horizontal field of view, in degrees.
    pub fov: f32,
}

impl Preset {
    /// Every `[PresetN]` in a `cpresets.ini`, in file order.
    pub fn parse(text: &str) -> Vec<Preset> {
        let mut out = Vec::new();
        for s in ini::parse(text) {
            // `Preset1` is the single-player camera; `Preset1h` and friends are the same camera for
            // a split screen, and a frame here is never split.
            let is_single = s
                .name
                .strip_prefix("Preset")
                .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()));
            if !is_single {
                continue;
            }
            out.push(Preset {
                name: s.get("Name").unwrap_or("").trim_matches('"').to_string(),
                angle_y: s.number("AngleY").unwrap_or(180.0),
                angle_x: s.number("AngleX").unwrap_or(65.0),
                distance: s.number("FollowingDistance").unwrap_or(13.0),
                look_ahead: s.number("LookAheadDistance").unwrap_or(20.0),
                fov: s.number("FOV").unwrap_or(85.0),
            });
        }
        out
    }
}

/// One part of a vehicle: a model, and the node of the chassis it is placed at.
#[derive(Clone, Debug)]
pub struct Part {
    pub model: String,
    /// The name of a node of the chassis model; `Bike` is the chassis's own root.
    pub placement: String,
}

/// A parsed `.sub`: how one bike is built out of the shared chassis and its own parts.
#[derive(Clone, Debug, Default)]
pub struct Vehicle {
    pub parts: Vec<Part>,
    /// `[Objects] MCF`: the animation set the chassis is posed from — `bike.mcf` on every `.sub` on
    /// this disc. The chassis model itself comes from that set's own `SLTFile`.
    pub mcf: String,
    /// `from` -> `to`, as `[Material Changes]` lists them.
    pub materials: Vec<(String, String)>,
}

impl Vehicle {
    pub fn parse(text: &str) -> Vehicle {
        let sections = ini::parse(text);
        let mut parts = Vec::new();
        let mut materials = Vec::new();
        let mut mcf = String::new();
        for section in &sections {
            let lower = section.name.to_ascii_lowercase();
            if lower.starts_with("subobject") {
                if let Some(model) = section.get("SLT") {
                    parts.push(Part {
                        model: model.to_string(),
                        placement: section.get("Placement").unwrap_or("Bike").to_string(),
                    });
                }
            } else if lower == "objects" {
                mcf = section.get("MCF").unwrap_or("").to_string();
            } else if lower == "material changes" {
                // Here the keys are the names the meshes use and the values are what to load
                // instead, so the section is read as pairs rather than looked up by name.
                for (from, to) in &section.keys {
                    materials.push((from.clone(), to.clone()));
                }
            }
        }
        Vehicle { parts, mcf, materials }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCN: &str = "[SceneInfo]\nSceneName=Supercross01\nDefaultStartPosition=101.49,90.12,458.48\n\
         DefaultStartDirection=0.00,0.00,1.00\n\n[Environment]\nTerrainFile=SX01.ps2.trn\n\n\
         [Fog]\nColorRGB=114,117,123\nMinimumFogDistance=5.00\nMaximumFogDistance=2000.00\n\n\
         [Lights]\nNumberOfLights = 2\n\n[Light1]\nType=Ambient\nColorRGB=109,120,135\n\n\
         [Light2]\n;  The Sun\nType=Directional\nColorRGB=180,178,174\n\
         Position=-457.71,767.67,-368.00\nTargetPosition=384.0,384.0,90.0\n\n\
         [StaticModels]\nNumberOfStaticModels=2\n\n[Model1]\nSLT=bale_single_02\n\
         Position=627.03,94.29,651.93\nLookVector=-0.76,-0.40,-0.51\nUpVector=-0.51,0.86,0.09\n\n\
         [Model2]\nSLT=bale_single_08\nPosition=274.51,92.74,114.97\n";

    #[test]
    fn a_scene_gives_a_start_a_sun_and_its_props() {
        let s = Scene::parse(SCN);
        assert_eq!(s.name, "Supercross01");
        assert_eq!(s.start, v3(101.49, 90.12, 458.48));
        assert_eq!(s.terrain, "SX01.ps2.trn");
        assert_eq!(s.fog_color, [114, 117, 123]);
        assert_eq!(s.lights.len(), 2);
        assert_eq!(s.models.len(), 2);
        assert_eq!(s.models[0].model, "bale_single_02");
        // The sun points from its position towards what it aims at, and so downwards.
        assert!(s.sun().y < 0.0, "{:?}", s.sun());
        // A prop with no vectors of its own still gets a usable transform.
        assert_eq!(s.models[1].transform().point(v3(0.0, 0.0, 0.0)), s.models[1].pos);
    }

    /// free09's `[Fog]` (10..1000, `Visibility` 1, `Haziness` 0.6) against what the save state of
    /// `recording2.p2m2` holds in its renderer's fog pair, bit for bit; and nat08, the one scene whose
    /// `Visibility` is under 1, where the minimum matters.
    #[test]
    fn the_fog_starts_where_the_haziness_puts_it() {
        let s = Scene::parse(
            "[Fog]\nColorRGB=150,151,139\nVisibility=1.00\nHaziness=0.60\n\
             MinimumFogDistance=10.00\nMaximumFogDistance=1000.00\n",
        );
        let (start, end) = s.fog_range();
        assert_eq!(s.fog_haziness.to_bits(), 0x3f19_9999, "0.6 truncated, as the original reads it");
        assert_eq!((start.to_bits(), end.to_bits()), (0x43bb_3334, 0x447a_0000), "{start} {end}");
        let (start, end) = fog_range(300.0, 450.0, 0.5, 0.8);
        assert_eq!(end, 375.0);
        assert!((start - 0.2 * (375.0 - 64.0)).abs() < 1e-3, "{start}");
        // Out-of-range fractions clamp, and missing ones take the reader's defaults.
        assert_eq!(fog_range(10.0, 1000.0, 2.0, -1.0), (936.0, 1000.0));
        assert_eq!(fog_range(10.0, 1000.0, f32::NAN, f32::NAN), (-54.0, 10.0));
        assert_eq!(Scene::parse(SCN).fog_range(), (0.5 * (2000.0 - 64.0), 2000.0));
    }

    #[test]
    fn a_material_names_its_texture() {
        let m = Material::parse(
            "[General]\nTotalShaderPasses = 1\n\n[ShaderPass_1]  ;---\n\
             microProgramName = Vu1StripDiffuse.vuo\nTextureMap_1 = bales_01_03\nMipMap_1 = 2\n",
        );
        assert_eq!(m.texture.as_deref(), Some("bales_01_03"));
        assert_eq!(m.microprogram.as_deref(), Some("Vu1StripDiffuse.vuo"));
        assert_eq!(m.shading(), Shading::Diffuse);
        assert_eq!((m.alpha_test, m.blend, m.no_depth_write), (None, None, false));
    }

    #[test]
    fn a_material_says_how_it_is_lit_and_laid_over_the_frame() {
        // `seats_red.psm` from a supercross stadium, cut down.
        let m = Material::parse(
            "[General]\n[ShaderPass_1]  ;---\nmicroProgramName = Vu1StripPrelit.vuo\n\
             TextureMap_1 = seats_red\nalphaBlendingOnOff = 1\nalphaTestOnOff = 1\n\
             alphaTestMethod = 5\nalphaReferenceValue = 80\n\
             processingMethodWhenFailedInAlphaTest = 1\nzWriteDisabled = 0\n\
             TextureAnimation = unique\n[ShaderPass_1_Animator_1]\nanimatedMapFramesPerSecond = 15\n",
        );
        assert_eq!(m.shading(), Shading::Prelit);
        assert_eq!(m.alpha_test, Some(Some(80.0)));
        assert!(m.fail_keeps_color);
        assert_eq!(m.blend, Some([0, 1, 1]));
        assert!(!m.no_depth_write);
        assert_eq!(m.animation.as_deref(), Some("unique"));
        // An empty value is no value: `alphaTestOnOff =` is not "on".
        let m = Material::parse(
            "[ShaderPass_1]\nmicroProgramName = Vu1StripConstantColor.vuo\nalphaTestOnOff =\n\
             ShaderRGBA = 64.0, 128.0, 128.0, 128.0\n",
        );
        assert_eq!(m.alpha_test, None);
        assert_eq!(m.shading(), Shading::Constant([64.0, 128.0, 128.0, 128.0]));
    }

    #[test]
    fn a_material_says_whether_its_texture_repeats_or_clamps() {
        // `palmfraunb.psm` clamps along v only; most materials say nothing and repeat both ways.
        let m = Material::parse(
            "[ShaderPass_1]\nTextureMap_1 = palmfraunb\nwrapModeHorizontal = 0\nwrapModeVertical = 1\n",
        );
        assert_eq!(m.clamp, [false, true]);
        assert_eq!(Material::parse("[ShaderPass_1]\nTextureMap_1 = x\n").clamp, [false; 2]);
    }

    #[test]
    fn the_follow_camera_presets_skip_the_split_screen_copies() {
        let p = Preset::parse(
            "[NumberOfPresets]\nNumberOfPresets = 11\n\n[Preset1]\nName = \"Race Cam 1\"\n\
             AngleY = 180.0\nAngleX = 65.0\nFollowingDistance = 13.0\nFOV = 85.0\n\
             LookAheadDistance = 20.0\n\n[Preset2]\nName = \"Race Cam 2\"\nFollowingDistance = 9.0\n\n\
             [Preset1h]\nName = \"Race Cam 1 split\"\nFollowingDistance = 7.0\n",
        );
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "Race Cam 1");
        assert_eq!(p[0].distance, 13.0);
        assert_eq!(p[0].fov, 85.0);
        // The second preset gives no angles, so it falls back to the first camera's.
        assert_eq!(p[1].distance, 9.0);
        assert_eq!(p[1].angle_x, 65.0);
    }
}
