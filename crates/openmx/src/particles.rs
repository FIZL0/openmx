//! The biplane's particles (P58): the engine smoke, the exhaust and the wreck's dust, as the original
//! emits them off the disc's own files.
//!
//! **The files.** A freeride track's second archive (`free06_2.res`) carries the plane's emitter list,
//! `tracks\biplaneparticles.dat` ([`game_render::biplane::emitters`]), and the `.emt` each entry names
//! beside it. An `.emt` is the `.psm`'s plain-text format: `[General Info]` (the rates, the life and
//! size ranges, the start velocity and its noise, the fades, the gravity scale, the speed gate),
//! `[Properties]` (`T`/`F` switches) and `[Animation]` ([`Emt`]). The pictures are the track's own
//! particle sheet, `particle\<track>particles.prt` in the track's first archive: one texture
//! (`<track>particles.pst`), `Frame<n>` rectangles in its pixels and named groups of frames, `"Dust 1"`
//! being frames 49 to 64 ([`Atlas`]). An emitter draws a random frame of its group per particle.
//!
//! **An emitter, a frame** (the original's speed emitter, `research/oracle.txt` P58): its point is
//! where its node is this frame, and its speed is how far that point moved since the last frame over
//! the frame's length — so a smoke emitter riding the exhaust is gated by the plane's own speed. Below
//! `MinimumEmissionSpeed` it emits nothing. Otherwise a rate is drawn between the two emit rates, a
//! second's worth of it times the frame's length is added to a carry, and the whole particles in the
//! carry are emitted (at least `MinParticlesPerFrame`), spread along the path from last frame's point
//! to this one. Each takes a random size, life and growth from its ranges, the emitter's velocity
//! turned by the node, a noise point in a ball between the noise bounds turned the same way, and the
//! emitter's own velocity when `ApplyEmitterVelocity` says so. The first frame after an emitter is
//! switched on its last point is its point: it has not moved.
//!
//! **A particle, a frame:** its age grows by the frame in milliseconds and it dies past its life; a
//! growing one grows by its rate and rises by the same; one with a velocity moves by it; drag takes
//! `Drag` of the velocity a second and gravity `GravityMultiplier` of 32.2 ft/s² — in that order.
//! Then, on a frame whose tick count is odd, one that collides meets the terrain under it (P61): an
//! `.emt` with a `GroundLevel` collides, and so does a `CollisionModel` entry's with `HasCollision`
//! (the wreck's chunks). Under the ground and still going down, its velocity is scaled by
//! `Restitution`; within a tenth of a foot of the ground it is put on it and lies there for the rest
//! of its life, and deeper it bounces, its y velocity turned round where it is ([`Particle::land`]).
//!
//! **Wind** is not in the update: an `.emt` with `Wind` adds the particle system's wind to each
//! particle's start velocity, before the noise. The original's track system is built with no wind and
//! nothing sets it, so on the disc's tracks it adds nothing ([`PlaneFx::wind`]).
//!
//! **Which emitters are on** is the vehicle's particle manager's: a flag word per frame, `Wrecking`
//! while the plane is wrecked and `NotWrecking` while it is not, and an emitter is on while none of its
//! `Disable` bits are in it ([`Disable`]). Each type adds its own:
//!
//! * **Generic** (`Smoke`, on `Biplane_Exhaust_L`): nothing more. With `biplanesmoke.emt`'s 35 ft/s gate
//!   it smokes whenever the exhaust moves that fast and the plane is not wrecked — the white trail the
//!   capture shows in the dive.
//! * **Exhaust** (`Left`/`Right Exhaust`): `f = throttle × ½ × (MaxSpeed − airspeed) / MaxSpeed`. At
//!   `f ≤ 0` the emitter is off, and otherwise its velocity's y is the file's times `f + ½`: puffs off
//!   the stacks while the plane runs up slowly, none once it is past `MaxSpeed` or off the throttle.
//! * **CollisionModel** (`Vehicle Dust`, `Vehicle Chunks`): one emission per contact of the plane's
//!   collision model, at the contact, as if it had come from the body's velocity a frame back — the
//!   wreck's dust where the hull hits the ground. Its flags are its own: no contact at all is
//!   `NoContact`, which both entries disable.
//!
//! The tire dust (`Dust`) and the prop wash (`HelicopterDust`) are not ported: the list keeps them and
//! [`PlaneFx`] skips them.
//!
//! **Its own random numbers.** A particle is presentation: nothing here reads or draws from the
//! simulation's `rand`, and nothing the simulation reads is written ([`Rng`]).
//!
//! This file is std and `game_render`'s plain half only, so the tests compile it in by path.

use game_render::biplane::Emitter;
use game_render::ini;
use game_render::mat::{M4, V3, v3};

/// One particle's worth of a gravity scale: the original's `32.2`, feet a second squared.
pub const GRAVITY: f32 = 32.2;

/// The particle sheet's file name in a track's archive: `particle\<track>particles.prt`.
pub fn sheet(track: &str) -> String {
    format!("{track}particles.prt")
}

/// The sheet's texture: `<track>particles.pst`.
pub fn sheet_texture(track: &str) -> String {
    format!("{track}particles.pst")
}

/// An `.emt`: how one emitter makes its particles. Where a file leaves a key out, the original's
/// constructor default stands.
#[derive(Clone, Debug, PartialEq)]
pub struct Emt {
    /// `ParticleGroupName`: the frames of the sheet its particles are drawn from.
    pub group: String,
    /// `Minimum`/`MaximumEmitRate`, particles a second.
    pub rate: (f32, f32),
    /// `Minimum`/`MaximumLife`, milliseconds.
    pub life: (f32, f32),
    /// `Minimum`/`MaximumInitialSize`, feet.
    pub size: (f32, f32),
    /// `Minimum`/`MaximumGrowthRate`, feet a second.
    pub growth: (f32, f32),
    /// `Drag`: the share of the velocity lost a second.
    pub drag: f32,
    /// `MinParticlesPerFrame`.
    pub min_per_frame: f32,
    /// `InitialPercentAge`: how far into its life a particle starts.
    pub initial_age: f32,
    /// `Velocity`, in the emitter's frame.
    pub velocity: V3,
    /// `Minimum`/`MaximumVelocityNoise`.
    pub noise: (V3, V3),
    /// `Minimum`/`MaximumEmitPointExtent`.
    pub extent: (V3, V3),
    /// `FadeInTillPercentAge` and `FadeOutFromPercentAge`, as shares of the life.
    pub fade_in: f32,
    pub fade_out: f32,
    /// `GravityMultiplier`.
    pub gravity: f32,
    /// `MinimumEmissionSpeed`, feet a second.
    pub min_speed: f32,
    /// `GroundLevel` is in the file: its particles meet the ground. The value itself is not what they
    /// meet — the terrain under them is — so only its presence counts. A `CollisionModel` entry's
    /// `HasCollision` switches it on as well ([`PlaneFx::new`]).
    pub collides: bool,
    /// `Restitution`: what share of its velocity a particle keeps when it meets the ground.
    pub restitution: f32,
    /// The `[Properties]` this port acts on.
    pub has_velocity: bool,
    pub grows: bool,
    pub fades: bool,
    pub fades_in: bool,
    pub falls: bool,
    pub has_drag: bool,
    pub apply_emitter_velocity: bool,
    pub mirror_x: bool,
    pub mirror_y: bool,
    /// `Wind`: the particle system's wind is added to each particle's start velocity.
    pub wind: bool,
}

impl Default for Emt {
    /// The original's constructor: what an `.emt` that says nothing gets.
    fn default() -> Emt {
        Emt {
            group: String::new(),
            rate: (0.0, 10.0),
            life: (500.0, 3500.0),
            size: (0.0, 5.0),
            growth: (0.0, 1.0),
            drag: 0.0,
            min_per_frame: 0.0,
            initial_age: 0.0,
            velocity: V3::default(),
            noise: (V3::default(), V3::default()),
            extent: (V3::default(), V3::default()),
            fade_in: 0.25,
            fade_out: 0.7,
            gravity: 3.0,
            min_speed: 0.0,
            collides: false,
            restitution: 0.55,
            has_velocity: false,
            grows: false,
            fades: false,
            fades_in: false,
            falls: false,
            has_drag: false,
            apply_emitter_velocity: false,
            mirror_x: false,
            mirror_y: false,
            wind: false,
        }
    }
}

impl Emt {
    /// Read an `.emt`'s text.
    pub fn parse(text: &str) -> Emt {
        let sections = ini::parse(text);
        let find = |key: &str| sections.iter().find_map(|s| s.get(key));
        let number = |key: &str, or: f32| find(key).and_then(|v| v.trim().parse().ok()).unwrap_or(or);
        let vector = |key: &str, or: V3| {
            find(key)
                .and_then(|v| v.split(',').map(|p| p.trim().parse::<f32>().ok()).collect::<Option<Vec<_>>>())
                .filter(|n| n.len() == 3)
                .map_or(or, |n| v3(n[0], n[1], n[2]))
        };
        let flag = |key: &str| find(key).is_some_and(ini::switch);
        let d = Emt::default();
        Emt {
            group: find("ParticleGroupName").map_or(String::new(), |g| g.trim().trim_matches('"').to_string()),
            rate: (number("MinimumEmitRate", d.rate.0), number("MaximumEmitRate", d.rate.1)),
            life: (number("MinimumLife", d.life.0), number("MaximumLife", d.life.1)),
            size: (number("MinimumInitialSize", d.size.0), number("MaximumInitialSize", d.size.1)),
            growth: (number("MinimumGrowthRate", d.growth.0), number("MaximumGrowthRate", d.growth.1)),
            drag: number("Drag", d.drag),
            min_per_frame: number("MinParticlesPerFrame", d.min_per_frame),
            initial_age: number("InitialPercentAge", d.initial_age),
            velocity: vector("Velocity", d.velocity),
            noise: (vector("MinimumVelocityNoise", d.noise.0), vector("MaximumVelocityNoise", d.noise.1)),
            extent: (vector("MinimumEmitPointExtent", d.extent.0), vector("MaximumEmitPointExtent", d.extent.1)),
            fade_in: number("FadeInTillPercentAge", d.fade_in),
            fade_out: number("FadeOutFromPercentAge", d.fade_out),
            gravity: number("GravityMultiplier", d.gravity),
            min_speed: number("MinimumEmissionSpeed", d.min_speed),
            collides: find("GroundLevel").is_some(),
            restitution: number("Restitution", d.restitution),
            has_velocity: flag("HasVelocity"),
            grows: flag("Grows"),
            fades: flag("Fades"),
            fades_in: flag("FadesIn"),
            falls: flag("Gravity"),
            has_drag: flag("HasDrag"),
            apply_emitter_velocity: flag("ApplyEmitterVelocity"),
            mirror_x: flag("RandomMirrorAboutX"),
            mirror_y: flag("RandomMirrorAboutY"),
            wind: flag("Wind"),
        }
    }
}

/// A track's particle sheet: each frame's rectangle as a share of the texture, and the groups.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Atlas {
    /// `[left, top, right, bottom]`, `0..1` of the texture.
    pub frames: Vec<[f32; 4]>,
    /// Each group's name and its first and last frame.
    pub groups: Vec<(String, usize, usize)>,
}

impl Atlas {
    /// Read a `.prt` for a texture `width` by `height`: the file's rectangles are in its pixels, and
    /// the original divides them by the texture's size the first frame it draws.
    pub fn parse(text: &str, width: usize, height: usize) -> Atlas {
        let sections = ini::parse(text);
        let count = sections.iter().find_map(|s| s.number("NumFrames")).unwrap_or(0.0) as usize;
        let (w, h) = (width.max(1) as f32, height.max(1) as f32);
        let frames = (0..count)
            .map(|i| {
                let name = format!("Frame{i}");
                let s = sections.iter().find(|s| s.name.eq_ignore_ascii_case(&name));
                let n = |k: &str| s.and_then(|s| s.number(k)).unwrap_or(0.0);
                [n("UVLeft") / w, n("UVTop") / h, n("UVRight") / w, n("UVBottom") / h]
            })
            .collect();
        let groups = sections
            .iter()
            .filter(|s| s.name.to_ascii_lowercase().starts_with("group"))
            .filter_map(|s| {
                let name = s.get("Name")?.trim().trim_matches('"').to_string();
                Some((name, s.number("FirstFrame")? as usize, s.number("LastFrame")? as usize))
            })
            .collect();
        Atlas { frames, groups }
    }

    /// A group's frames, first and last, by name ignoring case.
    pub fn group(&self, name: &str) -> Option<(usize, usize)> {
        self.groups.iter().find(|g| g.0.eq_ignore_ascii_case(name)).map(|g| (g.1, g.2))
    }
}

/// The particles' own random numbers: an xorshift, so a particle never draws from the simulation's
/// `rand` and two runs of the same ride make the same smoke.
#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng(seed.max(1))
    }

    /// The next number in `0..1`.
    pub fn unit(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Between `a` and `b`.
    pub fn between(&mut self, (a, b): (f32, f32)) -> f32 {
        a + (b - a) * self.unit()
    }

    /// A point in the ball of radius ½ about the origin.
    fn ball(&mut self) -> V3 {
        loop {
            let p = v3(self.unit() - 0.5, self.unit() - 0.5, self.unit() - 0.5);
            if p.dot(p) <= 0.25 {
                return p;
            }
        }
    }

    /// Between two corners, through the ball: the middle plus the span times a point in it.
    fn spread(&mut self, (lo, hi): (V3, V3)) -> V3 {
        let s = self.ball();
        let mid = lo.add(hi).scale(0.5);
        v3(mid.x + (hi.x - lo.x) * s.x, mid.y + (hi.y - lo.y) * s.y, mid.z + (hi.z - lo.z) * s.z)
    }
}

/// One particle in the air.
#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    /// Which of the emitters made it, which its update and its fades follow.
    pub emitter: usize,
    pub at: V3,
    pub vel: V3,
    /// Feet across, and feet a second it grows by.
    pub size: f32,
    pub growth: f32,
    /// Milliseconds lived, and to live.
    pub age: f32,
    pub life: f32,
    /// The sheet's frame, and whether it is drawn mirrored left to right and top to bottom.
    pub frame: usize,
    pub mirror: (bool, bool),
    /// It met the ground slower than a tenth of a foot under it and lies there: it no longer moves,
    /// drags, falls or meets the ground again. It still ages and grows.
    pub landed: bool,
}

impl Particle {
    /// One frame of `dt` seconds under `emt`. False once it has outlived its life.
    pub fn step(&mut self, emt: &Emt, dt: f32) -> bool {
        self.age += dt * 1000.0;
        if self.age > self.life {
            return false;
        }
        if emt.grows {
            self.size = (self.size + self.growth * dt).max(0.0);
            self.at.y += self.growth * dt;
        }
        if self.landed {
            return true;
        }
        if emt.has_velocity {
            self.at = self.at.add(self.vel.scale(dt));
        }
        if emt.has_drag {
            self.vel = self.vel.sub(self.vel.scale(emt.drag * dt));
        }
        if emt.falls {
            self.vel.y -= emt.gravity * GRAVITY * dt;
        }
        true
    }

    /// Meet the ground at height `ground` under it (P61), after [`Particle::step`] on a frame the
    /// ground is tested: only when it is under the ground and still going down. Its velocity is
    /// scaled by the restitution; then within a tenth of a foot it is put on the ground and lies
    /// there, and deeper it bounces, its y velocity turned round where it is.
    pub fn land(&mut self, emt: &Emt, ground: f32) {
        if self.landed || !emt.collides {
            return;
        }
        let under = self.at.y - ground;
        if under < 0.0 && self.vel.y < 0.0 {
            self.vel = self.vel.scale(emt.restitution);
            if -0.1 < under {
                self.at.y = ground;
                self.landed = true;
            } else {
                self.vel.y = -self.vel.y;
            }
        }
    }

    /// How opaque it is, `0..1`: faded in up to `FadeInTillPercentAge` of its life when the file
    /// says it fades in, and out from `FadeOutFromPercentAge` when it fades. The curve is drawn by
    /// the original's VU1 program, which the port does not read: this is linear, matched by eye.
    pub fn alpha(&self, emt: &Emt) -> f32 {
        let t = (self.age / self.life.max(1.0)).clamp(0.0, 1.0);
        let mut a: f32 = 1.0;
        if emt.fades_in && t < emt.fade_in {
            a = a.min(t / emt.fade_in.max(1e-6));
        }
        if emt.fades && t > emt.fade_out {
            a = a.min((1.0 - t) / (1.0 - emt.fade_out).max(1e-6));
        }
        a.clamp(0.0, 1.0)
    }
}

/// An emitter's state between frames: where it was, and the part of a particle it owes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Source {
    /// Last frame's point; `None` when it was off.
    pub last: Option<V3>,
    pub carry: f32,
}

impl Source {
    /// One frame of an emitter at `at`, on: `turn` is its node's rotation (None for a world-aligned
    /// one), `velocity` the file's velocity after its type has had its say, `moving` the emitter's own
    /// velocity for `ApplyEmitterVelocity`, `wind` the particle system's for `Wind`. Pushes what it
    /// emits onto `out`, made by emitter `index`.
    #[allow(clippy::too_many_arguments)]
    pub fn emit(
        &mut self,
        index: usize,
        emt: &Emt,
        frames: (usize, usize),
        at: V3,
        last: Option<V3>,
        turn: Option<M4>,
        velocity: V3,
        moving: V3,
        wind: V3,
        dt: f32,
        rng: &mut Rng,
        out: &mut Vec<Particle>,
    ) {
        let from = last.or(self.last).unwrap_or(at);
        self.last = Some(at);
        if dt <= 0.0 {
            return;
        }
        let speed = at.sub(from).len() / dt;
        if speed < emt.min_speed {
            return;
        }
        self.carry += rng.between(emt.rate) * dt;
        let whole = self.carry.floor();
        self.carry -= whole;
        let n = (whole.max(emt.min_per_frame.floor())) as usize;
        let turned = |d: V3| turn.map_or(d, |m| m.dir(d));
        for i in 0..n {
            let along = from.add(at.sub(from).scale((i + 1) as f32 / n as f32));
            let mut vel = turned(velocity);
            if emt.apply_emitter_velocity {
                vel = vel.add(moving);
            }
            if emt.wind {
                vel = vel.add(wind);
            }
            let vel = vel.add(turned(rng.spread(emt.noise)));
            let life = rng.between(emt.life);
            let (first, last) = frames;
            let frame = first + ((rng.unit() * (last + 1 - first) as f32) as usize).min(last - first);
            out.push(Particle {
                emitter: index,
                at: along.add(turned(rng.spread(emt.extent))),
                vel,
                size: rng.between(emt.size),
                growth: rng.between(emt.growth),
                age: life * emt.initial_age,
                life,
                frame,
                mirror: (emt.mirror_x && rng.unit() < 0.5, emt.mirror_y && rng.unit() < 0.5),
                landed: false,
            });
        }
    }

    /// Switched off: the next frame on starts from where it is then.
    pub fn off(&mut self) {
        self.last = None;
    }
}

/// The manager's flag bits, and an entry's `Disable` line read into them.
pub struct Disable;

impl Disable {
    pub const NO_CONTACT: u32 = 1;
    pub const GROUND_CONTACT: u32 = 2;
    pub const OBJECT_CONTACT: u32 = 4;
    pub const WRECKING: u32 = 8;
    pub const NOT_WRECKING: u32 = 0x10;

    /// `NoContact | ObjectContact` and the like; `Never` and anything unknown are no bits.
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

/// What one entry of the list does with its emitter.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// On its node, gated only by the flags.
    Generic,
    /// On its node, by the throttle and the airspeed against `MaxSpeed`.
    Exhaust { max_speed: f32 },
    /// At each contact of the collision model.
    CollisionModel,
}

/// One entry the port emits from.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    /// The plane's node it rides on.
    pub node: Option<String>,
    pub disable: u32,
    pub emt: Emt,
    /// Its group's frames in the sheet.
    pub frames: (usize, usize),
    /// One source for a node's entry; one per contact for the collision model's.
    pub sources: Vec<Source>,
}

/// What the plane is doing this frame, as the particles read it.
#[derive(Clone, Debug, Default)]
pub struct PlaneFrame {
    /// Each entry's node, in the world, by the node's name: the plane's placement times the node's
    /// bind transform. An entry whose node is missing is off.
    pub nodes: Vec<(String, M4)>,
    /// The throttle `0..1` and the airspeed in feet a second.
    pub throttle: f32,
    pub airspeed: f32,
    /// The plane is wrecked (its crash reason is not 0).
    pub wrecked: bool,
    /// The body's velocity, feet a second.
    pub vel: V3,
    /// Where the collision model touches the ground and where it touches anything else.
    pub ground_contacts: Vec<V3>,
    pub object_contacts: Vec<V3>,
}

/// One particle as it is drawn: a square facing the camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drawn {
    pub at: V3,
    /// Feet across.
    pub size: f32,
    /// `[left, top, right, bottom]` of the sheet, mirrored already.
    pub uv: [f32; 4],
    pub alpha: f32,
}

/// The plane's emitters and the particles they made.
#[derive(Clone, Debug)]
pub struct PlaneFx {
    pub entries: Vec<Entry>,
    pub atlas: Atlas,
    pub particles: Vec<Particle>,
    /// The list's entries the port does not emit from, by name and type.
    pub skipped: Vec<(String, String)>,
    pub rng: Rng,
    /// The particle system's wind, feet a second, which an emitter with `Wind` adds to what it emits.
    /// The original's track system is built with none and nothing sets it: free06's and the first
    /// recording's track hold 0, 0, 0 in their save states (journal `biplane-particles` P61).
    pub wind: V3,
    /// Frames stepped. The original tests a particle against the ground only on a frame its tick
    /// counter is odd; this counts the fx's own.
    pub tick: u32,
}

impl PlaneFx {
    /// The list's entries the port emits from, each with its `.emt` out of `read` (a file name to its
    /// bytes; the list names them in any case) and its frames out of `atlas`. An entry whose `.emt`
    /// or group is missing is skipped like a type the port has not ported.
    pub fn new(list: &[Emitter], atlas: Atlas, read: impl Fn(&str) -> Option<Vec<u8>>) -> PlaneFx {
        let mut entries = Vec::new();
        let mut skipped = Vec::new();
        for e in list {
            let kind = match e.kind.to_ascii_lowercase().as_str() {
                "generic" => Kind::Generic,
                "exhaust" => Kind::Exhaust { max_speed: e.max_speed },
                "collisionmodel" => Kind::CollisionModel,
                _ => {
                    skipped.push((e.name.clone(), e.kind.clone()));
                    continue;
                }
            };
            let Some(mut emt) = read(&e.emitter).map(|b| Emt::parse(&ini::text(&b))) else {
                skipped.push((e.name.clone(), format!("{}: no {}", e.kind, e.emitter)));
                continue;
            };
            let Some(frames) = atlas.group(&emt.group) else {
                skipped.push((e.name.clone(), format!("{}: no group {:?}", e.kind, emt.group)));
                continue;
            };
            // The collision model's entry turns the ground on for its emitter after reading the `.emt`.
            emt.collides |= kind == Kind::CollisionModel && e.has_collision;
            entries.push(Entry {
                name: e.name.clone(),
                kind,
                node: e.node.clone(),
                disable: Disable::parse(&e.disable),
                emt,
                frames,
                sources: Vec::new(),
            });
        }
        PlaneFx { entries, atlas, particles: Vec::new(), skipped, rng: Rng::new(0x5eed_0058), wind: V3::default(), tick: 0 }
    }

    /// One frame of `dt` seconds over no ground at all: [`PlaneFx::step_over`] with nothing under
    /// anything. The shell always has its terrain to hand; the tests step without one.
    #[allow(dead_code)]
    pub fn step(&mut self, plane: &PlaneFrame, dt: f32) {
        self.step_over(plane, dt, &mut |_| None);
    }

    /// One frame of `dt` seconds: the particles already in the air first, then what each entry emits
    /// this frame, as the original's manager runs them. `ground` is the terrain's height under a point,
    /// `None` where there is none; a particle that collides meets it on odd frames
    /// ([`Particle::land`]).
    pub fn step_over(&mut self, plane: &PlaneFrame, dt: f32, ground: &mut dyn FnMut(V3) -> Option<f32>) {
        self.tick = self.tick.wrapping_add(1);
        let entries = &self.entries;
        let tested = self.tick & 1 == 1;
        self.particles.retain_mut(|p| {
            let emt = &entries[p.emitter].emt;
            if !p.step(emt, dt) {
                return false;
            }
            if tested && emt.collides && !p.landed {
                // No ground under it leaves the point where it is: not under anything.
                let under = ground(p.at).unwrap_or(p.at.y);
                p.land(emt, under);
            }
            true
        });
        let wind = self.wind;
        let global = if plane.wrecked { Disable::WRECKING } else { Disable::NOT_WRECKING };
        for (index, entry) in self.entries.iter_mut().enumerate() {
            match entry.kind {
                Kind::Generic | Kind::Exhaust { .. } => {
                    let node = entry.node.as_ref().and_then(|n| plane.nodes.iter().find(|(name, _)| name.eq_ignore_ascii_case(n)));
                    let mut velocity = entry.emt.velocity;
                    let mut on = entry.disable & global == 0;
                    if let Kind::Exhaust { max_speed } = entry.kind {
                        let f = plane.throttle * 0.5 * (max_speed - plane.airspeed) / max_speed.max(1e-6);
                        on &= f > 0.0;
                        velocity.y *= f + 0.5;
                    }
                    entry.sources.resize_with(1, Source::default);
                    let source = &mut entry.sources[0];
                    match node {
                        Some((_, m)) if on => {
                            let at = m.point(v3(0.0, 0.0, 0.0));
                            source.emit(index, &entry.emt, entry.frames, at, None, Some(*m), velocity, plane.vel, wind, dt, &mut self.rng, &mut self.particles);
                        }
                        _ => source.off(),
                    }
                }
                Kind::CollisionModel => {
                    let mut flags = global;
                    if !plane.ground_contacts.is_empty() {
                        flags |= Disable::GROUND_CONTACT;
                    }
                    if !plane.object_contacts.is_empty() {
                        flags |= Disable::OBJECT_CONTACT;
                    }
                    if flags & (Disable::GROUND_CONTACT | Disable::OBJECT_CONTACT) == 0 {
                        flags |= Disable::NO_CONTACT;
                    }
                    if entry.disable & flags != 0 {
                        entry.sources.clear();
                        continue;
                    }
                    let contacts: Vec<V3> = plane.ground_contacts.iter().chain(&plane.object_contacts).copied().collect();
                    entry.sources.resize_with(contacts.len(), Source::default);
                    for (source, &at) in entry.sources.iter_mut().zip(&contacts) {
                        // As if the point had come from the body's velocity a frame back: its speed is
                        // the body's, and its particles spread along that path.
                        let from = at.sub(plane.vel.scale(dt));
                        source.emit(index, &entry.emt, entry.frames, at, Some(from), None, entry.emt.velocity, plane.vel, wind, dt, &mut self.rng, &mut self.particles);
                    }
                }
            }
        }
    }

    /// The particles as squares to draw, in the order they were made.
    pub fn drawn(&self) -> Vec<Drawn> {
        self.particles
            .iter()
            .filter_map(|p| {
                let emt = &self.entries[p.emitter].emt;
                let [mut l, mut t, mut r, mut b] = *self.atlas.frames.get(p.frame)?;
                if p.mirror.0 {
                    (l, r) = (r, l);
                }
                if p.mirror.1 {
                    (t, b) = (b, t);
                }
                Some(Drawn { at: p.at, size: p.size, uv: [l, t, r, b], alpha: p.alpha(emt) })
            })
            .collect()
    }

    /// How many particles each entry has in the air, by the entry's name.
    pub fn counts(&self) -> Vec<(&str, usize)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| (e.name.as_str(), self.particles.iter().filter(|p| p.emitter == i).count()))
            .collect()
    }
}
