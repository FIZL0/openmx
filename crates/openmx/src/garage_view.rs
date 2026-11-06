//! The Garage's bike and rider (P213): the seat's pick as the ride draws it — the same chassis, parts
//! and skinned body, in the pick's livery, plate, gear and colours — standing in the page's own
//! corner of the stage and slowly turning, as the original's garage shows the bike being picked.
//!
//! Dressed from the disc again whenever the pick changes, into assets of its own: the menus hold no
//! ride's. The rider sits the bike in the start gate's breathing clip, which is a riding body's own
//! (the original's garage poses its rider with clips of the menus' own we cannot read).
//!
//! The dressing runs off the menu thread (P258), neighbours ahead: the stage keeps the last dressed
//! pick on show until the asked one is ready, and never shows a pick no longer asked.

use game_render::disc::Disc;
use game_render::gpu::{Actor, Mips, Renderer, Scene3, Target, View};
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
    /// Each texture's levels as the GPU takes them, by texture and cut-out, made on the dress's
    /// worker: made at the upload, they were most of its frame (P258).
    mips: Vec<(usize, u8, Mips)>,
}

/// The disc and the wardrobe a dress reads, opened by the first: here while no dress is under way,
/// with the dress on its worker while one is.
#[derive(Default)]
struct Tools {
    disc: Option<Disc>,
    kit: Option<dress::Kit>,
}

/// A dress under way (P258): its pick, and the worker that hands back the tools and what it made.
type Job = (roster::Pick, crate::thread::JoinHandle<(Tools, Option<Dressed>)>);

/// How many dressed picks the stage keeps besides the one it shows: the row's two neighbours, made
/// ahead, and the last few shown, so stepping back and forth over a row draws at once.
const KEPT: usize = 6;

/// In the browser, which has no second thread and dresses on the menu's own, how long a pick has to
/// stay before the stage dresses it: a run of quick presses is one dress, of the last.
#[cfg(target_arch = "wasm32")]
const SETTLE: std::time::Duration = std::time::Duration::from_millis(150);

/// What of a pick the stage shows: its bike, rider and look. The Driving Model and the preset's name
/// change nothing on it, so stepping them dresses nothing.
fn shown_of(pick: &roster::Pick) -> roster::Pick {
    roster::Pick { set: roster::Pick::default().set, preset: String::new(), fliers: Vec::new(), ..pick.clone() }
}

/// The stage the Garage shows its seat's pick on.
///
/// Dressing a pick — the disc's archives read, the gear painted, the models built — is most of a
/// tenth of a second, which on the menu's thread was a frame that long at every step of a row, and a
/// press held across it long enough to repeat (P258). So a pick is dressed on a worker
/// ([`crate::thread`]: inline in the browser, after the pick settles), the stage drawing the last
/// pick it has until the new one is ready while the rows change at once. A dress finished for a pick
/// no longer the one asked for is kept, never shown: the stage never steps back to an older pick.
#[derive(Default)]
pub struct Stage {
    /// Here while no dress is under way.
    tools: Tools,
    /// The dress under way.
    job: Option<Job>,
    /// The pick asked for, as [`shown_of`] has it, and since when.
    wanted: Option<(roster::Pick, crate::clock::Instant)>,
    /// The picks to dress ahead while nothing else is to be, nearest first.
    ahead: Vec<roster::Pick>,
    /// The pick the stage shows, and what it came to; a pick that failed to dress is kept too, so it
    /// is not tried again every frame.
    pick: Option<roster::Pick>,
    dressed: Option<Dressed>,
    /// Dressed picks not shown, oldest first.
    kept: Vec<(roster::Pick, Option<Dressed>)>,
    /// Whether the GPU holds this dress's models (it forgets them for a ride's).
    pub uploaded: bool,
    /// The software frame's pixels under the stage, as the menu drew them, and where.
    under: Option<([usize; 4], Vec<u8>)>,
}

impl Stage {
    /// Ask for the stage in `pick`. It shows it at once if it has it dressed, and otherwise starts
    /// dressing it, showing what it showed until that is done. Answers whether what it shows changed.
    pub fn dress(&mut self, pick: &roster::Pick) -> bool {
        let pick = shown_of(pick);
        if self.wanted.as_ref().is_none_or(|(p, _)| *p != pick) {
            self.wanted = Some((pick.clone(), crate::clock::Instant::now()));
        }
        self.collect();
        let mut changed = false;
        if self.pick.as_ref() != Some(&pick)
            && let Some(i) = self.kept.iter().position(|(p, _)| *p == pick)
        {
            let (p, d) = self.kept.remove(i);
            if let Some(old) = self.pick.take() {
                let dressed = self.dressed.take();
                self.keep(old, dressed);
            }
            (self.pick, self.dressed, self.uploaded) = (Some(p), d, false);
            changed = true;
        }
        self.start();
        changed
    }

    /// The picks to dress ahead once the one asked for is: a row's neighbours, as
    /// [`Menu::garage_neighbours`] gives them. Not in the browser, whose dress would be a frame's.
    pub fn ahead(&mut self, picks: &[roster::Pick]) {
        if cfg!(target_arch = "wasm32") {
            return;
        }
        self.ahead = picks.iter().map(shown_of).collect();
    }

    /// Off the Garage: what was dressed ahead goes, and nothing more is started. The shown pick stays,
    /// for the Garage opened again.
    pub fn leave(&mut self) {
        self.collect();
        (self.wanted, self.ahead) = (None, Vec::new());
        self.kept.clear();
    }

    /// Whether a dress is under way.
    #[cfg(test)]
    fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The pick the stage shows, as [`shown_of`] has it.
    #[cfg(test)]
    fn shows(&self) -> Option<&roster::Pick> {
        self.pick.as_ref()
    }

    /// A finished dress's tools back, and what it made into the kept ones.
    fn collect(&mut self) {
        if !self.job.as_ref().is_some_and(|(_, h)| h.is_finished()) {
            return;
        }
        let (pick, handle) = self.job.take().expect("finished above");
        let dressed = match handle.join() {
            Ok((tools, dressed)) => {
                self.tools = tools;
                dressed
            }
            // The tools went with it: the next dress opens them again.
            Err(_) => {
                println!("garage: the dress of {} on {} stopped", pick.rider, pick.bike);
                None
            }
        };
        self.keep(pick, dressed);
    }

    /// `pick` among the kept ones, the oldest let go past [`KEPT`].
    fn keep(&mut self, pick: roster::Pick, dressed: Option<Dressed>) {
        self.kept.retain(|(p, _)| *p != pick);
        self.kept.push((pick, dressed));
        if self.kept.len() > KEPT {
            self.kept.remove(0);
        }
    }

    /// Start the next dress, if one is wanted and none is under way: the pick asked for, then the
    /// ones ahead.
    fn start(&mut self) {
        if self.job.is_some() {
            return;
        }
        let have = |p: &roster::Pick| self.pick.as_ref() == Some(p) || self.kept.iter().any(|(k, _)| k == p);
        let next = match &self.wanted {
            Some((p, _since)) if !have(p) => {
                // The browser's dress is the menu's frame: one when the presses stop.
                #[cfg(target_arch = "wasm32")]
                if _since.elapsed() < SETTLE {
                    return;
                }
                p.clone()
            }
            Some(_) => match self.ahead.iter().find(|p| !have(p)) {
                Some(p) => p.clone(),
                None => return,
            },
            None => return,
        };
        let mut tools = std::mem::take(&mut self.tools);
        let pick = next.clone();
        let handle = crate::thread::spawn(move || {
            let dressed = match dress_in(&mut tools, &pick) {
                Ok(d) => Some(d),
                Err(e) => {
                    println!("garage: no model of {} on {}: {e}", pick.rider, pick.bike);
                    None
                }
            };
            (tools, dressed)
        });
        self.job = Some((next, handle));
    }

    /// Wait for the dress under way and show the pick asked for, for a shot or a test that wants the
    /// stage as it ends up rather than as it is this frame.
    pub fn settle(&mut self) {
        let Some((pick, _)) = self.wanted.clone() else { return };
        for _ in 0..3 {
            if let Some((_, h)) = &self.job {
                while !h.is_finished() {
                    std::thread::sleep(std::time::Duration::from_millis(2));
                }
            }
            self.collect();
            #[cfg(target_arch = "wasm32")]
            {
                self.wanted = Some((pick.clone(), crate::clock::Instant::now() - SETTLE));
            }
            self.dress(&pick);
            if self.pick.as_ref() == Some(&pick) {
                return;
            }
        }
    }
}

/// Dress `pick` with `tools`, opening them if they are not yet.
fn dress_in(tools: &mut Tools, pick: &roster::Pick) -> Result<Dressed, String> {
    if tools.disc.is_none() {
        tools.disc = Some(Disc::open_configured()?);
    }
    let disc = tools.disc.as_mut().expect("opened above");
    let kit = tools.kit.get_or_insert_with(|| dress::Kit::open(disc));
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
    let mut mips: Vec<(usize, u8, Mips)> = Vec::new();
    for loaded in assets.models.values() {
        for m in 0..loaded.model.materials.len().max(1) {
            let Some(t) = loaded.textures.get(m).copied().flatten().filter(|&t| t < assets.textures.len()) else { continue };
            let surface = loaded.surfaces.get(m).copied().unwrap_or_default();
            let keep = if surface.keep_coverage { surface.cutoff } else { 0 };
            if !mips.iter().any(|&(at, k, _)| (at, k) == (t, keep)) {
                mips.push((t, keep, Mips::of(&assets.textures[t], keep)));
            }
        }
    }
    Ok(Dressed { assets, bike, parts, body, centre, radius, mips })
}

impl Stage {

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
            for (t, keep, mips) in &d.mips {
                r.put_mips(*t, *keep, mips);
            }
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
                Actor { model, at, pose, lod_from: None, alpha: 1.0, casts: false }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The picks one step either side of `g`'s row, as [`Menu::garage_neighbours`] gives them.
    fn neighbours(g: &roster::Garage, c: &roster::Catalogue) -> Vec<roster::Pick> {
        [1, -1]
            .into_iter()
            .map(|by| {
                let mut n = g.clone();
                n.turn(c, by);
                n.pick
            })
            .collect()
    }

    /// P258: ten quick Paint presses, three frames apart, leave the stage on the tenth pick; no
    /// frame's [`Stage::dress`] takes over 20 ms, since the dress is on its worker; and what the
    /// stage shows only ever changes to the pick asked for then, never back to an older one. Prints
    /// what a dress costs, which was the frame's before.
    #[test]
    fn ten_quick_paint_presses_end_on_the_tenth_without_a_long_frame() {
        let mut disc = match Disc::open_configured() {
            Ok(d) => d,
            Err(e) => return println!("skipped: no disc ({e})"),
        };
        let c = roster::Catalogue::read(&mut disc).unwrap();
        drop(disc);
        let mut g = roster::Garage::new(&c, &roster::Pick { bike: "yz125".into(), ..roster::Pick::default() });
        g.row = roster::ROWS.iter().position(|r| *r == roster::Row::Livery).unwrap();
        // What one dress costs on this thread: the frame a step was, before.
        let mut tools = Tools::default();
        dress_in(&mut tools, &g.pick).unwrap();
        let mut once = Vec::new();
        for by in [1, 1, -1, -1] {
            let mut n = g.clone();
            n.turn(&c, by);
            let t = crate::clock::Instant::now();
            dress_in(&mut tools, &n.pick).unwrap();
            once.push(t.elapsed());
        }
        println!("a dress on the menu's thread: {once:?}");

        let mut stage = Stage::default();
        stage.dress(&g.pick);
        stage.settle();
        assert_eq!(stage.shows(), Some(&shown_of(&g.pick)), "the Garage opened on its pick");
        let (mut slowest, mut frames) = (Duration::ZERO, Vec::new());
        let mut frame = |stage: &mut Stage, g: &roster::Garage| {
            let t = crate::clock::Instant::now();
            let before = stage.shows().cloned();
            let changed = stage.dress(&g.pick);
            stage.ahead(&neighbours(g, &c));
            let took = t.elapsed();
            slowest = slowest.max(took);
            frames.push(took);
            if stage.shows() != before.as_ref() {
                assert!(changed);
                assert_eq!(stage.shows(), Some(&shown_of(&g.pick)), "the stage showed a pick not asked for");
            }
            std::thread::sleep(Duration::from_micros(16_667));
        };
        for _ in 0..10 {
            g.turn(&c, 1);
            for _ in 0..3 {
                frame(&mut stage, &g);
            }
        }
        let until = crate::clock::Instant::now() + Duration::from_secs(20);
        while stage.shows() != Some(&shown_of(&g.pick)) && crate::clock::Instant::now() < until {
            frame(&mut stage, &g);
        }
        assert_eq!(stage.shows(), Some(&shown_of(&g.pick)), "the stage ends on the tenth pick");
        assert!(stage.dressed.is_some());
        frames.sort();
        println!("a menu frame's dress, after: median {:?}, slowest {slowest:?} of {}", frames[frames.len() / 2], frames.len());
        assert!(slowest < Duration::from_millis(20), "a frame took {slowest:?}");
        // Left alone, it dresses the row's neighbours ahead, and a step onto one shows on the press's
        // frame.
        let until = crate::clock::Instant::now() + Duration::from_secs(20);
        while crate::clock::Instant::now() < until {
            stage.dress(&g.pick);
            stage.ahead(&neighbours(&g, &c));
            if !stage.busy() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        g.turn(&c, -1);
        stage.dress(&g.pick);
        assert_eq!(stage.shows(), Some(&shown_of(&g.pick)), "a kept pick shows at once");
        // What is left on the frame that shows a new pick on the GPU: its upload, the levels made
        // already. Said, not held to a number: it is the GPU's, which a test may not have.
        if let (Ok(mut r), Some(d)) = (Renderer::headless(), &stage.dressed) {
            let t = crate::clock::Instant::now();
            r.forget_assets();
            for (at, keep, mips) in &d.mips {
                r.put_mips(*at, *keep, mips);
            }
            let names: Vec<&String> = d.assets.models.keys().collect();
            r.upload_models(&d.assets, &names);
            r.finish();
            println!("the upload on the frame a pick shows: {:?}", t.elapsed());
        }
    }
}
