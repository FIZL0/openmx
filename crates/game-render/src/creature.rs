//! A track's creatures: the birds of a `.toy`'s bird manager, and its `SequenceAnimatedToy`s (V44).
//!
//! Both are drawn only — nothing in the ride touches them — but the birds answer the rider: a
//! `[TriggerN]` box with a `ScareBirds` action sends the perched birds near it up their take-off
//! splines. So this module is stepped by the simulation's ticks with each rider's body rays
//! ([`Creatures::step`]), not by frames, and two runs of the same ride see the same birds.
//!
//! **Splines.** `[BirdSplines]`, then `[BirdSplineN]`: a `VUE` file of nodes, `LoopSequence` (default
//! on), `FPS` (default 60, which is the speed in feet a second, not a frame rate), `PerchLookVector`,
//! and the spline it is entered from (`FromSplineName` at `FromNode`) and the one it leads to
//! (`ToSplineName` at `ToNode`). A `.vue` is a 3ds Max export: `FRAME n` lines, each followed by a
//! `transform "<object>"` line of twelve floats, the object's matrix with Z up. A node is its
//! translation and its Z axis, both turned to the game's Y up — `(f9, f11, f10)` and `(f6, f8, f7)`.
//!
//! **Birds.** `[Birds]` places single birds (`MCFName`, `StartPosition`, `IsFlying`,
//! `StartingSplineName`, `StartingSplineNode`), and `[BirdFlocks]` flies `NumFlyingBirds` of one
//! `.mcf` round a looping spline. A flock is laid out at load in groups of three or four
//! (`rand()`), each group on its own node, a third of the flock's size apart along the spline; every
//! bird after the first is then given an offset of its own from the spline, in a pattern of eight
//! that grows by `BirdFlockOffset` feet each time round ([`flock_offset`]).
//!
//! A flying bird heads for a target point at its spline's `FPS`; within two feet of it the bird
//! moves on a node, and on a looping spline its target is that node shifted by the bird's offset —
//! up, and sideways across the node's up. Where the spline is not looping and runs out, the bird
//! either lands (if what follows is not looping either: a landing spline leading to a take-off
//! one) or flies on into the spline it leads to. On a looping spline a bird passing the node a
//! landing spline is entered from takes that spline if nobody else has, and slows down along it.
//! A `ScareBirds` sends every bird sitting within `ScareDistance` of `ScarePosition` on.
//!
//! **Sequence toys.** `[SequenceAnimatedToyN]` is a model at `Position`, faced along
//! `LookVector`/`UpVector`, that plays its `MotionToPlay` clips once each, in the order of
//! `MotionSequence`: at each step a roll of `rand()` against that step's `ProbabilityList` percent
//! decides whether it plays or the sequence moves on to the next step ([`Toy::next`]).
//!
//! **What is the port's own.** The original's `rand()` is one generator for the whole game, seeded
//! from the clock at the first call; this module has a generator of its own on the library's
//! default seed, so a flock is laid out the same way every run. The original animates a bird only
//! while it is among the nearest to the camera and a sequence toy only while it is on screen; here
//! every one is animated every tick, because a camera has no business in a tick. The research notes
//! (oracle.txt, V44) have the rest.

use crate::disc::Res;
use crate::ini::{self, Section};
use crate::mat::{M4, V3, v3};
use crate::psg::Model;
use crate::rider::MotionSet;
use crate::skin::{Clip, Pose, Skeleton};
use crate::toy::upright;
use game_sim::rng::Rng;
use ps2_float::{Vf, fpu};

/// The original's `rand()` scale, `2⁻³¹` as the single it multiplies by.
const UNIT: f32 = 4.656_613e-10;

/// How close a bird has to come to its target to have reached it, in feet.
const REACHED: f32 = 2.0;

/// The motions a bird's `.mcf` lists, in the order the bird asks for them by.
pub const IDLE: usize = 0;
pub const TAKE_OFF: usize = 1;
pub const FLY: usize = 2;
pub const LAND: usize = 3;

/// One node of a bird spline: where it is, and the up its bird flies with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub pos: V3,
    pub up: V3,
}

/// A `.vue`'s nodes, one a `transform` line.
pub fn vue(text: &str) -> Vec<Node> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        if words.next() != Some("transform") {
            continue;
        }
        let _object = words.next();
        let f: Vec<f32> = words.filter_map(|w| w.parse().ok()).collect();
        if f.len() < 12 {
            continue;
        }
        out.push(Node { pos: v3(f[9], f[11], f[10]), up: v3(f[6], f[8], f[7]) });
    }
    out
}

/// One `[BirdSplineN]`.
#[derive(Clone, Debug, Default)]
pub struct Spline {
    pub name: String,
    pub vue: String,
    pub nodes: Vec<Node>,
    /// `LoopSequence`.
    pub looping: bool,
    /// `WaitToBeTriggered`: read and kept, and never asked for by the bird code.
    pub wait: bool,
    /// `FPS`: feet a second along it.
    pub fps: f32,
    /// `PerchLookVector`: which way a bird that lands at its end faces.
    pub perch_look: V3,
    from_name: String,
    to_name: String,
    /// `FromSplineName`, as an index into the manager's splines.
    pub from: Option<usize>,
    /// `FromNode` less one.
    pub from_node: i32,
    /// `ToSplineName`, as an index.
    pub to: Option<usize>,
    /// `ToNode` less one (and less one again when a bird is switched onto it, as the original does).
    pub to_node: i32,
    /// A bird is on it or on its way down it: no second one takes it.
    pub occupied: bool,
}

fn spline(s: &Section) -> Spline {
    let int = |k: &str, d: i32| s.get(k).map_or(d, |v| v.trim().parse().unwrap_or(0));
    let look = s.numbers("PerchLookVector").filter(|v| v.len() >= 3).map_or(v3(0.0, 0.0, 1.0), |v| v3(v[0], v[1], v[2]));
    Spline {
        name: s.get("Name").unwrap_or("").to_string(),
        vue: s.get("VUE").unwrap_or("").trim_matches('"').trim().to_string(),
        nodes: Vec::new(),
        looping: s.get("LoopSequence").is_none_or(ini::switch),
        wait: s.get("WaitToBeTriggered").is_some_and(ini::switch),
        fps: s.number("FPS").unwrap_or(60.0),
        perch_look: look,
        from_name: s.get("FromSplineName").unwrap_or("").to_string(),
        to_name: s.get("ToSplineName").unwrap_or("").to_string(),
        from: None,
        from_node: int("FromNode", 0) - 1,
        to: None,
        to_node: int("ToNode", 0) - 1,
        occupied: false,
    }
}

/// The flock offset of the bird `i` places after the manager's first, `step` feet a ring: `(side, up)`.
///
/// The original's pattern of eight, ring after ring: no side offset for the first two of a ring
/// and alternating sides for the rest, no height for the third and fourth and below for the second,
/// seventh and eighth. The side grows after the first of a ring and the height after the last, so
/// the first ring is beside and over the spline at `0` and `step` and each ring after is wider.
pub fn flock_offset(count: usize, step: i32) -> Vec<(f32, f32)> {
    let mut out = vec![(0.0, 0.0)];
    let (mut side, mut up) = (0i32, step);
    for u in 0..count.saturating_sub(1) {
        let v = u % 8;
        let wide = v >= 2;
        let mut x = if wide { side as f32 } else { 0.0 };
        if wide && u % 2 == 1 {
            x = -x;
        }
        let mut y = if (2..4).contains(&v) { 0.0 } else { up as f32 };
        if v > 5 || v == 1 {
            y = -y;
        }
        out.push((x, y));
        if v == 0 {
            side += step;
        }
        if v == 7 {
            up += step;
        }
    }
    out.truncate(count);
    out
}

/// A clip of a body, as one of the names it is asked for by.
#[derive(Clone, Debug)]
pub struct Motion {
    pub name: String,
    pub clip: Option<Clip>,
    pub map: Vec<u8>,
    pub looping: bool,
}

/// A `.mcf`'s geometry and clips: what a bird or a sequence toy is drawn with.
#[derive(Clone, Debug)]
pub struct Body {
    pub mcf: String,
    /// The model's name as the renderer's assets know it.
    pub model: String,
    pub skeleton: Skeleton,
    set: MotionSet,
    /// Every clip asked for so far, by name.
    pub motions: Vec<Motion>,
}

impl Body {
    fn load(archives: &[Res], mcf: &str) -> Result<Body, String> {
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        let text = read(mcf).ok_or_else(|| format!("{mcf}: no such member"))?;
        let text = ini::text(&text);
        let set = MotionSet::parse(&text);
        // `Looping=TRUE` in these files, which the rider's reader (`yes` only) does not take: read
        // again here the way the original's switch reader takes it, section by section.
        let looping: Vec<bool> = ini::parse(&text)
            .iter()
            .filter(|s| s.name.to_ascii_lowercase().starts_with("motion") && s.get("NAME").is_some())
            .map(|s| s.get("Looping").is_some_and(ini::switch))
            .collect();
        let model = set.geometry();
        let psg = format!("{model}.psg");
        let bytes = read(&psg).ok_or_else(|| format!("{mcf}: {psg}: no such member"))?;
        let parsed = Model::parse(&bytes).map_err(|e| format!("{psg}: {e}"))?;
        let mut body = Body { mcf: mcf.to_string(), model, skeleton: Skeleton::of(&parsed), set, motions: Vec::new() };
        // A bird asks for its four motions by their place in the list, so they are bound first and
        // in order; a sequence toy asks by name ([`Body::motion`]).
        for (i, m) in body.set.motions.clone().iter().enumerate() {
            body.bind(archives, &m.name, Some(&m.clip), looping.get(i).copied().unwrap_or(false));
        }
        Ok(body)
    }

    /// The index of the motion of that name, bound now if it was not yet. A name the `.mcf` lists
    /// plays its `VUE`'s `.MOT`; one it does not is `<name>.MOT`, the original's load-on-demand.
    fn motion(&mut self, archives: &[Res], name: &str) -> usize {
        if let Some(i) = self.motions.iter().position(|m| m.name.eq_ignore_ascii_case(name)) {
            return i;
        }
        self.bind(archives, name, None, false)
    }

    fn bind(&mut self, archives: &[Res], name: &str, vue: Option<&str>, looping: bool) -> usize {
        let file = match vue {
            Some(v) => format!("{}{}.mot", self.set.directory, v.rsplit_once('.').map_or(v, |(s, _)| s)),
            None => format!("{}{name}.mot", self.set.directory),
        };
        let clip = archives.iter().find_map(|a| a.read_suffix(&file).ok()).and_then(|b| Clip::parse(&b).ok());
        let map = clip.as_ref().map_or_else(Vec::new, |c| c.joints(&self.skeleton));
        self.motions.push(Motion { name: name.to_string(), clip, map, looping });
        self.motions.len() - 1
    }

    /// The joint matrices of a motion `time` seconds in.
    pub fn palette(&self, motion: Option<usize>, time: f32) -> Vec<M4> {
        let mut pose = Pose::new(self.skeleton.len());
        if let Some(m) = motion.and_then(|m| self.motions.get(m))
            && let Some(clip) = &m.clip
        {
            clip.pose(time, &m.map, &mut pose);
        }
        self.skeleton.world(&pose)
    }

    fn duration(&self, motion: Option<usize>) -> Option<f32> {
        motion.and_then(|m| self.motions.get(m)).and_then(|m| m.clip.as_ref()).map(Clip::duration)
    }

    fn looping(&self, motion: usize) -> bool {
        self.motions.get(motion).is_some_and(|m| m.looping)
    }
}

/// A clip being played: which, how far in, and whether it has run out (a clip that does not loop
/// stops at its end and holds its last pose until something plays another).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Playing {
    pub motion: Option<usize>,
    pub time: f32,
    pub finished: bool,
}

impl Playing {
    fn play(&mut self, motion: Option<usize>) {
        *self = Playing { motion, time: 0.0, finished: false };
    }

    fn advance(&mut self, body: &Body, dt: f32) {
        if self.finished {
            return;
        }
        self.time += dt;
        let Some(d) = body.duration(self.motion) else {
            self.finished = true;
            return;
        };
        if self.time >= d {
            if self.motion.is_some_and(|m| body.looping(m)) && d > 0.0 {
                self.time -= d;
            } else {
                self.time = d;
                self.finished = true;
            }
        }
    }
}

/// One bird.
#[derive(Clone, Debug)]
pub struct Bird {
    pub body: usize,
    pub spline: usize,
    pub node: i32,
    pub next: i32,
    pub target: V3,
    pub pos: V3,
    /// Where it was at the tick before, which a picture between ticks slides from.
    pub before: V3,
    /// Its offset from a looping spline: sideways, then up.
    pub offset: (f32, f32),
    pub flying: bool,
    /// 0 flying a spline at its speed, 1 coming down a landing spline.
    pub mode: i32,
    pub forward: V3,
    pub up: V3,
    pub playing: Playing,
    /// A flock's bird that has not been laid out yet. Load-time only.
    unplaced: bool,
}

/// One `ScareBirds` action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scare {
    pub at: V3,
    pub distance: f32,
    pub delay: f32,
}

/// What of one rider touches a trigger: the bike's body rays (`bikem.col`, the eleven segments
/// from the seat out) at the chassis, in world space. The original's trigger is struck through
/// the collision pass, and of a riding bike's probes only these rays reach it: the sweep is a
/// sphere, whose pair test has no case for a box, and the second ray probe ignores the trigger
/// category. Not the rider's position, which is a point the collision pass never tests.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rays(pub Vec<[V3; 2]>);

impl Rays {
    /// A single point, for a rider with no body rays.
    pub fn at(p: V3) -> Rays {
        Rays(vec![[p, p]])
    }
}

/// One `[TriggerN]` with the actions this module answers to.
#[derive(Clone, Debug)]
pub struct Trigger {
    pub name: String,
    pub at: V3,
    /// `Box`: half the box along its right, up and look (the shape keeps it as its half extents).
    pub half: V3,
    /// `Radius`, which makes it a sphere instead.
    pub radius: Option<f32>,
    pub right: V3,
    pub up: V3,
    pub look: V3,
    pub enabled: bool,
    pub one_time: bool,
    pub percent: i32,
    pub fired: bool,
    pub scares: Vec<Scare>,
}

impl Trigger {
    /// Whether a point is inside: a segment of no length.
    pub fn holds(&self, p: V3) -> bool {
        self.struck(p, p)
    }

    /// Whether the segment `a`–`b` touches the shape. For a box this is the original's
    /// segment-against-box test, in the box's own frame: the segment's midpoint `m` and half
    /// length `d`, apart on one of the box's axes (`|m| > half + |d|`) or on one of the three
    /// axes across the segment, and they miss; the edges count as touching.
    pub fn struck(&self, a: V3, b: V3) -> bool {
        let local = |p: V3| {
            let d = p.sub(self.at);
            v3(d.dot(self.right), d.dot(self.up), d.dot(self.look))
        };
        match self.radius {
            Some(r) => segment_point_distance(a, b, self.at) < r,
            None => {
                let (a, b, h) = (local(a), local(b), self.half);
                let vf = |v: V3| Vf::new(v.x, v.y, v.z, 1.0);
                segment_touches_box(Vf::ZERO, vf(h), vf(a), vf(b), &BOX_SPACE)
            }
        }
    }
}

/// The box test's segment transform when the points are already in the box's frame.
const BOX_SPACE: [Vf; 4] = [
    Vf::new(1.0, 0.0, 0.0, 0.0),
    Vf::new(0.0, 1.0, 0.0, 0.0),
    Vf::new(0.0, 0.0, 1.0, 0.0),
    Vf::new(0.0, 0.0, 0.0, 1.0),
];

/// The engine's segment-against-box test, the leaf of a ray tree against a box shape: the segment
/// `a`–`b` through `m` (rows right, up, look, origin, summed origin first), then about the box's
/// `centre` with half extents `half`. Its midpoint `m` and half length `d`: apart on a box axis
/// (`|m| - half - |d| > 0`) or on one of the three axes across the segment, and they miss; touching
/// counts. The VU's sums and the FPU's accumulator, as the original runs them.
pub fn segment_touches_box(centre: Vf, half: Vf, a: Vf, b: Vf, m: &[Vf; 4]) -> bool {
    let through = |p: Vf| {
        let acc = m[3].add(Vf::ZERO);
        let acc = Vf::madd(acc, m[0], Vf::splat(p.x()));
        let acc = Vf::madd(acc, m[1], Vf::splat(p.y()));
        Vf::madd(acc, m[2], Vf::splat(p.z()))
    };
    let (a, b) = (through(a).sub(centre), through(b).sub(centre));
    let mid = a.add(b).mul(Vf::splat(0.5));
    let d = mid.sub(a);
    let abs = |v: Vf| Vf::from_bits(v.to_bits().map(|x| x & 0x7fff_ffff));
    let (ad, am) = (abs(d), abs(mid));
    let apart = am.sub(half).sub(ad);
    if apart.x() > 0.0 || apart.y() > 0.0 || apart.z() > 0.0 {
        return false;
    }
    let (h, (mx, my, mz), (dx, dy, dz)) = (half, (mid.x(), mid.y(), mid.z()), (d.x(), d.y(), d.z()));
    let across = |m1: f32, d1: f32, m2: f32, d2: f32| fpu::abs(fpu::msub(fpu::mula(m1, d1), m2, d2));
    let reach = |h1: f32, a1: f32, h2: f32, a2: f32| fpu::madd(fpu::mula(h1, a1), h2, a2);
    across(my, dz, mz, dy) <= reach(h.y(), ad.z(), h.z(), ad.y())
        && across(mx, dz, mz, dx) <= reach(h.x(), ad.z(), h.z(), ad.x())
        && across(mx, dy, my, dx) <= reach(h.x(), ad.y(), h.y(), ad.x())
}

/// How near the segment `a`–`b` comes to `p`.
fn segment_point_distance(a: V3, b: V3, p: V3) -> f32 {
    let ab = b.sub(a);
    let len = ab.dot(ab);
    let t = if len > 0.0 { (p.sub(a).dot(ab) / len).clamp(0.0, 1.0) } else { 0.0 };
    p.sub(a.add(ab.scale(t))).len()
}

fn trigger(s: &Section, actions: &[Section], n: usize) -> Trigger {
    let vec = |k: &str, d: V3| s.numbers(k).filter(|v| v.len() >= 3).map_or(d, |v| v3(v[0], v[1], v[2]));
    // The box's frame as the trigger reader builds it (its basis call with orthogonalising off):
    // up and look each normalised and kept as they are, right their cross. Every bird trigger on
    // the disc has the two square to each other, so this frame is a rotation and its transpose
    // takes a point into the box.
    let look = vec("LookVector", v3(0.0, 0.0, 1.0)).norm();
    let up = vec("UpVector", v3(0.0, 1.0, 0.0)).norm();
    let right = up.cross(look);
    let count = s.number("NumberOfActions").unwrap_or(0.0) as usize;
    let mut scares = Vec::new();
    for a in 1..=count {
        let name = format!("Trigger{n}Action{a}");
        let Some(act) = actions.iter().find(|x| x.name.eq_ignore_ascii_case(&name)) else { continue };
        if !act.get("ActionType").is_some_and(|t| t.trim().eq_ignore_ascii_case("ScareBirds")) {
            continue;
        }
        let at = act.numbers("ScarePosition").filter(|v| v.len() >= 3).map_or(V3::default(), |v| v3(v[0], v[1], v[2]));
        scares.push(Scare {
            at,
            distance: act.number("ScareDistance").unwrap_or(0.0),
            delay: act.number("ScareDelay").unwrap_or(0.0),
        });
    }
    Trigger {
        name: s.get("Name").unwrap_or("").to_string(),
        at: vec("Position", V3::default()),
        half: vec("Box", V3::default()),
        radius: s.number("Radius").filter(|r| *r > 0.0),
        right,
        up,
        look,
        enabled: s.get("StartEnabled").is_none_or(ini::switch),
        one_time: s.get("OneTimeOnly").is_some_and(ini::switch),
        percent: s.number("RandomTriggerPercent").unwrap_or(100.0) as i32,
        fired: false,
        scares,
    }
}

/// One `[SequenceAnimatedToyN]`.
#[derive(Clone, Debug)]
pub struct Toy {
    pub name: String,
    pub body: usize,
    pub at: M4,
    /// `MotionToPlayN`, as the body's motion indices.
    pub motions: Vec<usize>,
    /// `MotionSequence`: one-based indices into `motions`.
    pub sequence: Vec<i32>,
    /// `ProbabilityList`: percent, step by step.
    pub chance: Vec<i32>,
    pub cursor: usize,
    pub playing: Playing,
    before: Playing,
}

impl Toy {
    /// The next motion: from the cursor on, each step plays with its percent of chance, and once
    /// every step has been passed over the next is played whatever its chance.
    fn next(&mut self, rng: &mut Rng) {
        let count = self.sequence.len();
        if count == 0 {
            self.playing.play(None);
            return;
        }
        let mut tries = 0;
        loop {
            let r = rng.rand();
            let at = self.cursor;
            let chance = self.chance.get(at).copied().unwrap_or(0);
            let picked = ((r as f32 * UNIT * 100.0) as i32) <= chance - 1 || tries > count;
            tries += 1;
            self.cursor = (self.cursor + 1) % count;
            if picked {
                let m = usize::try_from(self.sequence[at] - 1).ok().and_then(|i| self.motions.get(i)).copied();
                self.playing.play(m);
                return;
            }
        }
    }
}

/// Something to draw: the model, where, and its joints.
#[derive(Clone, Debug)]
pub struct Placed {
    pub model: String,
    pub at: M4,
    pub palette: Vec<M4>,
}

/// Every creature of a track.
#[derive(Clone, Debug, Default)]
pub struct Creatures {
    pub bodies: Vec<Body>,
    pub splines: Vec<Spline>,
    pub birds: Vec<Bird>,
    pub triggers: Vec<Trigger>,
    pub toys: Vec<Toy>,
    pub rng: Rng,
    /// What was left out, with why.
    pub skipped: Vec<String>,
    /// The tick length last stepped by, which a picture between ticks winds the clips back by.
    last_dt: f32,
}

impl Creatures {
    /// The creatures of the track whose `.toy` is `<track>.toy` in one of these archives.
    pub fn load(archives: &[Res], track: &str) -> Creatures {
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        match read(&format!("{track}.toy")) {
            Some(text) => Creatures::parse(archives, &ini::text(&text)),
            None => Creatures::default(),
        }
    }

    /// The creatures a `.toy`'s text lists, their files read out of these archives.
    pub fn parse(archives: &[Res], text: &str) -> Creatures {
        let sections = ini::parse(text);
        let section = |name: &str| sections.iter().find(|s| s.name.eq_ignore_ascii_case(name));
        let count = |head: &str, key: &str| section(head).and_then(|s| s.number(key)).map_or(0, |n| n.max(0.0) as usize);
        let mut c = Creatures { rng: Rng::new(), ..Creatures::default() };
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());

        // The manager's settings. Only `BirdFlockOffset` is the birds' business here; the distance
        // and per-tick budget are the original's animation level of detail, which is not ported.
        let offset_step = section("BirdManagers").and_then(|s| s.number("BirdFlockOffset")).map_or(0, |v| v as i32);

        for i in 1..=count("BirdSplines", "NumberOfBirdSplines") {
            let Some(s) = section(&format!("BirdSpline{i}")) else { continue };
            let mut sp = spline(s);
            match read(&sp.vue) {
                Some(bytes) => sp.nodes = vue(&ini::text(&bytes)),
                None => c.skipped.push(format!("spline {}: {} is in no archive", sp.name, sp.vue)),
            }
            c.splines.push(sp);
        }
        let index = |splines: &[Spline], name: &str| splines.iter().position(|s| s.name.eq_ignore_ascii_case(name.trim()));
        for i in 0..c.splines.len() {
            c.splines[i].from = index(&c.splines, &c.splines[i].from_name.clone());
            c.splines[i].to = index(&c.splines, &c.splines[i].to_name.clone());
        }

        // The single birds, then the flocks', in the manager's one list.
        for i in 1..=count("Birds", "NumberOfBirds") {
            let Some(s) = section(&format!("Bird{i}")) else { continue };
            let mcf = s.get("MCFName").unwrap_or("").to_string();
            let Some(body) = c.body(archives, &mcf) else { continue };
            let start = s.numbers("StartPosition").filter(|v| v.len() >= 3).map_or(V3::default(), |v| v3(v[0], v[1], v[2]));
            let flying = s.get("IsFlying").is_some_and(ini::switch);
            let name = s.get("StartingSplineName").unwrap_or("");
            let Some(spline) = index(&c.splines, name).filter(|&k| c.splines[k].nodes.len() >= 2) else {
                c.skipped.push(format!("bird {i}: spline {name} has no nodes"));
                continue;
            };
            let node = s.number("StartingSplineNode").map_or(0, |n| n as i32 - 1).max(0);
            let bird = c.bird(body, spline, node, start, flying);
            c.birds.push(bird);
        }
        let mut flocks = Vec::new();
        for i in 1..=count("BirdFlocks", "NumberOfBirdFlocks") {
            let Some(s) = section(&format!("BirdFlock{i}")) else { continue };
            let mcf = s.get("MCFName").unwrap_or("").to_string();
            let name = s.get("LoopingSpline").unwrap_or("");
            let number = s.number("NumFlyingBirds").map_or(0, |n| n.max(0.0) as usize);
            let Some(spline) = index(&c.splines, name).filter(|&k| c.splines[k].nodes.len() >= 2) else {
                c.skipped.push(format!("flock {i}: spline {name} has no nodes"));
                continue;
            };
            let Some(body) = c.body(archives, &mcf) else { continue };
            for _ in 0..number {
                let mut b = c.bird(body, spline, 0, V3::default(), true);
                b.unplaced = true;
                c.birds.push(b);
            }
            flocks.push((spline, number));
        }
        for (spline, number) in flocks {
            c.lay_out(spline, number);
        }
        let offsets = flock_offset(c.birds.len(), offset_step);
        for (b, o) in c.birds.iter_mut().zip(offsets) {
            b.offset = o;
        }

        let actions: Vec<Section> = sections.iter().filter(|s| s.name.to_ascii_lowercase().contains("action")).cloned().collect();
        for i in 1..=count("Triggers", "NumberOfTriggers") {
            let Some(s) = section(&format!("Trigger{i}")) else { continue };
            let t = trigger(s, &actions, i);
            if !t.scares.is_empty() {
                c.triggers.push(t);
            }
        }

        for i in 1..=count("SequenceAnimatedToys", "NumberOfSequenceAnimatedToys") {
            let Some(s) = section(&format!("SequenceAnimatedToy{i}")) else { continue };
            let mcf = s.get("MCFName").unwrap_or("").to_string();
            let Some(body) = c.body(archives, &mcf) else { continue };
            let vec = |k: &str, d: V3| s.numbers(k).filter(|v| v.len() >= 3).map_or(d, |v| v3(v[0], v[1], v[2]));
            let at = upright(vec("Position", V3::default()), vec("LookVector", v3(0.0, 0.0, 1.0)), vec("UpVector", v3(0.0, 1.0, 0.0)));
            let mut motions = Vec::new();
            for m in 1..=s.number("NumberOfMotionsInSequence").map_or(0, |n| n.max(0.0) as usize) {
                let name = s.get(&format!("MotionToPlay{m}")).unwrap_or("").trim().to_string();
                motions.push(c.bodies[body].motion(archives, &name));
            }
            let steps = s.number("NumberInSequence").map_or(0, |n| n.max(0.0) as usize);
            let list = |k: &str| -> Vec<i32> {
                let mut v: Vec<i32> = s.get(k).unwrap_or("").split(',').filter_map(|w| w.trim().parse().ok()).collect();
                v.resize(steps, 0);
                v
            };
            let mut toy = Toy {
                name: s.get("Name").unwrap_or("").to_string(),
                body,
                at,
                motions,
                sequence: list("MotionSequence"),
                chance: list("ProbabilityList"),
                cursor: 0,
                playing: Playing::default(),
                before: Playing::default(),
            };
            toy.next(&mut c.rng);
            toy.before = toy.playing;
            c.toys.push(toy);
        }
        c
    }

    fn body(&mut self, archives: &[Res], mcf: &str) -> Option<usize> {
        if let Some(i) = self.bodies.iter().position(|b| b.mcf.eq_ignore_ascii_case(mcf)) {
            return Some(i);
        }
        match Body::load(archives, mcf) {
            Ok(b) => {
                self.bodies.push(b);
                Some(self.bodies.len() - 1)
            }
            Err(e) => {
                self.skipped.push(e);
                None
            }
        }
    }

    fn bird(&mut self, body: usize, spline: usize, node: i32, start: V3, flying: bool) -> Bird {
        let mut b = Bird {
            body,
            spline,
            node,
            next: node + 1,
            target: start,
            pos: start,
            before: start,
            offset: (0.0, 0.0),
            flying,
            mode: 0,
            forward: v3(0.0, 0.0, 1.0),
            up: v3(0.0, 1.0, 0.0),
            playing: Playing::default(),
            unplaced: false,
        };
        let sp = &mut self.splines[spline];
        if flying {
            if let Some(n) = sp.nodes.get(b.next as usize) {
                b.target = n.pos;
            }
            b.playing.play(Some(FLY));
        } else {
            if !sp.looping {
                sp.occupied = true;
                b.forward = sp.perch_look;
            }
            b.playing.play(Some(IDLE));
        }
        b
    }

    /// A flock's birds onto their spline: groups of three or four, each on its own node.
    fn lay_out(&mut self, spline: usize, number: usize) {
        let third = 0.333f32 * number as f32;
        let nodes = self.splines[spline].nodes.clone();
        let groups = (third + 1.0) as i32;
        let mut placed = 0;
        for g in 0..groups {
            if placed >= number {
                break;
            }
            let size = (self.rng.rand() as f32 * UNIT * 2.0) as usize + 3;
            let mut in_group = 0;
            for b in &mut self.birds {
                if in_group >= size || placed >= number {
                    break;
                }
                if !b.unplaced {
                    continue;
                }
                b.spline = spline;
                b.node = (g as f32 * (nodes.len() as f32 / third)) as i32;
                b.next = b.node + 1;
                if b.next as usize >= nodes.len() {
                    b.next = 0;
                }
                b.target = nodes[b.next as usize].pos;
                b.pos = nodes[(b.node as usize).min(nodes.len() - 1)].pos;
                b.before = b.pos;
                b.playing.play(Some(FLY));
                b.unplaced = false;
                in_group += 1;
                placed += 1;
            }
        }
    }

    /// The models to load for drawing.
    pub fn models(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for b in &self.bodies {
            if !out.contains(&b.model.as_str()) {
                out.push(&b.model);
            }
        }
        out
    }

    /// One tick: the triggers the riders' body rays touch (each rider a list of world-space
    /// segments, [`Rays::at`] for a bare point), then every bird, then every toy.
    pub fn step(&mut self, dt: f32, riders: &[Rays]) {
        self.last_dt = dt;
        for b in &mut self.birds {
            b.before = b.pos;
        }
        for t in 0..self.triggers.len() {
            let tr = &self.triggers[t];
            if !tr.enabled || tr.one_time && tr.fired || !riders.iter().any(|r| r.0.iter().any(|&[a, b]| tr.struck(a, b))) {
                continue;
            }
            if tr.percent != 100 && tr.percent < (self.rng.rand() as f32 * UNIT * 100.0) as i32 + 1 {
                continue;
            }
            self.triggers[t].fired = true;
            for s in self.triggers[t].scares.clone() {
                self.scare(s);
            }
        }
        for b in 0..self.birds.len() {
            self.update(b, dt);
        }
        for toy in &mut self.toys {
            toy.before = toy.playing;
            if toy.playing.finished {
                toy.next(&mut self.rng);
            }
            toy.playing.advance(&self.bodies[toy.body], dt);
        }
    }

    /// Every sitting bird near the scare sent on its way.
    fn scare(&mut self, s: Scare) {
        for b in 0..self.birds.len() {
            if self.birds[b].flying || self.birds[b].pos.sub(s.at).len() >= s.distance {
                continue;
            }
            self.transitions(b);
            self.birds[b].node -= 1;
            self.advance(b);
            self.birds[b].flying = true;
            self.birds[b].mode = 0;
        }
    }

    fn update(&mut self, b: usize, dt: f32) {
        if self.birds[b].flying && self.birds[b].target.sub(self.birds[b].pos).len() < REACHED {
            if self.birds[b].mode != 2 {
                self.advance(b);
            }
            self.transitions(b);
        }
        if self.birds[b].flying {
            self.fly(b, dt);
        }
        let body = &self.bodies[self.birds[b].body];
        self.birds[b].playing.advance(body, dt);
    }

    fn play(&mut self, b: usize, motion: usize) {
        self.birds[b].playing.play(Some(motion));
    }

    /// The bird's node and next node kept on its spline.
    fn wrap(&mut self, b: usize) {
        let n = self.splines[self.birds[b].spline].nodes.len() as i32;
        let bird = &mut self.birds[b];
        if bird.node < 0 || bird.node >= n {
            bird.node = 0;
        }
        bird.next = bird.node + 1;
        if bird.next >= n {
            bird.next = 0;
        }
    }

    /// On a node: the next one is the target, shifted by the bird's offset on a looping spline,
    /// and the bird turns to face it with the node's up.
    fn advance(&mut self, b: usize) {
        self.birds[b].node += 1;
        self.wrap(b);
        let sp = &self.splines[self.birds[b].spline];
        let bird = &mut self.birds[b];
        let next = sp.nodes[bird.next as usize];
        bird.target = if sp.looping {
            let raised = next.pos.add(v3(0.0, bird.offset.1, 0.0));
            let toward = raised.sub(bird.pos).norm();
            raised.add(next.up.cross(toward).scale(bird.offset.0))
        } else {
            next.pos
        };
        let toward = bird.target.sub(bird.pos).norm();
        let up = sp.nodes[bird.node as usize].up;
        face(bird, toward, up);
    }

    /// Onto another spline at a node, `node` read the way the original reads it: less one unless
    /// it is 0, and the last node if that leaves it below 0. A spline with no nodes is refused.
    fn switch(&mut self, b: usize, spline: Option<usize>, node: i32) -> bool {
        let Some(s) = spline.filter(|&s| s < self.splines.len() && !self.splines[s].nodes.is_empty()) else {
            return false;
        };
        let mut node = if node != 0 { node - 1 } else { node };
        if node < 0 {
            node = self.splines[s].nodes.len() as i32 - 1;
        }
        self.birds[b].spline = s;
        self.birds[b].node = node;
        self.wrap(b);
        true
    }

    /// What a bird does at a node: land at the end of a landing spline, fly on into the spline
    /// this one leads to, or turn off a looping spline down a free landing spline.
    fn transitions(&mut self, b: usize) {
        let s = self.birds[b].spline;
        let node = self.birds[b].node;
        if !self.splines[s].looping {
            let n = self.splines[s].nodes.len() as i32;
            let Some(to) = self.splines[s].to.filter(|_| node >= n - 2) else { return };
            if !self.splines[to].looping && self.birds[b].flying {
                if node < n - 1 {
                    return;
                }
                // The original looks for a `[BirdPerchN]` point here first; no perch on the disc
                // has a landing spline whose file is on it, so none is ported.
                if self.birds[b].mode == 1 {
                    let at = self.splines[s].nodes[node as usize].pos;
                    let look = self.splines[s].perch_look;
                    let bird = &mut self.birds[b];
                    bird.pos = at;
                    bird.target = at;
                    face(bird, look, v3(0.0, 1.0, 0.0));
                }
                self.birds[b].flying = false;
                self.birds[b].mode = 0;
                self.play(b, IDLE);
            } else {
                self.splines[s].occupied = false;
                let was_looping = self.splines[s].looping;
                let to_node = self.splines[s].to_node;
                self.switch(b, Some(to), to_node);
                let now_looping = self.splines[self.birds[b].spline].looping;
                self.play(b, if !was_looping && !now_looping { TAKE_OFF } else { FLY });
            }
        } else {
            let Some(land) = (0..self.splines.len()).find(|&i| {
                let sp = &self.splines[i];
                sp.from == Some(s) && sp.from_node == node && !sp.occupied
            }) else {
                return;
            };
            self.switch(b, Some(land), 0);
            let now = self.birds[b].spline;
            if self.splines[now].looping {
                return;
            }
            self.birds[b].mode = 1;
            self.splines[now].occupied = true;
        }
    }

    /// Toward the target: at the spline's speed, or slowing to a third of it down a landing spline.
    fn fly(&mut self, b: usize, dt: f32) {
        let sp = &self.splines[self.birds[b].spline];
        let (fps, n) = (sp.fps, sp.nodes.len() as i32);
        let bird = &mut self.birds[b];
        let toward = bird.target.sub(bird.pos).norm();
        if bird.mode == 0 {
            bird.pos = bird.pos.add(toward.scale(dt * fps));
            if bird.playing.finished && bird.playing.motion == Some(TAKE_OFF) {
                bird.playing.play(Some(FLY));
            }
        } else {
            let left = (n - bird.node) as f32 / n as f32;
            bird.pos = bird.pos.add(toward.scale((left.min(0.65) + 0.35) * dt * fps));
            if bird.node == n - 3 && bird.playing.motion != Some(LAND) {
                bird.playing.play(Some(LAND));
            }
        }
    }

    /// Every creature to draw, `blend` of the way from the tick before the last to the last.
    pub fn placed(&self, blend: f32) -> Vec<Placed> {
        let back = (1.0 - blend.clamp(0.0, 1.0)) * self.last_dt;
        let mut out = Vec::with_capacity(self.birds.len() + self.toys.len());
        for b in &self.birds {
            let body = &self.bodies[b.body];
            let pos = b.before.add(b.pos.sub(b.before).scale(blend.clamp(0.0, 1.0)));
            let time = if b.playing.time >= back { b.playing.time - back } else { b.playing.time };
            out.push(Placed {
                model: body.model.clone(),
                at: upright(pos, b.forward, b.up),
                palette: body.palette(b.playing.motion, time),
            });
        }
        for t in &self.toys {
            let body = &self.bodies[t.body];
            let p = if t.playing.motion == t.before.motion && t.playing.time >= back { t.playing.time - back } else { t.playing.time };
            out.push(Placed { model: body.model.clone(), at: t.at, palette: body.palette(t.playing.motion, p) });
        }
        out
    }
}

/// Turn a bird to face along `forward` with `up`, unless that is no direction at all.
fn face(bird: &mut Bird, forward: V3, up: V3) {
    if forward.cross(up).len() > 1e-6 {
        bird.forward = forward;
        bird.up = up;
    }
}
