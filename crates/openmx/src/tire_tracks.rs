//! The bike's tire tracks (V40): the strips the original lays behind a wheel on the dirt, as its
//! vehicle particle manager runs the `TireTrack` entries of the track's `mxparticles.dat`.
//!
//! **The files.** Every off-road track's second archive has `tracks\<track>\mxparticles.dat` (the
//! player's; `mxaiparticles.dat` is the AI's copy, the same two entries), read by
//! [`game_render::biplane::emitters`]. Two of its entries are `Type = TireTrack`, `Front Tire Track`
//! on tire 0 and `Rear Tire Track` on tire 1: `Width` 0.26 and 0.275 ft (half the strip's width),
//! `MinimumCompressionThreshold` 0.85 and 0.75, and alike in the rest — `NumberOfStrips` 32,
//! `HeightAboveGround` 0.3, `BreakAngle` 10°, `MinimumSpeedThreshold` 3 ft/s,
//! `MaximumCompressionThreshold` 1, `BasePercentAlpha` 0.4, `FadeInDistance` 1 and
//! `FadeOutDistance` 10 ft, `Disable = NoContact | ObjectContact | Wrecking`, and `ZoneDescriptor =
//! TireTrack`, whose section of `mxparticlezones.dat` turns zone 1 off (`None`): a surface 0 lays
//! nothing. `Material = ttrack` is `bike\tex\ttrack.psm`, one pass of `bike\tex\ttrack.pst` (32 by 64,
//! black, a tread in its alpha, at most 0.37) mixed `(Cs − Cd) × As + Cd` with the alpha test on.
//!
//! **On or off** is the manager's, as for the plane's particles: each tire's flags (`NoContact` when
//! neither the ground ray nor the collision sweep touched it, `GroundContact`, `ObjectContact`) and
//! the wreck's (`Wrecking`, else `NotWrecking`); an entry whose `Disable` meets them is switched off —
//! its strip ended, nothing faded out left, all of the fade-in to go — and back on when it does not.
//! A zone change on its tire to a zone the descriptor turns off does the same and holds it off until
//! the tire is on a zone that is on.
//!
//! **A tick of an entry that is on** ([`Track::step`]): `d` is how far its tire's point moved since
//! last tick (none the tick after its strip ended). The tire *marks* while it spins or is compressed
//! past `MinimumCompressionThreshold`, and the bike goes faster than `MinimumSpeedThreshold`. Marking,
//! it fades in over `FadeInDistance` of travel; not marking, out over `FadeOutDistance`, and quicker
//! as the bike slows under the threshold. Each side's remainder is set from the other's fraction
//! every tick, which is the original's arithmetic and not a symmetric fade: a mark that has faded all
//! the way out leaves no fade-in to do, and the next one starts at full strength. The alpha is that
//! fraction times `BasePercentAlpha`, raised towards 1 by how far the compression is between the two
//! thresholds. Over 0.005 it is laid ([`Strip::add`]); otherwise the strip ends.
//!
//! **A strip** is a row of two points across the tire every so often: a row is laid once the point
//! is 2 ft from the last one laid, 0.3 ft (plus up to 0.005 of noise) up the contact's normal and
//! `Width` either side of the line back to the segment's first row. The texture runs across it and
//! along it by `u`, which grows each tick by half the distance from the last row laid — not since last
//! tick, so a slow wheel stretches it — and a segment ends where the line bends past `BreakAngle` or
//! `u` passes 7: a restart row at the same point, `u` back to −8, a new segment. Every row of a segment
//! carries the alpha the segment started with.
//!
//! **What is kept** is a ring of `NumberOfStrips` batches of 16 points — eight rows — laid round and
//! round; a batch is drawn as one triangle strip, a point flagged as a restart drawing no triangle. A
//! batch begun mid-segment repeats the row before it so the strip carries on. Nothing fades with age:
//! an old mark is there until the ring comes round and overwrites it, a row at a time (32 × 8 rows,
//! a couple of hundred to a few hundred feet of marks a tire).
//!
//! The noise is drawn from the strips' own generator, never the simulation's: the original's calls
//! the C library's `rand`, which would put the picture in the simulation's sequence.

use game_render::biplane::Emitter;
use game_render::glow::{Glow, Vertex};
use game_render::ini;
use game_render::mat::V3;

/// The player's vehicle effects list, beside each off-road track's other files.
pub const LIST: &str = "mxparticles.dat";
/// The zone descriptors' file.
pub const ZONES: &str = "mxparticlezones.dat";
/// Zones a descriptor names, `Zone1` to `Zone16`.
pub const ZONE_COUNT: usize = 16;
/// Points to a batch of the ring.
pub const BATCH: usize = 16;
/// How far the point goes before a row is laid.
const LAY_DISTANCE: f32 = 2.0;
/// `u` a texture's length along: the strip's scale on its distance.
const U_SCALE: f32 = 1.0;
/// `u` at which a segment ends, and minus which the next one starts.
const SEGMENT: f32 = 8.0;
/// The most the height is jittered by.
const JITTER: f32 = 0.005;

/// The manager's flag bits, as the plane's particles have them.
pub struct Disable;

impl Disable {
    pub const NO_CONTACT: u32 = 1;
    pub const GROUND_CONTACT: u32 = 2;
    pub const OBJECT_CONTACT: u32 = 4;
    pub const WRECKING: u32 = 8;
    pub const NOT_WRECKING: u32 = 0x10;

    /// `NoContact | ObjectContact` and the like; anything unknown is no bits.
    pub fn parse(line: &str) -> u32 {
        line.split('|')
            .map(|w| match w.trim().to_ascii_lowercase().as_str() {
                "nocontact" => Disable::NO_CONTACT,
                "groundcontact" => Disable::GROUND_CONTACT,
                "objectcontact" => Disable::OBJECT_CONTACT,
                "wrecking" => Disable::WRECKING,
                "notwrecking" => Disable::NOT_WRECKING,
                _ => 0,
            })
            .fold(0, |a, b| a | b)
    }
}

/// One `TireTrack` entry as the file has it, on the original's own defaults where it says nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct Params {
    pub name: String,
    /// `AttachedToTire`: 0 the front, 1 the rear.
    pub tire: usize,
    pub disable: u32,
    /// `Material`: the `.psm` (and texture) the strips are drawn with.
    pub material: String,
    /// `NumberOfStrips`: batches in the ring.
    pub strips: usize,
    /// `HeightAboveGround`, feet up the contact's normal.
    pub height: f32,
    /// `Width`: feet either side of the line.
    pub width: f32,
    /// `BreakAngle`, radians (the file's degrees).
    pub break_angle: f32,
    /// `MinimumSpeedThreshold`, ft/s.
    pub min_speed: f32,
    /// `MinimumCompressionThreshold` and `MaximumCompressionThreshold`.
    pub min_compression: f32,
    pub max_compression: f32,
    /// `BasePercentAlpha`.
    pub base_alpha: f32,
    /// `FadeInDistance` and `FadeOutDistance`, feet.
    pub fade_in: f32,
    pub fade_out: f32,
    /// Which surfaces lay it: the descriptor's `Zone<n>` not `none`, surface `n − 1`.
    pub zones: [bool; ZONE_COUNT],
}

impl Params {
    /// The entry's constructor's values. The file gives every one on the disc; the strips and the
    /// height, which the constructor leaves unset, are the disc's.
    pub fn defaults(name: &str) -> Params {
        Params {
            name: name.to_string(),
            tire: 0,
            disable: 0,
            material: "ttrack".to_string(),
            strips: 32,
            height: 0.3,
            width: 1.0,
            break_angle: 10f32.to_radians(),
            min_speed: 10.0,
            min_compression: 250.0,
            max_compression: 500.0,
            base_alpha: 0.7,
            fade_in: 5.0,
            fade_out: 5.0,
            zones: [true; ZONE_COUNT],
        }
    }

    /// `e`'s keys over the defaults, its zones out of the zones file's `zones`.
    pub fn read(e: &Emitter, zones: Option<&str>) -> Params {
        let mut p = Params::defaults(&e.name);
        let num = |key: &str, into: &mut f32| {
            if let Some(v) = e.key(key).and_then(|v| v.trim().parse::<f32>().ok()) {
                *into = v;
            }
        };
        num("Width", &mut p.width);
        num("HeightAboveGround", &mut p.height);
        num("MinimumSpeedThreshold", &mut p.min_speed);
        num("MinimumCompressionThreshold", &mut p.min_compression);
        num("MaximumCompressionThreshold", &mut p.max_compression);
        num("BasePercentAlpha", &mut p.base_alpha);
        num("FadeInDistance", &mut p.fade_in);
        num("FadeOutDistance", &mut p.fade_out);
        let mut angle = 10.0;
        num("BreakAngle", &mut angle);
        p.break_angle = angle * 0.017_453_29;
        if let Some(n) = e.key("NumberOfStrips").and_then(|v| v.trim().parse::<usize>().ok()) {
            p.strips = n.max(1);
        }
        if let Some(t) = e.key("AttachedToTire").and_then(|v| v.trim().parse::<usize>().ok()) {
            p.tire = t;
        }
        if let Some(m) = e.key("Material") {
            p.material = m.trim().trim_matches('"').to_string();
        }
        p.disable = Disable::parse(&e.disable);
        let sections = zones.map(ini::parse).unwrap_or_default();
        let descriptor = e.key("ZoneDescriptor").unwrap_or("").trim().trim_matches('"');
        if let Some(s) = sections.iter().find(|s| s.name.eq_ignore_ascii_case(descriptor)) {
            for (n, on) in p.zones.iter_mut().enumerate() {
                let v = s.get(&format!("Zone{}", n + 1)).map(|v| v.trim().trim_matches('"'));
                *on = !v.is_some_and(|v| v.eq_ignore_ascii_case("none"));
            }
        }
        p
    }
}

/// The list's `TireTrack` entries, in its order.
pub fn list(text: &str, zones: Option<&str>) -> Vec<Params> {
    game_render::biplane::emitters(text)
        .iter()
        .filter(|e| e.kind.eq_ignore_ascii_case("TireTrack"))
        .map(|e| Params::read(e, zones))
        .collect()
}

/// A tire as the tracks read it, after the tick's simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tire {
    /// Where the suspension line meets the ground, and the ground's normal there.
    pub at: V3,
    pub normal: V3,
    /// The ground ray found the ground; the collision sweep put a contact on it.
    pub ground: bool,
    pub object: bool,
    /// How far the suspension is compressed.
    pub compression: f32,
    /// The drive asks more than the surface gives.
    pub spinning: bool,
    /// The surface under it.
    pub zone: u8,
}

impl Tire {
    /// The manager's flags for it.
    pub fn flags(&self) -> u32 {
        let mut f = 0;
        if !self.ground && !self.object {
            f |= Disable::NO_CONTACT;
        }
        if self.ground {
            f |= Disable::GROUND_CONTACT;
        }
        if self.object {
            f |= Disable::OBJECT_CONTACT;
        }
        f
    }
}

/// The bike this tick: its tires (front, rear), its speed and its wreck.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bike {
    pub tires: Vec<Tire>,
    /// The chassis' speed, ft/s.
    pub speed: f32,
    pub wrecking: bool,
}

/// One point of the ring.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub at: V3,
    /// Across the strip, 0 or 1, and along it.
    pub s: f32,
    pub t: f32,
    /// The GS's alpha, 0x80 opaque.
    pub alpha: u8,
    /// A restart: no triangle ends here.
    pub restart: bool,
}

/// The ring of batches a strip is written into.
#[derive(Clone, Debug, PartialEq)]
pub struct Ring {
    pub batches: Vec<[Option<Point>; BATCH]>,
    /// The batch being written and the next point in it.
    pub batch: usize,
    pub at: usize,
    /// The batch was begun and nothing written to it yet: its first row is a restart.
    fresh: bool,
}

impl Ring {
    fn new(batches: usize) -> Ring {
        Ring { batches: vec![[None; BATCH]; batches.max(1)], batch: 0, at: 0, fresh: false }
    }

    fn put(&mut self, p: Point) {
        self.batches[self.batch][self.at] = Some(p);
        self.at += 1;
    }

    fn mark(&mut self, at: usize) {
        if let Some(p) = self.batches[self.batch].get_mut(at).and_then(Option::as_mut) {
            p.restart = true;
        }
    }

    /// Every triangle the ring draws, oldest batch first.
    pub fn triangles(&self) -> Vec<[Point; 3]> {
        let n = self.batches.len();
        let mut out = Vec::new();
        for k in 1..=n {
            let b = &self.batches[(self.batch + k) % n];
            for i in 2..BATCH {
                if let (Some(a), Some(c), Some(p)) = (b[i - 2], b[i - 1], b[i])
                    && !p.restart
                {
                    out.push([a, c, p]);
                }
            }
        }
        out
    }
}

/// The strip one entry lays.
#[derive(Clone, Debug, PartialEq)]
pub struct Strip {
    pub ring: Ring,
    pub ended: bool,
    /// Points in the segments laid since the strip began, two a row; 0 until its first row.
    pub count: u32,
    /// Along the texture.
    pub u: f32,
    /// The point of the last row laid, the segment's line's far end and its first row.
    pub last_lay: V3,
    pub prev_laid: V3,
    pub start: V3,
    /// The segment's direction, once it has one.
    pub dir: Option<V3>,
    /// The alpha the segment's rows carry.
    pub alpha: u8,
    /// The row being written, its two points.
    edges: [V3; 2],
    /// The last row handed to the ring, and the one going after it.
    held: [Point; 4],
    /// Rows ever handed to the ring.
    emits: u32,
    rng: Rng,
}

impl Strip {
    pub fn new(batches: usize, seed: u32) -> Strip {
        Strip {
            ring: Ring::new(batches),
            ended: true,
            count: 0,
            u: 0.0,
            last_lay: V3::default(),
            prev_laid: V3::default(),
            start: V3::default(),
            dir: None,
            alpha: 0x80,
            edges: [V3::default(); 2],
            held: [Point::default(); 4],
            emits: 0,
            rng: Rng::new(seed),
        }
    }

    /// Stop laying: the next row laid starts a new strip.
    pub fn end(&mut self) {
        if !self.ended {
            if self.count > 0 {
                self.count = 0;
                self.u = 0.0;
            }
            self.ended = true;
        }
    }

    /// The tire at `at`, the ground facing `normal`, `alpha` strong: a row if it has gone far enough.
    pub fn add(&mut self, width: f32, height: f32, break_angle: f32, at: V3, normal: V3, alpha: f32) {
        let a8 = (alpha.clamp(0.0, 1.0) * 128.0) as u8;
        if self.ended && a8 > 0 {
            (self.prev_laid, self.start, self.ended) = (at, at, false);
        }
        let d = at.sub(self.last_lay).len();
        let h = height + self.rng.unit() * JITTER;
        if at == self.prev_laid {
            return;
        }
        self.u += d / LAY_DISTANCE * U_SCALE;
        if a8 == 0 || (d < LAY_DISTANCE && self.u + U_SCALE < SEGMENT) {
            return;
        }
        self.last_lay = at;
        let side = self.prev_laid.sub(at).norm().cross(normal);
        let up = at.add(normal.scale(h));
        self.edges = [up.sub(side.scale(width)), up.add(side.scale(width))];
        if self.count == 0 {
            self.emit(true);
            self.count += 2;
            (self.prev_laid, self.start, self.dir, self.alpha) = (at, at, None, a8);
            return;
        }
        self.emit(false);
        let along = at.sub(self.start).norm();
        let dir = *self.dir.get_or_insert(along);
        let angle = along.dot(dir).clamp(-1.0, 1.0).acos();
        if angle <= break_angle && self.u + U_SCALE <= SEGMENT {
            return;
        }
        self.count += 2;
        self.u = -SEGMENT;
        self.dir = Some(at.sub(self.prev_laid).norm());
        (self.start, self.prev_laid) = (at, at);
        self.emit(true);
        self.alpha = a8;
    }

    /// The row at the edges into the ring: a restart, or one carrying on from the last.
    fn emit(&mut self, restart: bool) {
        let carry = !restart && self.ring.at == 0 && self.emits != 0;
        let first = if carry { 2 } else { 0 };
        self.emits += 1;
        for (k, (at, s)) in [(self.edges[0], 0.0), (self.edges[1], 1.0)].into_iter().enumerate() {
            self.held[first + k] = Point { at, s, t: self.u, alpha: self.alpha, restart: false };
        }
        let n = if carry { 4 } else { 2 };
        for p in &mut self.held[..2] {
            p.restart = restart || carry;
        }
        if self.ring.at == 0 {
            self.ring.fresh = true;
        }
        let mut left = n;
        let mut k = 0;
        while left != 0 {
            for j in 0..2 {
                let mut p = self.held[k + j];
                if left == n && self.ring.fresh {
                    p.restart = true;
                }
                self.ring.put(p);
            }
            self.ring.fresh = false;
            if self.ring.at + 1 > BATCH - 1 {
                self.ring.at = 0;
                self.ring.batch = (self.ring.batch + 1) % self.ring.batches.len();
                if left != 2 {
                    for j in 0..2 {
                        self.ring.put(Point { restart: true, ..self.held[k + j] });
                    }
                }
            }
            k += 2;
            left -= 2;
        }
        let at = self.ring.at;
        self.ring.mark(at);
        if at + 1 < BATCH {
            self.ring.mark(at + 1);
        }
    }
}

/// One entry and its strip.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub params: Params,
    /// Switched on by the flags, and held off by its zone.
    pub on: bool,
    pub zone_off: bool,
    /// Fade-out and fade-in distance still to go.
    pub out_left: f32,
    pub in_left: f32,
    /// Where its tire was last tick.
    pub last: V3,
    /// The zone it last saw its tire on.
    zone: Option<u8>,
    pub strip: Strip,
}

impl Track {
    pub fn new(params: Params, seed: u32) -> Track {
        let strip = Strip::new(params.strips, seed);
        Track { params, on: true, zone_off: false, out_left: 5.0, in_left: 0.0, last: V3::default(), zone: None, strip }
    }

    fn off(&mut self) {
        self.strip.end();
        self.out_left = 0.0;
        self.in_left = self.params.fade_in;
    }

    /// The manager's part: on or off by `flags` (the tire's and the wreck's), its zone, and a tick
    /// of laying while it is on.
    pub fn step(&mut self, tire: &Tire, flags: u32, speed: f32) {
        if self.params.disable & flags != 0 {
            if self.on {
                self.on = false;
                self.off();
            }
        } else {
            self.on = true;
        }
        if self.zone != Some(tire.zone) {
            self.zone = Some(tire.zone);
            if self.params.zones.get(usize::from(tire.zone)).copied().unwrap_or(true) {
                self.zone_off = false;
            } else {
                self.zone_off = true;
                self.off();
            }
        }
        if self.on && !self.zone_off {
            self.lay(tire, speed);
        }
    }

    /// A tick of an entry that is on.
    fn lay(&mut self, tire: &Tire, speed: f32) {
        let p = &self.params;
        if self.strip.ended {
            self.last = tire.at;
        }
        let d = tire.at.sub(self.last).len();
        let marking = (tire.spinning || tire.compression > p.min_compression) && speed > p.min_speed;
        let fraction = if marking {
            let f = (p.fade_in - self.in_left) / p.fade_in;
            self.in_left = (self.in_left - d).max(0.0);
            self.out_left = f * p.fade_out;
            f
        } else {
            let f = self.out_left / p.fade_out;
            self.out_left = (self.out_left - d).max(0.0);
            if p.min_speed > 1.0 && speed < p.min_speed {
                self.out_left *= speed / p.min_speed;
            }
            self.in_left = f * p.fade_in;
            f
        };
        self.last = tire.at;
        let t = ((tire.compression - p.min_compression) / (p.max_compression - p.min_compression)).min(1.0).max(0.0);
        let alpha = fraction * (p.base_alpha + t * (1.0 - p.base_alpha));
        if alpha > 0.005 {
            let (width, height, angle) = (p.width, p.height, p.break_angle);
            self.strip.add(width, height, angle, tire.at, tire.normal, alpha);
        } else {
            self.strip.end();
        }
    }
}

/// A bike's tracks: its `TireTrack` entries, each on its tire.
#[derive(Clone, Debug, PartialEq)]
pub struct Tracks {
    pub tracks: Vec<Track>,
}

impl Tracks {
    /// `list`'s entries, their noise seeded from `seed` (a rider's own, so two riders differ).
    pub fn new(list: &[Params], seed: u32) -> Tracks {
        let tracks = list.iter().enumerate().map(|(k, p)| Track::new(p.clone(), seed.wrapping_mul(31).wrapping_add(k as u32 + 1))).collect();
        Tracks { tracks }
    }

    /// A tick of the bike.
    pub fn step(&mut self, bike: &Bike) {
        let wreck = if bike.wrecking { Disable::WRECKING } else { Disable::NOT_WRECKING };
        for t in &mut self.tracks {
            let Some(tire) = bike.tires.get(t.params.tire) else { continue };
            t.step(tire, tire.flags() | wreck, bike.speed);
        }
    }

    /// The strips as glows cut from glow texture `texture`: the tread across and along, its alpha
    /// the GS's over 0x80, the colour the texel's own.
    pub fn glows(&self, texture: usize) -> Glow {
        let mut tris = Vec::new();
        for t in &self.tracks {
            for tri in t.strip.ring.triangles() {
                // The texture repeats: take whole lengths off so a long strip keeps its precision.
                let base = tri.iter().map(|p| p.t).fold(f32::INFINITY, f32::min).floor();
                tris.push(tri.map(|p| Vertex {
                    at: [p.at.x, p.at.y, p.at.z],
                    uv: [p.s, p.t - base],
                    tint: [1.0, 1.0, 1.0, f32::from(p.alpha) / 128.0],
                }));
            }
        }
        Glow { texture, additive: false, tris }
    }
}

/// The strips' own noise: an xorshift, never the simulation's `rand`.
#[derive(Clone, Debug, PartialEq)]
struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Rng {
        Rng(seed.max(1))
    }

    fn unit(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}
