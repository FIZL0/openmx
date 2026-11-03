//! The pause's Tricks page's rider (P259): the ride's own body on the ride's own bike, in their skin,
//! performing the lit trick on a loop in the page's stage ([`crate::menu::Menu::trick_stage`]).
//!
//! A body and a chassis of the page's own, loaded off the ride's archives the first time the page is
//! up, so the riders' own rigs and their clocks are never touched: posing these moves nothing the
//! ride or a replay reads. Drawn with the ride's own models — on the GPU by the ride's renderer over
//! the menu ([`Renderer::render_over`]), in software into copies of them skinned each frame.
//!
//! The clip is the trick's own where the rider has one; the bike plays its own where it has one too
//! and otherwise holds its rest shape, hands kept on the bars as the ride keeps them (MD41). The moves
//! that are the bike's and not a clip — the flips, a 360, the whip, a wheelie — turn the whole bike by
//! the port's own, illustrative amounts.

use game_render::chassis::grips;
use game_render::gpu::{Actor, Renderer, Scene3, Target, View};
use game_render::mat::{M4, V3, v3};
use game_render::raster::Frame;
use game_render::anim::PackClips;
use game_render::disc::Disc;
use game_render::rider::{Anchors, Body, Sex};
use game_render::scene::Part;
use game_render::vehicle::{self, Chassis, Drive};
use game_render::world::{Assets, Camera, Lighting, Loaded, draw_model};
use game_sim::motion::Player;
use game_sim::trick::{Set, Stunt};
use game_sim::vehicle_anim::{self as va, Blend};
use ps2_float::F;

use crate::clock::Instant;
use crate::menu::Menu;
use crate::menu::trick_list::{Show, TrickRow};
use crate::split;

/// How long the loop rests between two goes at a trick, in seconds.
const REST: f32 = 0.6;
/// The airborne pose's middle, `air_Mid`, which the whip and a trick with no clip ride in; a front
/// flip rides in its forward end, `air_Fwd` ([`va::AIR`]).
const AIR_MID: i32 = va::AIR + 1;
/// `turn_riding_Bck` and `turn_riding_Fwd`: leaning back for a wheelie, forward for a stoppie.
const LEAN_BACK: i32 = va::TURN_RIDING + 2;
const LEAN_FORWARD: i32 = va::TURN_RIDING;
/// How far a wheelie lifts the nose and a stoppie the tail, and how far the whip lays the bike over,
/// in radians: by eye.
const WHEEL_TILT: f32 = 0.45;
const WHIP_LEAN: f32 = 1.2;

/// The page's rider and bike.
pub struct TrickView {
    body: Option<Body>,
    chassis: Option<Chassis>,
    /// What to aim at, and how far out the camera stands: the bike's middle and its half-size.
    centre: V3,
    radius: f32,
    /// The tricks on show, one a paused view, and when each one's loop started.
    lit: Vec<(TrickRow, Instant)>,
    /// The software path's copies of the bike and the body, skinned into each frame.
    soft: Option<(Option<Loaded>, Option<Loaded>)>,
}

/// One frame of the stage: where the bike stands, and the chassis' and the body's node matrices.
pub struct Posed {
    pub at: M4,
    pub chassis: Vec<M4>,
    pub body: Option<Vec<M4>>,
}

/// The ride's models the stage draws: the bike's, its parts and the body's.
pub struct Models<'a> {
    pub bike: &'a str,
    pub parts: &'a [Part],
    pub body: Option<&'a str>,
}

impl TrickView {
    /// The stage for a body and a chassis loaded as the ride's, framed on `bike`'s model in `assets`.
    pub fn new(body: Option<Body>, chassis: Option<Chassis>, assets: &Assets, bike: &str) -> TrickView {
        let bounds = assets.models.get(bike).and_then(|loaded| {
            let mut points = loaded.tris.iter().flat_map(|t| t.v.iter()).map(|v| v.pos);
            let first = points.next()?;
            Some(points.fold((first, first), |(lo, hi), p| {
                (v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)), v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)))
            }))
        });
        let (min, max) = bounds.unwrap_or((v3(-0.6, 0.0, -2.4), v3(0.6, 3.0, 2.4)));
        let mut centre = min.add(max).scale(0.5);
        let mut radius = max.sub(min).len() / 2.0;
        if body.is_some() {
            // The rider stands above the seat, and a flip swings them round the middle.
            centre.y += 0.12 * radius;
            radius *= 1.25;
        }
        TrickView { body, chassis, centre, radius, lit: Vec::new(), soft: None }
    }

    /// Pose the stage for each of `rows` at `now`, in order: a trick's loop starts when it is lit.
    pub fn poses(&mut self, rows: &[&TrickRow], now: Instant) -> Vec<Posed> {
        self.lit.retain(|(lit, _)| rows.contains(&lit));
        rows.iter().map(|row| self.pose(row, now)).collect()
    }

    /// Pose the stage for `row` at `now`.
    fn pose(&mut self, row: &TrickRow, now: Instant) -> Posed {
        let start = match self.lit.iter().find(|(lit, _)| lit == row) {
            Some((_, at)) => *at,
            None => {
                self.lit.push((row.clone(), now));
                now
            }
        };
        let length = row.length.max(0.1);
        let t = now.duration_since(start).as_secs_f32() % (length + REST);
        let progress = (t / length).min(1.0);
        let state = state_of(row.show);
        let mut bare = false;
        if let Some(chassis) = &mut self.chassis {
            let clip = chassis.stance.lib.clip(state).filter(|_| !chassis.lacks(state));
            if clip.is_some() {
                play(&mut chassis.stance.set, clip, progress);
            }
            chassis.posing = clip.is_some();
            chassis.fading = 0.0;
            bare = clip.is_none();
            let rear_mount = chassis.node_rest("TireRear").unwrap_or_default();
            let on_ground = matches!(row.show, Show::Wheelie | Show::Stoppie);
            chassis.drive(1.0 / 60.0, Drive { rear_mount, on_ground, ..Drive::default() });
        }
        let body = self.body.as_mut().map(|body| {
            let clip = body.pose_lib.clip(state).or_else(|| body.pose_lib.clip(AIR_MID));
            play(&mut body.rig.set, clip, progress);
            body.posed = true;
            body.hold = 0.0;
            body.bare = bare;
            let held: Anchors = self.chassis.as_ref().map(grips).unwrap_or_default();
            body.palette(Some(&held)).0
        });
        let chassis = self.chassis.as_ref().map_or(Vec::new(), |c| c.world.clone());
        let at = M4::placement(self.centre.scale(-1.0), v3(0.0, 0.0, 1.0), v3(0.0, 1.0, 0.0)).mul(motion(row.show, progress));
        Posed { at, chassis, body }
    }

    /// The camera on the stage: from the bike's side, a little above and in front, as the Garage's.
    fn camera(&self) -> Camera {
        let fov = 0.5f32;
        let back = self.radius / (fov / 2.0).tan() * 1.05;
        let eye = v3(1.0, 0.28, 0.35).norm().scale(back);
        Camera { eye, at: v3(0.0, 0.0, 0.0), fov, near: 0.1, far: back * 4.0 }
    }

    /// The models to draw and where, each with the pose it takes, if any: 0 the chassis', 1 the body's.
    fn actors<'a>(&self, models: &Models<'a>, posed: &Posed) -> Vec<(&'a str, M4, Option<usize>)> {
        let mut out = vec![(models.bike, posed.at, (!posed.chassis.is_empty()).then_some(0))];
        for part in models.parts {
            let node = self.chassis.as_ref().and_then(|c| c.node(&part.placement));
            out.push((part.model.as_str(), node.map_or(posed.at, |n| n.mul(posed.at)), None));
        }
        if let (Some(body), Some(_)) = (models.body, &posed.body) {
            out.push((body, posed.at, Some(1)));
        }
        out
    }

    /// Draw the stage into `rect` of what the GPU just drew, with the ride's own models.
    pub fn draw_gpu(&self, r: &mut Renderer, t: &Target, assets: &Assets, rect: [u32; 4], models: &Models, posed: &Posed) {
        let camera = self.camera();
        let poses = [posed.chassis.clone(), posed.body.clone().unwrap_or_default()];
        let actors: Vec<Actor> = self
            .actors(models, posed)
            .into_iter()
            .map(|(model, at, pose)| Actor { model, at, pose, lod_from: None, alpha: 1.0, casts: false })
            .collect();
        let light = light();
        let views = [View { rect, camera: &camera, actors: &actors }];
        r.render_over(t, assets, &Scene3 { views: &views, poses: &poses, overlay: &[], sprites: &[], light: &light });
    }

    /// [`TrickView::draw_gpu`] for each paused view's stage.
    pub fn draw_gpu_each(&self, r: &mut Renderer, t: &Target, assets: &Assets, models: &Models, posed: &[([usize; 4], Posed)]) {
        for (rect, p) in posed {
            self.draw_gpu(r, t, assets, rect.map(|v| v as u32), models, p);
        }
    }

    /// Draw the stage into `rect` of a software frame, over what is there: the bike and the body
    /// skinned into copies of the ride's models, the parts as they are.
    pub fn draw_soft(&mut self, frame: &mut Frame, assets: &Assets, rect: [usize; 4], models: &Models, posed: &Posed) {
        let [x, y, w, h] = rect;
        let (x, y) = (x.min(frame.width), y.min(frame.height));
        let (w, h) = (w.min(frame.width - x), h.min(frame.height - y));
        if w == 0 || h == 0 {
            return;
        }
        let soft = self.soft.get_or_insert_with(|| {
            (assets.models.get(models.bike).cloned(), models.body.and_then(|b| assets.models.get(b)).cloned())
        });
        if let (Some(bike), false) = (&mut soft.0, posed.chassis.is_empty()) {
            bike.tris = game_render::skin::skin(&bike.model, &posed.chassis, 0);
        }
        if let (Some(body), Some(pose)) = (&mut soft.1, &posed.body) {
            body.tris = game_render::skin::skin(&body.model, pose, 0);
        }
        let camera = self.camera();
        let vp = camera.view_projection(w, h);
        let light = light();
        let mut stage = Frame::new(w, h, [0, 0, 0]);
        let soft = self.soft.as_ref().expect("made above");
        for (model, at, pose) in self.actors(models, posed) {
            let loaded = match pose {
                Some(0) => soft.0.as_ref(),
                Some(_) => soft.1.as_ref(),
                None => assets.models.get(model),
            };
            if let Some(loaded) = loaded {
                draw_model(&mut stage, loaded, assets, at, vp, camera.eye, &light);
            }
        }
        for j in 0..h {
            for i in 0..w {
                if stage.depth[j * w + i] > 0.0 {
                    let (from, to) = ((j * w + i) * 3, ((y + j) * frame.width + x + i) * 3);
                    frame.color[to..to + 3].copy_from_slice(&stage.color[from..from + 3]);
                }
            }
        }
    }
}

/// Load the stage's rider and bike off the disc as the ride loads its own (`rider` its archive and
/// name, `bike` its archive and `.mcf`), each with the trick packs' clips, framed on the ride's
/// `bike_model`. `None` when neither loads.
#[allow(clippy::too_many_arguments)]
pub fn load(
    disc: &mut Disc,
    rider: Option<(&str, &str)>,
    bike: Option<(&str, &str)>,
    prefix: &str,
    table: &[Stunt],
    motion: game_sim::motion::Params,
    pack: Option<&PackClips>,
    stunts: &Set,
    assets: &Assets,
    bike_model: &str,
) -> Option<TrickView> {
    let body = rider.and_then(|(member, name)| {
        let mut body = Body::load(vec![disc.res(member).ok()?], name, Sex::Male, "", table, motion).ok()?;
        if let Some(p) = pack {
            body.add_pack(p, &stunts.stunts);
        }
        Some(body)
    });
    let chassis = bike.filter(|(_, mcf)| !mcf.is_empty()).and_then(|(member, mcf)| {
        let mut chassis = Chassis::load(vec![disc.res(member).ok()?], mcf, prefix, table, vehicle::params()).ok()?;
        if let Some(p) = pack {
            chassis.add_pack(p, &stunts.stunts);
        }
        Some(chassis)
    });
    match (&body, &chassis) {
        (None, None) => {
            println!("tricks page: no rider or bike to show");
            None
        }
        _ => Some(TrickView::new(body, chassis, assets, bike_model)),
    }
}

/// Each paused view showing the Tricks page, in `rects`' views of `players`: its stage in the
/// frame's pixels, and the trick lit on it.
pub fn shown(menu: &mut Menu, rects: &[split::Rect], players: usize) -> Vec<([usize; 4], TrickRow)> {
    let mut out = Vec::new();
    for (v, at) in menu.pause_views(rects, players) {
        let lit = menu.with_seat(v, |m| Some((m.trick_lit()?.clone(), m.trick_stage((at.w, at.h))?)));
        if let Some((row, [x, y, w, h])) = lit {
            let px = |f: f32| f.max(0.0) as usize;
            out.push(([at.x + px(x), at.y + px(y), px(w), px(h)], row));
        }
    }
    out
}

/// The animation state a trick's clip is read under.
fn state_of(show: Show) -> i32 {
    match show {
        Show::Clip(state) => state,
        Show::Backflip => va::BACKFLIP,
        Show::Spin { right: false } => va::SPIN_LEFT,
        Show::Spin { right: true } => va::SPIN_RIGHT,
        // leant full forward on the airborne row, as the ride's is (P159)
        Show::FrontFlip => va::AIR,
        Show::Whip => AIR_MID,
        Show::Wheelie => LEAN_BACK,
        Show::Stoppie => LEAN_FORWARD,
    }
}

/// Put `clip` of `set` on, `progress` of the way through it.
fn play(set: &mut game_render::anim::Set, clip: Option<game_sim::motion::ClipId>, progress: f32) {
    let time = F(set.timing(clip).duration.0 * progress);
    let player = Player { time, prev: time, ..Player::default() };
    set.apply_vehicle(Blend::One { phase: F(progress), ramp: F(0.0), clip }, &player);
}

/// How the whole bike is turned `progress` of the way through a trick, about its middle.
fn motion(show: Show, progress: f32) -> M4 {
    let turn = std::f32::consts::TAU * progress;
    let rise = (std::f32::consts::PI * progress).sin();
    // nose up by `a`
    let pitch = |a: f32| M4::placement(v3(0.0, 0.0, 0.0), v3(0.0, a.sin(), a.cos()), v3(0.0, a.cos(), -a.sin()));
    match show {
        Show::Backflip => pitch(turn),
        Show::FrontFlip => pitch(-turn),
        Show::Spin { right } => {
            let a = if right { -turn } else { turn };
            M4::placement(v3(0.0, 0.0, 0.0), v3(a.sin(), 0.0, a.cos()), v3(0.0, 1.0, 0.0))
        }
        Show::Whip => {
            let a = WHIP_LEAN * rise;
            M4::placement(v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 1.0), v3(a.sin(), a.cos(), 0.0))
        }
        Show::Wheelie => pitch(WHEEL_TILT * rise),
        Show::Stoppie => pitch(-WHEEL_TILT * rise),
        Show::Clip(_) => M4::IDENTITY,
    }
}

/// The light the stage is drawn in: the Garage's, a sun from high over the camera's shoulder, no fog.
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A flip comes back round to where it started and is upside down halfway; the clip-only tricks
    /// leave the bike as it stands.
    #[test]
    fn flips_go_once_round() {
        let up = |m: M4| m.dir(v3(0.0, 1.0, 0.0));
        for show in [Show::Backflip, Show::FrontFlip] {
            assert!((up(motion(show, 0.5)).y + 1.0).abs() < 1e-4, "{show:?}");
            assert!((up(motion(show, 1.0)).y - 1.0).abs() < 1e-4, "{show:?}");
        }
        let nose = |m: M4| m.dir(v3(0.0, 0.0, 1.0));
        assert!(nose(motion(Show::Backflip, 0.25)).y > 0.99, "a backflip lifts the nose first");
        assert!(nose(motion(Show::FrontFlip, 0.25)).y < -0.99);
        assert!(nose(motion(Show::Wheelie, 0.5)).y > 0.4 && nose(motion(Show::Stoppie, 0.5)).y < -0.4);
        assert_eq!(motion(Show::Clip(3), 0.5).0, M4::IDENTITY.0);
        assert_eq!(state_of(Show::Clip(7)), 7);
        assert_eq!(va::POSE_CLIPS[(AIR_MID - va::TURN_STOP) as usize], "air_Mid");
        assert_eq!(va::POSE_CLIPS[(LEAN_BACK - va::TURN_STOP) as usize], "turn_riding_Bck");
        assert_eq!(va::POSE_CLIPS[(LEAN_FORWARD - va::TURN_STOP) as usize], "turn_riding_Fwd");
    }
}
