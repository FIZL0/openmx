//! The Garage's bike and rider (P213): the seat's pick as the ride draws it — the same chassis, parts
//! and skinned body, in the pick's livery, plate, gear and colours — standing in the page's own
//! corner of the stage and slowly turning, as the original's garage shows the bike being picked.
//!
//! Dressed from the disc again whenever the pick changes, into assets of its own: the menus hold no
//! ride's. The rider sits the bike in the start gate's breathing clip, which is a riding body's own
//! (the original's garage poses its rider with clips of the menus' own we cannot read).

use game_render::disc::Disc;
use game_render::gpu::{Actor, Renderer, Scene3, Target, View};
use game_render::mat::{M4, V3, v3};
use game_render::raster::Frame;
use game_render::rider::{Body, Sex, Skeleton, anchors_of};
use game_render::scene::Vehicle;
use game_render::world::{Assets, Camera, Lighting, draw_model};
use game_render::ini;
use game_sim::vehicle_anim::Blend;
use ps2_float::F;

use crate::{dress, member_ending, menu::Menu, roster};

/// How fast the stand turns, in radians a second: once round in about sixteen seconds, by eye.
const TURN: f32 = 0.4;
/// The body's clip: breathing on the bike at the start gate, and the stopped pose where a rider has
/// none.
const CLIPS: [&str; 2] = ["get_ready_breathe", "turn_stop_Mid"];

/// The dressed pick and how to draw it.
struct Dressed {
    assets: Assets,
    bike: String,
    /// Each part's model and its place on the chassis.
    parts: Vec<(String, M4)>,
    /// The body's model, and its pose for the GPU (node matrices); the software path's is skinned
    /// into the model's triangles.
    body: Option<(String, Vec<M4>)>,
    /// What to aim at, and how far out the camera stands: the bike's middle and its half-size.
    centre: V3,
    radius: f32,
}

/// The stage the Garage shows its seat's pick on.
#[derive(Default)]
pub struct Stage {
    disc: Option<Disc>,
    kit: Option<dress::Kit>,
    /// The pick the stage is dressed in, and what it came to; a pick that failed to dress is kept
    /// too, so it is not tried again every frame.
    pick: Option<roster::Pick>,
    dressed: Option<Dressed>,
    /// Whether the GPU holds this dress's models (it forgets them for a ride's).
    pub uploaded: bool,
    /// The software frame's pixels under the stage, as the menu drew them, and where.
    under: Option<([usize; 4], Vec<u8>)>,
}

impl Stage {
    /// Dress the stage in `pick` if it is not already. Answers whether it changed.
    pub fn dress(&mut self, pick: &roster::Pick) -> bool {
        if self.pick.as_ref() == Some(pick) {
            return false;
        }
        self.pick = Some(pick.clone());
        self.uploaded = false;
        self.dressed = match self.dress_in(pick) {
            Ok(d) => Some(d),
            Err(e) => {
                println!("garage: no model of {} on {}: {e}", pick.rider, pick.bike);
                None
            }
        };
        true
    }

    fn dress_in(&mut self, pick: &roster::Pick) -> Result<Dressed, String> {
        if self.disc.is_none() {
            self.disc = Some(Disc::open_configured()?);
        }
        let disc = self.disc.as_mut().expect("opened above");
        let kit = self.kit.get_or_insert_with(|| dress::Kit::open(disc));
        let mut assets = Assets::default();
        // Its name and number, on the plates and the jersey.
        assets.make_texture(dress::NAME_TEXTURE, kit.texture(disc, &pick.rider, &pick.look, &pick.bike));
        // The bike: its `.sub`'s chassis and parts, in its livery.
        let member = member_ending(disc, &format!("{}.res", pick.bike))?;
        let res = [disc.res(&member)?];
        let vehicle = Vehicle::parse(&ini::text(&res[0].read_suffix(&format!("{}.sub", pick.bike))?));
        let bike = "garage-bike".to_string();
        let file = game_render::vehicle::chassis_model(&res, &vehicle);
        if !assets.load_repainted(&res, &file, &bike, &vehicle.materials) {
            return Err(format!("no {file} in {member}"));
        }
        let chassis = &assets.models[&bike].model;
        let skeleton = Skeleton::of(chassis);
        let anchors = anchors_of(&skeleton, &[]);
        let node = |name: &str| chassis.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name)).map(|i| chassis.world(i));
        let placed: Vec<(String, String, M4)> = vehicle
            .parts
            .iter()
            .map(|part| (part.model.clone(), format!("garage-{}", part.model), node(&part.placement).unwrap_or(M4::IDENTITY)))
            .collect();
        let mut parts = Vec::new();
        for (model, name, at) in placed {
            if assets.load_repainted(&res, &model, &name, &vehicle.materials) {
                parts.push((name, at));
            }
        }
        let (min, max) = assets.models[&bike]
            .tris
            .iter()
            .flat_map(|t| t.v.iter())
            .fold((v3(f32::MAX, f32::MAX, f32::MAX), v3(f32::MIN, f32::MIN, f32::MIN)), |(lo, hi), v| {
                (v3(lo.x.min(v.pos.x), lo.y.min(v.pos.y), lo.z.min(v.pos.z)), v3(hi.x.max(v.pos.x), hi.y.max(v.pos.y), hi.z.max(v.pos.z)))
            });
        // The rider in its gear and the pick's colours, posed on the bike.
        let body = match member_ending(disc, &format!("{}.res", pick.rider)) {
            Ok(rider) => {
                let mut res = vec![disc.res(&rider)?];
                let mut body = Body::load(vec![disc.res(&rider)?], &pick.rider, Sex::Male, "", &[], game_sim::motion::Params::default())?;
                if let Some(custom) = &kit.custom {
                    res.push(disc.res(custom)?);
                }
                let name = "garage-rider".to_string();
                let pairs = kit.repaint(&body.rider.suit, &pick.look);
                if assets.load_repainted(&res, body.geometry(), &name, &pairs) {
                    let clip = CLIPS.iter().find_map(|c| body.rig.set.id(c));
                    let blend = Blend::One { phase: F(0.0), ramp: F(0.0), clip };
                    body.rig.set.apply_vehicle(blend, &game_sim::motion::Player::default());
                    body.posed = true;
                    let (pose, _) = body.palette(Some(&anchors));
                    if let Some(loaded) = assets.models.get_mut(&name) {
                        loaded.tris = game_render::skin::skin(&loaded.model, &pose, 0);
                    }
                    Some((name, pose))
                } else {
                    None
                }
            }
            Err(e) => {
                println!("garage: no rider to show: {e}");
                None
            }
        };
        let mut centre = min.add(max).scale(0.5);
        let mut radius = max.sub(min).len() / 2.0;
        if body.is_some() {
            // The rider stands above the seat: aim a little higher and stand back for them.
            centre.y += 0.12 * radius;
            radius *= 1.1;
        }
        Ok(Dressed { assets, bike, parts, body, centre, radius })
    }

    /// The models to draw at `turn` radians round, each with its place and whether the body's pose
    /// is its own.
    fn actors(&self, turn: f32) -> Vec<(&str, M4, bool)> {
        let Some(d) = &self.dressed else { return Vec::new() };
        // Turned about the bike's middle, which then stands at the origin.
        let spin = M4::placement(v3(0.0, 0.0, 0.0), v3(turn.sin(), 0.0, turn.cos()), v3(0.0, 1.0, 0.0));
        let at = M4::placement(d.centre.scale(-1.0), v3(0.0, 0.0, 1.0), v3(0.0, 1.0, 0.0)).mul(spin);
        let mut out = vec![(d.bike.as_str(), at, false)];
        out.extend(d.parts.iter().map(|(name, node)| (name.as_str(), node.mul(at), false)));
        if let Some((name, _)) = &d.body {
            out.push((name.as_str(), at, true));
        }
        out
    }

    /// The camera on the stage, `w` by `h` pixels: from the bike's side and a little above and in
    /// front, as the original's showroom looks at it.
    fn camera(&self) -> Option<Camera> {
        let d = self.dressed.as_ref()?;
        let fov = 0.5f32;
        let back = d.radius / (fov / 2.0).tan() * 1.05;
        let eye = v3(1.0, 0.28, 0.35).norm().scale(back);
        Some(Camera { eye, at: v3(0.0, 0.0, 0.0), fov, near: 0.1, far: back * 4.0 })
    }

    /// How far round the stand is `tick` menu frames in.
    pub fn turn(tick: u32) -> f32 {
        // the start of a turn faces the bike's right side to the camera, nose to the left
        tick as f32 / 60.0 * TURN - 0.6
    }

    /// The light the stage is drawn in: a sun from high over the camera's shoulder, no fog.
    fn light() -> Lighting {
        Lighting {
            sun: v3(-0.5, -0.8, -0.35).norm(),
            sun_color: [0.85, 0.85, 0.85],
            ambient: [0.45, 0.45, 0.45],
            fog_color: [0, 0, 0],
            fog_near: 1.0e6,
            fog_far: 2.0e6,
        }
    }

    /// Draw the stage over the menu the GPU just drew, into `rect` of the frame.
    pub fn draw_gpu(&mut self, r: &mut Renderer, t: &Target, rect: [u32; 4], turn: f32) {
        let (Some(d), Some(camera)) = (&self.dressed, self.camera()) else { return };
        if !self.uploaded {
            // The menus' sprites are kept; a ride's models and textures are not this stage's.
            r.forget_assets();
            let names: Vec<&String> = d.assets.models.keys().collect();
            r.upload_models(&d.assets, &names);
            self.uploaded = true;
        }
        let mut poses = Vec::new();
        let actors: Vec<Actor> = self
            .actors(turn)
            .into_iter()
            .map(|(model, at, posed)| {
                let pose = match (posed, &d.body) {
                    (true, Some((_, p))) => {
                        poses.push(p.clone());
                        Some(poses.len() - 1)
                    }
                    _ => None,
                };
                Actor { model, at, pose, lod_from: None, alpha: 1.0 }
            })
            .collect();
        let light = Stage::light();
        let views = [View { rect, camera: &camera, actors: &actors }];
        r.render_over(t, &d.assets, &Scene3 { views: &views, poses: &poses, overlay: &[], sprites: &[], light: &light });
    }

    /// Draw the stage into `rect` of a software frame the menu is in. `fresh` says the menu was just
    /// drawn under it; otherwise what it drew there is put back first. Answers whether it drew.
    pub fn draw_soft(&mut self, frame: &mut Frame, rect: [usize; 4], turn: f32, fresh: bool) -> bool {
        let [x, y, w, h] = clip(rect, frame.width, frame.height);
        if w == 0 || h == 0 || self.dressed.is_none() {
            return false;
        }
        let row = |j: usize| (y + j) * frame.width * 3 + x * 3;
        match &self.under {
            Some((at, pixels)) if !fresh && *at == [x, y, w, h] => {
                for j in 0..h {
                    frame.color[row(j)..row(j) + w * 3].copy_from_slice(&pixels[j * w * 3..(j + 1) * w * 3]);
                }
            }
            _ => {
                let mut pixels = Vec::with_capacity(w * h * 3);
                for j in 0..h {
                    pixels.extend_from_slice(&frame.color[row(j)..row(j) + w * 3]);
                }
                self.under = Some(([x, y, w, h], pixels));
            }
        }
        let camera = self.camera().expect("dressed");
        let d = self.dressed.as_ref().expect("dressed");
        let mut stage = Frame::new(w, h, [0, 0, 0]);
        let vp = camera.view_projection(w, h);
        let light = Stage::light();
        for (model, at, _) in self.actors(turn) {
            if let Some(loaded) = d.assets.models.get(model) {
                draw_model(&mut stage, loaded, &d.assets, at, vp, camera.eye, &light);
            }
        }
        for j in 0..h {
            for i in 0..w {
                if stage.depth[j * w + i] > 0.0 {
                    let (from, to) = ((j * w + i) * 3, row(j) + i * 3);
                    frame.color[to..to + 3].copy_from_slice(&stage.color[from..from + 3]);
                }
            }
        }
        true
    }
}

/// `rect` cut to a `width` by `height` frame.
fn clip([x, y, w, h]: [usize; 4], width: usize, height: usize) -> [usize; 4] {
    let (x, y) = (x.min(width), y.min(height));
    [x, y, w.min(width - x), h.min(height - y)]
}

/// The stage's rect in a `size` frame's pixels, while the menu shows the Garage.
pub fn rect(menu: &Menu, size: (usize, usize)) -> Option<[usize; 4]> {
    let [x, y, w, h] = menu.garage_stage(size)?;
    Some([x.max(0.0) as usize, y.max(0.0) as usize, w.max(0.0) as usize, h.max(0.0) as usize])
}

/// The pick the Garage shows: the seat its Player row is on.
pub fn shown_pick(menu: &Menu) -> Option<&roster::Pick> {
    menu.garage_pick()
}
