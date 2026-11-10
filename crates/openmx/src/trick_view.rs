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
//!
//! Each view's camera (P319) stands as near as the trick's whole motion lets it, fitted to the
//! bike's, its parts' and the rider's points over the loop, and orbits them at the Garage's pace.
//! The view's right stick, or a drag across the stage, takes the orbit over — round, and up and down
//! within limits — and it holds there; [`HOLD`] seconds after the last turn it eases back into its
//! slow orbit from where it was left.

use game_render::chassis::grips;
use game_render::gpu::{Actor, Renderer, Scene3, Target, View};
use game_render::mat::{M4, V3, v3};
use game_render::raster::Frame;
use game_render::anim::PackClips;
use game_render::disc::Disc;
use game_render::psg::Model;
use game_render::rider::{Anchors, Body, Sex};
use game_render::scene::Part;
use game_render::vehicle::{self, Chassis, Drive};
use game_render::world::{Assets, Camera, Lighting, Loaded, draw_model};
use game_sim::motion::Player;
use game_sim::trick::{Set, Stunt};
use game_sim::vehicle_anim::{self as va, Blend};
use ps2_float::F;

use crate::clock::Instant;
use crate::garage_view::{FOV, TURN, bounds_of, fit_distance, points_of};
use crate::menu::Menu;
use crate::menu::trick_list::{Show, TrickRow};
use crate::split;
use crate::window::Mouse;
use game_sim::input::{AXIS_CENTRE, Report};

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
/// Where the orbit starts, round and up from level, in radians: the Garage's view, from the bike's
/// right side, a little in front and above. Yaw 0 looks from in front of the nose and grows towards
/// the camera's right.
const YAW_REST: f32 = 1.2341;
const PITCH_REST: f32 = 0.2585;
/// How far up and down the player can take the camera, in radians from level.
const PITCH: [f32; 2] = [-0.35, 1.1];
/// How long the orbit holds after the player's last turn, and how long it then eases back into its
/// pace over, in seconds.
pub const HOLD: f32 = 5.0;
const EASE: f32 = 1.5;
/// How fast the right stick at full lean turns the camera, in radians a second, and how far it
/// leans before it counts, of its whole reach.
const STICK_RATE: f32 = 2.4;
const DEAD: f32 = 0.2;
/// How far a drag across the whole stage turns the camera, in radians.
const DRAG_TURN: f32 = std::f32::consts::PI;
/// How quickly the camera's aim and distance settle on a newly lit trick's, per second.
const SETTLE: f32 = 8.0;
/// How many steps through a trick's loop its fit poses it at, and how many of each model's points
/// it reads at each.
const SWEEP: usize = 12;
const SWEEP_POINTS: usize = 500;
/// How many steps round the fit looks from, and the steps of pitch it refits at.
const FIT_YAWS: usize = 32;
const PITCH_STEP: f32 = 0.05;

/// The camera round a paused view's stage (P319): orbiting slowly by itself, or where the player's
/// right stick or drag left it, until it has been left alone [`HOLD`] seconds.
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    /// Round the stage and up from level, in radians.
    pub yaw: f32,
    pub pitch: f32,
    /// When the player last turned it, and when it last stepped.
    touched: Option<Instant>,
    last: Option<Instant>,
    /// What it looks at and how far out it stands, settling on the lit trick's.
    aim: V3,
    back: f32,
    /// The stick's lean, right and down, and a drag's turn, round and up, for the next step.
    stick: [f32; 2],
    drag: [f32; 2],
    /// When it began, and the whole second and the state it last said itself in.
    born: Instant,
    said: Option<(u64, &'static str)>,
}

impl Orbit {
    fn new(now: Instant) -> Orbit {
        Orbit {
            yaw: YAW_REST,
            pitch: PITCH_REST,
            touched: None,
            last: None,
            aim: v3(0.0, 0.0, 0.0),
            back: 0.0,
            stick: [0.0; 2],
            drag: [0.0; 2],
            born: now,
            said: None,
        }
    }

    /// How much of its own pace the orbit turns at `now`: none while held, easing up to all of it.
    fn pace(&self, now: Instant) -> f32 {
        let Some(at) = self.touched else { return 1.0 };
        let s = ((now.saturating_duration_since(at).as_secs_f32() - HOLD) / EASE).clamp(0.0, 1.0);
        s * s * (3.0 - 2.0 * s)
    }

    /// Step to `now`, settling on `aim` and `back`: the player's turns since the last step, or the
    /// orbit's own. The stick turns the way it leans (right goes round to the camera's right, up
    /// tips the camera up to look from lower), a drag the way the stage under it is pulled.
    fn step(&mut self, now: Instant, aim: V3, back: f32) {
        let dt = self.last.map_or(0.0, |l| now.saturating_duration_since(l).as_secs_f32()).min(0.25);
        self.last = Some(now);
        let lean = |v: f32| if v.abs() < DEAD { 0.0 } else { v.signum() * (v.abs() - DEAD) / (1.0 - DEAD) };
        let turn = [lean(self.stick[0]) * STICK_RATE * dt + self.drag[0], lean(self.stick[1]) * STICK_RATE * dt + self.drag[1]];
        self.drag = [0.0; 2];
        if turn != [0.0, 0.0] || (lean(self.stick[0]), lean(self.stick[1])) != (0.0, 0.0) {
            self.touched = Some(now);
            self.yaw += turn[0];
            self.pitch = (self.pitch + turn[1]).clamp(PITCH[0], PITCH[1]);
        } else {
            self.yaw += TURN * self.pace(now) * dt;
        }
        self.yaw = self.yaw.rem_euclid(std::f32::consts::TAU);
        let k = if self.back == 0.0 { 1.0 } else { 1.0 - (-dt * SETTLE).exp() };
        self.aim = self.aim.add(aim.sub(self.aim).scale(k));
        self.back += (back - self.back) * k;
    }

    /// The unit step from what it looks at towards the eye.
    fn dir(&self) -> V3 {
        dir_of(self.yaw, self.pitch)
    }

    fn camera(&self, reach: f32) -> Camera {
        let eye = self.aim.add(self.dir().scale(self.back));
        Camera { eye, at: self.aim, fov: FOV, near: 0.1, far: self.back + reach * 4.0 }
    }

    /// A line for `seat`'s camera once a second, and whenever it starts or stops turning by itself.
    fn say(&mut self, seat: usize, now: Instant) {
        let pace = self.pace(now);
        let state = match self.touched {
            None => "orbiting",
            Some(_) if pace >= 1.0 => "orbiting",
            Some(_) if pace > 0.0 => "easing back",
            Some(_) => "held",
        };
        let second = now.saturating_duration_since(self.born).as_secs();
        if self.said.is_none_or(|(s, was)| s != second || was != state) {
            self.said = Some((second, state));
            println!(
                "tricks camera: {}P yaw {:.1} pitch {:.1} {state}, {:.1} s in",
                seat + 1,
                self.yaw.to_degrees(),
                self.pitch.to_degrees(),
                now.saturating_duration_since(self.born).as_secs_f32()
            );
        }
    }
}

/// The unit step from a stage's middle towards an eye `yaw` round and `pitch` up.
fn dir_of(yaw: f32, pitch: f32) -> V3 {
    v3(pitch.cos() * yaw.sin(), pitch.sin(), pitch.cos() * yaw.cos())
}

/// A pad's right stick, right and down, each -1 to 1: none from a pad that is not there.
pub fn lean_of(r: &Report) -> [f32; 2] {
    if r.status() == 0xff {
        return [0.0; 2];
    }
    let axis = |i: usize| ((r.axis(i) as f32 - AXIS_CENTRE as f32) / 128.0).clamp(-1.0, 1.0);
    [axis(0), axis(1)]
}

/// What a trick's fit reads (P319): its models' points, the bike's and the rider's to be skinned
/// in each pose.
#[derive(Default)]
struct Shapes {
    bike: Option<Model>,
    /// The bike at rest, where it has no pose; each part's at rest and the node it rides on.
    bike_rest: Vec<V3>,
    parts: Vec<(String, Vec<V3>)>,
    body: Option<Model>,
}

/// A trick's whole loop as the camera frames it: the middle of all it reaches, the points from
/// there, and the distances fitted to them by stage shape and pitch.
struct Swept {
    aim: V3,
    reach: f32,
    points: Vec<V3>,
    fits: Vec<((u32, i32), f32)>,
}

impl Swept {
    /// How far out a camera at `pitch` stands on a stage `aspect` wide for each unit high, all the
    /// way round.
    fn back(&mut self, aspect: f32, pitch: f32) -> f32 {
        let step = (pitch / PITCH_STEP).round() as i32;
        let key = (aspect.to_bits(), step);
        if let Some(&(_, b)) = self.fits.iter().find(|(k, _)| *k == key) {
            return b;
        }
        let pitch = step as f32 * PITCH_STEP;
        let dirs: Vec<V3> = (0..FIT_YAWS).map(|k| dir_of(std::f32::consts::TAU * k as f32 / FIT_YAWS as f32, pitch)).collect();
        let b = fit_distance(&self.points, v3(0.0, 0.0, 0.0), &dirs, FOV, aspect).max(self.reach * 0.5);
        if self.fits.len() > 64 {
            self.fits.clear();
        }
        self.fits.push((key, b));
        b
    }
}

/// The page's rider and bike.
pub struct TrickView {
    body: Option<Body>,
    chassis: Option<Chassis>,
    /// What the whole bike turns about: the bike's middle, a little above for the rider.
    centre: V3,
    /// The tricks on show, one a paused view, and when each one's loop started.
    lit: Vec<(TrickRow, Instant)>,
    /// The software path's copies of the bike and the body, skinned into each frame.
    soft: Option<(Option<Loaded>, Option<Loaded>)>,
    /// What the cameras are fitted to, and each trick's fit, once it has been lit (P319).
    shapes: Shapes,
    swept: Vec<(TrickRow, Swept)>,
    /// Each paused view's camera, by its seat, and the drag under way: its seat, where it last was
    /// in the frame, and its stage's width.
    orbits: Vec<(usize, Orbit)>,
    dragging: Option<(usize, [f32; 2], f32)>,
}

/// One frame of the stage: where the bike stands, the chassis' and the body's node matrices, and
/// the view's camera on them.
pub struct Posed {
    pub at: M4,
    pub chassis: Vec<M4>,
    pub body: Option<Vec<M4>>,
    pub camera: Camera,
}

/// A paused view showing the page: its seat, its stage in the frame's pixels, and its lit trick.
pub type Shown = (usize, [usize; 4], TrickRow);

/// The ride's models the stage draws: the bike's, its parts and the body's.
pub struct Models<'a> {
    pub bike: &'a str,
    pub parts: &'a [Part],
    pub body: Option<&'a str>,
}

impl TrickView {
    /// The stage for a body and a chassis loaded as the ride's, framed on `models` in `assets`.
    pub fn new(body: Option<Body>, chassis: Option<Chassis>, assets: &Assets, models: &Models) -> TrickView {
        let bike = models.bike;
        let bounds = assets.models.get(bike).and_then(|loaded| {
            let mut points = loaded.tris.iter().flat_map(|t| t.v.iter()).map(|v| v.pos);
            let first = points.next()?;
            Some(points.fold((first, first), |(lo, hi), p| {
                (v3(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)), v3(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)))
            }))
        });
        let (min, max) = bounds.unwrap_or((v3(-0.6, 0.0, -2.4), v3(0.6, 3.0, 2.4)));
        let mut centre = min.add(max).scale(0.5);
        if body.is_some() {
            // The rider stands above the seat, and a flip swings them round the middle.
            centre.y += 0.06 * max.sub(min).len();
        }
        let model = |name: &str| assets.models.get(name);
        let mut shapes = Shapes { bike: model(bike).map(|l| l.model.clone()), body: models.body.and_then(model).map(|l| l.model.clone()), ..Shapes::default() };
        if let Some(l) = model(bike) {
            points_of(&l.tris, M4::IDENTITY, SWEEP_POINTS, &mut shapes.bike_rest);
        }
        for part in models.parts {
            let mut rest = Vec::new();
            if let Some(l) = model(&part.model) {
                points_of(&l.tris, M4::IDENTITY, SWEEP_POINTS / 8, &mut rest);
            }
            shapes.parts.push((part.placement.clone(), rest));
        }
        TrickView { body, chassis, centre, lit: Vec::new(), soft: None, shapes, swept: Vec::new(), orbits: Vec::new(), dragging: None }
    }

    /// The right stick's lean on `seat`'s camera ([`lean_of`]), for its next step.
    pub fn stick(&mut self, seat: usize, lean: [f32; 2]) {
        if let Some((_, o)) = self.orbits.iter_mut().find(|(s, _)| *s == seat) {
            o.stick = lean;
        }
    }

    /// The mouse's (or a finger's) presses and moves in the frame's pixels since the last frame: a
    /// drag that starts on a view's stage turns its camera.
    pub fn mouse(&mut self, events: &[(Mouse, [f32; 2])], shown: &[Shown]) {
        for &(what, p) in events {
            match what {
                Mouse::Down => {
                    let on = |r: &[usize; 4]| p[0] >= r[0] as f32 && p[1] >= r[1] as f32 && p[0] < (r[0] + r[2]) as f32 && p[1] < (r[1] + r[3]) as f32;
                    self.dragging = shown.iter().find(|(_, r, _)| on(r)).map(|(seat, r, _)| (*seat, p, r[2].max(1) as f32));
                }
                Mouse::Move | Mouse::Up => {
                    if let Some((seat, last, width)) = self.dragging.as_mut() {
                        let k = DRAG_TURN / *width;
                        let turn = [-(p[0] - last[0]) * k, (p[1] - last[1]) * k];
                        *last = p;
                        if let Some((_, o)) = self.orbits.iter_mut().find(|(s, _)| s == seat) {
                            o.drag = [o.drag[0] + turn[0], o.drag[1] + turn[1]];
                        }
                    }
                    if what == Mouse::Up {
                        self.dragging = None;
                    }
                }
            }
        }
    }

    /// Pose the stage for each of `shown`'s views at `now`, in order, each with its camera: a trick's
    /// loop starts when it is lit.
    pub fn poses(&mut self, shown: &[Shown], now: Instant) -> Vec<([usize; 4], Posed)> {
        self.lit.retain(|(lit, _)| shown.iter().any(|(_, _, row)| row == lit));
        self.orbits.retain(|(seat, _)| shown.iter().any(|(s, _, _)| s == seat));
        let mut out = Vec::new();
        for (seat, rect, row) in shown {
            let mut posed = self.pose(row, now);
            if !self.swept.iter().any(|(r, _)| r == row) {
                let swept = self.sweep(row);
                self.swept.push((row.clone(), swept));
                // and the pose again, which the sweep moved
                posed = self.pose(row, now);
            }
            let swept = &mut self.swept.iter_mut().find(|(r, _)| r == row).expect("swept above").1;
            let o = match self.orbits.iter().position(|(s, _)| s == seat) {
                Some(i) => &mut self.orbits[i].1,
                None => {
                    self.orbits.push((*seat, Orbit::new(now)));
                    &mut self.orbits.last_mut().expect("pushed").1
                }
            };
            let back = swept.back(rect[2].max(1) as f32 / rect[3].max(1) as f32, o.pitch);
            o.step(now, swept.aim, back);
            o.say(*seat, now);
            posed.camera = o.camera(swept.reach);
            out.push((*rect, posed));
        }
        out
    }

    /// `row`'s whole loop as the camera frames it: the bike, its parts and the rider posed at
    /// [`SWEEP`] steps through it, and every point they reach (P319).
    fn sweep(&mut self, row: &TrickRow) -> Swept {
        let mut points = Vec::new();
        for k in 0..=SWEEP {
            let posed = self.pose_at(row, k as f32 / SWEEP as f32);
            match (&self.shapes.bike, posed.chassis.is_empty()) {
                (Some(m), false) => points_of(&game_render::skin::skin(m, &posed.chassis, 0), posed.at, SWEEP_POINTS, &mut points),
                _ => points.extend(self.shapes.bike_rest.iter().map(|p| posed.at.point(*p))),
            }
            for (placement, rest) in &self.shapes.parts {
                let node = self.chassis.as_ref().and_then(|c| c.node(placement));
                let at = node.map_or(posed.at, |n| n.mul(posed.at));
                points.extend(rest.iter().map(|p| at.point(*p)));
            }
            if let (Some(m), Some(pose)) = (&self.shapes.body, &posed.body) {
                points_of(&game_render::skin::skin(m, pose, 0), posed.at, SWEEP_POINTS, &mut points);
            }
        }
        if points.is_empty() {
            points.push(v3(0.0, 0.0, 0.0));
        }
        let (min, max) = bounds_of(&points);
        let aim = min.add(max).scale(0.5);
        let reach = max.sub(min).len() / 2.0;
        TrickView::swept_of(aim, reach, points)
    }

    fn swept_of(aim: V3, reach: f32, points: Vec<V3>) -> Swept {
        Swept { aim, reach, points: points.into_iter().map(|p| p.sub(aim)).collect(), fits: Vec::new() }
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
        let t = now.saturating_duration_since(start).as_secs_f32() % (length + REST);
        self.pose_at(row, (t / length).min(1.0))
    }

    /// Pose the stage for `row`, `progress` of the way through it.
    fn pose_at(&mut self, row: &TrickRow, progress: f32) -> Posed {
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
        // the view's camera is put on by `poses`
        let camera = Camera { eye: dir_of(YAW_REST, PITCH_REST).scale(10.0), at: v3(0.0, 0.0, 0.0), fov: FOV, near: 0.1, far: 100.0 };
        Posed { at, chassis, body, camera }
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
        let camera = posed.camera;
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
        let camera = posed.camera;
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
    models: &Models,
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
        _ => Some(TrickView::new(body, chassis, assets, models)),
    }
}

/// Each paused view showing the Tricks page, in `rects`' views of `players`: its seat, its stage in
/// the frame's pixels, and the trick lit on it.
pub fn shown(menu: &mut Menu, rects: &[split::Rect], players: usize) -> Vec<Shown> {
    let mut out = Vec::new();
    for (v, at) in menu.pause_views(rects, players) {
        let lit = menu.with_seat(v, |m| Some((m.trick_lit()?.clone(), m.trick_stage((at.w, at.h))?)));
        if let Some((row, [x, y, w, h])) = lit {
            let px = |f: f32| f.max(0.0) as usize;
            out.push((v, [at.x + px(x), at.y + px(y), px(w), px(h)], row));
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

    /// P319: left alone the camera goes round at the Garage's pace; the stick turns it and holds it
    /// there; HOLD seconds after the stick is let go it starts again from where it was, easing up to
    /// its pace with no jump, and the pitch stays inside its limits.
    #[test]
    fn the_orbit_turns_holds_for_the_stick_and_eases_back() {
        let t0 = Instant::now();
        let at = |s: f32| t0 + std::time::Duration::from_secs_f32(s);
        let tick = 1.0 / 30.0;
        let mut o = Orbit::new(t0);
        let (aim, back) = (v3(0.0, 1.0, 0.0), 4.0);
        let mut t = 0.0;
        o.step(at(t), aim, back);
        let first = o.yaw;
        while t < 2.0 - 1e-4 {
            t += tick;
            o.step(at(t), aim, back);
        }
        assert!(((o.yaw - first) - TURN * 2.0).abs() < 1e-3, "idle: {} in 2 s", o.yaw - first);
        assert_eq!(o.pace(at(t)), 1.0);
        // the stick right and up for a second
        let (yaw0, pitch0) = (o.yaw, o.pitch);
        for _ in 0..30 {
            o.stick = [1.0, -1.0];
            t += tick;
            o.step(at(t), aim, back);
        }
        assert!(o.yaw - yaw0 > STICK_RATE * 0.9, "the stick turned it round: {}", o.yaw - yaw0);
        assert!(o.pitch < pitch0 && o.pitch >= PITCH[0], "the stick tipped it up: {} -> {}", pitch0, o.pitch);
        o.stick = [0.0; 2];
        let let_go = t;
        let held = o.yaw;
        let mut last = o.yaw;
        let mut most = 0.0f32;
        while t < let_go + HOLD + EASE + 1.0 {
            t += tick;
            o.step(at(t), aim, back);
            let moved = o.yaw - last;
            if t <= let_go + HOLD {
                assert_eq!(o.yaw, held, "held at {:.2} s after the stick", t - let_go);
            }
            assert!(moved >= 0.0 && moved <= TURN * tick + 1e-5, "no jump: {moved} at {:.2} s", t - let_go);
            assert!(moved >= most - 1e-6, "the pace only grows: {moved} after {most}");
            most = most.max(moved);
            last = o.yaw;
        }
        assert!((most - TURN * tick).abs() < 1e-4, "back to its pace: {most}");
        // and it stops at its limits
        for _ in 0..200 {
            o.stick = [0.0, 1.0];
            t += tick;
            o.step(at(t), aim, back);
        }
        assert_eq!(o.pitch, PITCH[1]);
        o.drag = [0.0, -10.0];
        t += tick;
        o.step(at(t), aim, back);
        assert_eq!(o.pitch, PITCH[0]);
    }

    /// The right stick reads off axes 0 and 1, and a pad that is not there leans nowhere.
    #[test]
    fn the_right_stick_leans_off_axes_0_and_1() {
        let mut r = Report::default();
        r.0[0] = 0;
        r.0[4] = 0xff;
        r.0[5] = 0x00;
        r.0[6] = 0x00;
        r.0[7] = 0x00;
        let [x, y] = lean_of(&r);
        assert!(x > 0.99 && y < -0.99, "{x} {y}");
        r.0[0] = 0xff;
        assert_eq!(lean_of(&r), [0.0; 2]);
    }

    /// The fit leaves every point of the motion inside the frame from each direction it was fitted
    /// for, at any stage shape, and touches the frame's edge with one of them.
    #[test]
    fn a_fit_keeps_every_point_in_the_frame() {
        let mut points = Vec::new();
        for i in 0..400 {
            let a = i as f32 * 0.37;
            points.push(v3(a.sin() * 1.3, (a * 1.7).cos() * 0.9 + 0.4, (a * 0.6).sin() * 0.7));
        }
        let aim = v3(0.0, 0.3, 0.0);
        for aspect in [0.6, 1.0, 28.0 / 18.5, 2.4] {
            for pitch in [PITCH[0], 0.0, PITCH_REST, PITCH[1]] {
                let dirs: Vec<V3> = (0..FIT_YAWS).map(|k| dir_of(std::f32::consts::TAU * k as f32 / FIT_YAWS as f32, pitch)).collect();
                let d = fit_distance(&points, aim, &dirs, FOV, aspect);
                let ty = (FOV * 0.5).tan();
                let mut widest = 0.0f32;
                for e in &dirs {
                    let r = v3(e.z, 0.0, -e.x).norm();
                    let u = e.cross(r);
                    for p in &points {
                        let q = p.sub(aim);
                        let depth = d - q.dot(*e);
                        assert!(depth > 0.0);
                        let (sx, sy) = (q.dot(r).abs() / depth / (ty * aspect), q.dot(u).abs() / depth / ty);
                        assert!(sx <= 1.0 && sy <= 1.0, "aspect {aspect} pitch {pitch}: {sx} {sy}");
                        widest = widest.max(sx).max(sy);
                    }
                }
                assert!(widest > 0.9, "aspect {aspect} pitch {pitch}: it fills the frame ({widest})");
            }
        }
    }
}
