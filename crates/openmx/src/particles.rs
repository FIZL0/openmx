//! The biplane's particles (P58): the engine smoke, the exhaust and the wreck's dust, as the original
//! emits them off the disc's own files — and the bike's (P74), which the same manager runs off the
//! track's `mxparticles.dat`.
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
//! switched on its last point is its point: it has not moved. A node's emitter switched on by the
//! manager emits nothing on that frame — the manager runs after the emitters (P74: the capture's
//! carry stays put the tick the switch byte goes on, and the tick it goes off).
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
//!   `f ≤ 0` the emitter is off, and otherwise its velocity's *z* is the file's times `f + ½`: puffs
//!   off the stacks while the plane runs up slowly, none once it is past `MaxSpeed` or off the
//!   throttle. (P58 scaled y; the bike's capture, P74, shows z scaled and y the file's.)
//! * **CollisionModel** (`Vehicle Dust`, `Vehicle Chunks`): one emission per contact of the plane's
//!   collision model, at the contact, as if it had come from the collision object's relative velocity
//!   at its last resolved contact a frame back (P63; the body's where the caller has none) — the
//!   wreck's dust where the hull hits the ground. Its flags are its own: no contact at all is
//!   `NoContact`, which both entries disable. It is one emitter with one carry, run once for each
//!   contact in turn (P153), so two contacts emit twice the share a frame.
//!
//! * **Dust** (the three `Tire Dust`s, P60): on its node, and on its tire (`AttachedToTire`). The
//!   manager gives each tire flags of its own the same way — `NoContact` with neither a ground nor an
//!   object contact, `GroundContact`, `ObjectContact` — and an entry on a tire is also off while any
//!   of its `Disable` bits are in its tire's. Then the entry's own frame: a ground contact latches it
//!   on and an object contact off (neither leaves the latch as it was); unlatched, or with the strut's
//!   compression `c` not past `MinCompressionThreshold`, it is off. Otherwise its rate is the file's
//!   times `ModifierBase + ModifierScale × c`, and past `MinBurstCompressionThreshold` (with
//!   `UseBurstDust`) it bursts: `BurstModifierScale × c` more on the rate, the growth's low end times
//!   `GrowthMultiplier` (the span stays the file's) and the noise's x bounds times
//!   `VelocityMultiplier` — and the noise's top y becomes its top *z* times it, as the original
//!   writes it. Below the burst the growth and the noise are the file's again ([`DustKeys`]).
//! * **HelicopterDust** (the prop wash under `PropDummy`): the ground under its point, and the height
//!   `h` above it. Under `BeginFadeHeight` it emits at the file's rate, from there to `EndFadeHeight`
//!   at the rate times `1 − (h − Begin) / (End − Begin)`, and above that nothing.
//!
//! * **Spray** (the bike's `Tire Dirt Spray`s and `Tire Chunks`, P74): on its node and its tire. While
//!   the throttle is open, the clutch out and the tire spinning — or its side force is past
//!   `SideForceThreshold` either way — it throws: its velocity's x is `−SideForceScale × side`, the
//!   noise's x bounds widen by half of that each way, the noise's y bounds rise by
//!   `f × Min/MaxVelocityNoiseYScale` and its rate is the file's times `1 + ½f`, with
//!   `f = 1 − (speed + 1) × VelocityNoiseYRamp` held to `0..1`; and a quarter-second clock is wound.
//!   Otherwise the clock runs down, and while it has time left the emitter runs on at the file's rate
//!   times the time left over the quarter second ([`SprayKeys`]).
//! * **ParticleCollisionConstraint** (`Rider Dust`, `Rider Chunks`, `Foot Dust`, P74): one emitter,
//!   switched by the bike's flags alone, emitted by hand at each of the rider's collision spheres whose
//!   bit `AttachToParticleCollisionConstraints` has and whose own contact flags the entry allows, as if
//!   the point had come from its bone's velocity a frame back ([`ConstraintFrame`]).
//!
//! Both of those switch their emitter on themselves, and as the original's emitter does, the first
//! point it is run at after going on only becomes where it last was: it emits from the next (P153).
//!
//! The manager runs an entry only while its flag byte is exactly *live* ([`Entry::flag`]): none of its
//! `Disable` bits in the flags, and — for an entry on a tire — the tire's surface not a `none` zone.
//!
//! **Zones.** `ZoneDescriptor` names a section of `mxparticlezones.dat` ([`ZONES`]): `Zone1` to
//! `Zone16` a group of the sheet for each surface under the tire (or the prop), `none` for nothing,
//! the `.emt`'s own group where a zone is not named. The disc's file names none, so every zone is the
//! `.emt`'s group. A `none` turns the prop wash off; the tire dust's own frame turns its emitter back
//! on regardless, as the original's does.
//!
//! **MatchTerrainContour**: a particle of such an `.emt` is put on the ground under where it would
//! have started, raised by its emit point's extent's height — the prop wash's dust starts one to
//! three feet off the ground, wherever the plane is.
//!
//! **Its own random numbers.** A particle is presentation: nothing here reads or draws from the
//! simulation's `rand`, and nothing the simulation reads is written ([`Rng`]).
//!
//! This file is std and `game_render`'s plain half only, so the tests compile it in by path.

use game_render::biplane::Emitter;
use game_render::ini;
use game_render::mat::{M4, V3, v3};

/// The zones file: which group of the sheet each surface's dust is drawn from, by `ZoneDescriptor`.
/// The original looks in the track's own folder first and then in `Tracks\`; the archives' suffix
/// search finds whichever the track carries.
pub const ZONES: &str = "mxparticlezones.dat";

/// The bike's emitter list in a track's second archive, `tracks\<track>\mxparticles.dat` (P74). The
/// computer's riders read `mxaiparticles.dat` beside it, which on free09 is the same file.
pub const BIKE_PARTICLES: &str = "mxparticles.dat";

/// How many surface zones a zone descriptor names.
pub const ZONE_COUNT: usize = 16;

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
    /// `ParticlesToDistanceRatio` and `ParticlesToSpeedRatio` (P153): where either is above 0 and the
    /// emitter moves faster than `MinimumMovementSpeed`, a frame's share of particles is the distance
    /// it moved and its speed by these, held between the rate's two ends, in place of a random rate.
    pub distance_ratio: f32,
    pub speed_ratio: f32,
    pub min_movement_speed: f32,
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
    /// `MatchTerrainContour`: each particle starts on the ground under it, raised by its extent.
    pub matches_terrain: bool,
    /// `EmitMode` and its timers (V71), as [`Gate`] runs them: 0 on while switched on, 1 a one-shot,
    /// 2 on and off for random times, 3 on and off for fixed ones.
    pub mode: i32,
    /// `DelayTime`: seconds switched on before it starts.
    pub delay: f32,
    /// The four times the mode's keys fill, as the read stores them: the least and most time on,
    /// the least and most time off. Mode 1's `OneShotTime` is the most time on; mode 2's are the
    /// `RandomMin`/`MaxTimeOn`/`Off`; mode 3's `TimedTimeOn` and `TimedTimeOff` the two most.
    /// What a mode does not read stays 0.
    pub times: [f32; 4],
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
            distance_ratio: 0.0,
            speed_ratio: 0.0,
            min_movement_speed: 0.0,
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
            matches_terrain: false,
            mode: 0,
            delay: 0.0,
            times: [0.0; 4],
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
            distance_ratio: number("ParticlesToDistanceRatio", d.distance_ratio),
            speed_ratio: number("ParticlesToSpeedRatio", d.speed_ratio),
            min_movement_speed: number("MinimumMovementSpeed", d.min_movement_speed),
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
            matches_terrain: flag("MatchTerrainContour"),
            mode: find("EmitMode").and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(d.mode),
            delay: number("DelayTime", d.delay),
            times: {
                // Only the emitter's own mode's keys are read, each 0 where the file leaves it out.
                let mode = find("EmitMode").and_then(|v| v.trim().parse::<i32>().ok()).unwrap_or(d.mode);
                let n = |key: &str| number(key, 0.0);
                match mode {
                    1 => [0.0, n("OneShotTime"), 0.0, 0.0],
                    2 => [n("RandomMinTimeOn"), n("RandomMaxTimeOn"), n("RandomMinTimeOff"), n("RandomMaxTimeOff")],
                    3 => [0.0, n("TimedTimeOn"), 0.0, n("TimedTimeOff")],
                    _ => [0.0; 4],
                }
            },
        }
    }
}

/// An emitter's switch and timers (V71): the original's emit gate (`research/oracle.txt` § V71),
/// which every frame says whether the emitter emits, before the speed gate of [`Source::emit_over`]
/// has its say. Its arithmetic is the PS2's ([`ps2_float::fpu`]).
///
/// **On and off.** Switching on an emitter that is off sets it on and just switched on, and its idle
/// time to 0; switching off clears on and emitting, and the idle time. Neither touches the timers.
///
/// **A frame**, in this order: just switched on, it only takes its point (that flag cleared) and
/// emits nothing. Then while its delay is left it emits nothing, the delay running down only while
/// it is on. Then by its mode — off, it emits nothing and its idle time grows, in every mode:
/// - 1, a one-shot: while time is left it runs down and it emits (the frame it runs out too); then,
///   once it has emitted a particle in this shot, it is switched off; if it never did, it carries
///   on as mode 0.
/// - 2 and 3: on with time left, the time runs down; while some is left it emits, and when it runs
///   out the time off is drawn (2: the least time off and a random share up to the most, from the
///   game's `rand` scaled by 2⁻³¹; 3: the time off) and it is switched off. Off with time left, the
///   same, to the time on and switched on — and that frame it emits. With no time left at all it is
///   mode 0.
/// - 0: on, it emits and is emitting.
///
/// **A reset** (the emitter toy's reset and the trigger's `particles` action): the delay back to
/// `DelayTime`, the time to the most time on (mode 2: drawn as above), and a one-shot's "it emitted"
/// cleared. Reading the `.emt` does the same.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gate {
    pub mode: i32,
    pub delay_time: f32,
    /// The delay left, and the time left on or off.
    pub delay: f32,
    pub left: f32,
    /// [`Emt::times`].
    pub times: [f32; 4],
    /// Seconds off since it was last switched.
    pub idle: f32,
    /// [`Gate::SHOT`], [`Gate::JUST_ON`], [`Gate::ON`], [`Gate::EMITTING`].
    pub flags: u8,
}

impl Gate {
    /// A one-shot has emitted in this shot.
    pub const SHOT: u8 = 1;
    pub const JUST_ON: u8 = 2;
    pub const ON: u8 = 4;
    pub const EMITTING: u8 = 8;

    /// The gate of an `.emt` as it is read: off, its timers reset. `rand` is the game's `rand`, a
    /// number in `0..2³¹`.
    pub fn new(emt: &Emt, rand: &mut dyn FnMut() -> i32) -> Gate {
        let mut g = Gate { mode: emt.mode, delay_time: emt.delay, times: emt.times, ..Gate::default() };
        g.reset(rand);
        g
    }

    /// `min + (max − min) · rand · 2⁻³¹`.
    fn draw(min: f32, max: f32, rand: &mut dyn FnMut() -> i32) -> f32 {
        use ps2_float::fpu::{add, from_i32, mul, sub};
        let share = mul(f32::from_bits(0x3000_0000), from_i32(rand()));
        add(min, mul(sub(max, min), share))
    }

    pub fn reset(&mut self, rand: &mut dyn FnMut() -> i32) {
        self.delay = self.delay_time;
        match self.mode {
            1 => {
                self.left = self.times[1];
                self.flags &= !Gate::SHOT;
            }
            2 => self.left = Gate::draw(self.times[0], self.times[1], rand),
            3 => self.left = self.times[1],
            _ => {}
        }
    }

    pub fn on(&mut self) {
        if self.flags & Gate::ON == 0 {
            self.flags |= Gate::ON;
            self.idle = 0.0;
            self.flags |= Gate::JUST_ON;
        }
    }

    pub fn off(&mut self) {
        self.flags &= !(Gate::ON | Gate::EMITTING);
        self.idle = 0.0;
    }

    pub fn is_on(&self) -> bool {
        self.flags & Gate::ON != 0
    }

    /// Off for the frame: idle, not emitting.
    fn rest(&mut self, dt: f32) -> bool {
        self.idle = ps2_float::fpu::add(self.idle, dt);
        self.flags &= !Gate::EMITTING;
        false
    }

    /// One frame of `dt` seconds: whether it emits.
    pub fn pass(&mut self, dt: f32, rand: &mut dyn FnMut() -> i32) -> bool {
        use ps2_float::fpu::sub;
        if self.flags & Gate::JUST_ON != 0 {
            self.flags &= !Gate::JUST_ON;
            return false;
        }
        if !(self.delay <= 0.0) {
            if self.is_on() {
                self.delay = sub(self.delay, dt);
            }
            return false;
        }
        let on = self.is_on();
        match self.mode {
            1 if !on => return self.rest(dt),
            1 if !(self.left <= 0.0) => {
                self.left = sub(self.left, dt);
                return true;
            }
            1 if self.flags & Gate::SHOT != 0 => {
                self.off();
                return false;
            }
            2 | 3 if !(self.left <= 0.0) => {
                self.left = sub(self.left, dt);
                let out = self.left <= 0.0;
                return match (on, out) {
                    (_, false) if on => true,
                    (_, false) => self.rest(dt),
                    (true, true) => {
                        self.left = if self.mode == 2 { Gate::draw(self.times[2], self.times[3], rand) } else { self.times[3] };
                        self.off();
                        false
                    }
                    (false, true) => {
                        self.left = if self.mode == 2 { Gate::draw(self.times[0], self.times[1], rand) } else { self.times[1] };
                        self.on();
                        self.idle = 0.0;
                        true
                    }
                };
            }
            _ => {}
        }
        if on {
            self.flags |= Gate::EMITTING;
            true
        } else {
            self.rest(dt)
        }
    }

    /// What the frame's emit made: a one-shot that made a particle has had its shot.
    pub fn emitted(&mut self, made: usize) {
        if made > 0 && self.mode == 1 {
            self.flags |= Gate::SHOT;
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

    /// A number in `0..2³¹`, as the game's `rand` draws them, for a [`Gate`]'s timers.
    pub fn rand(&mut self) -> i32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 1) as i32
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
    /// Where the picture has it on the tick before (P162): the last tick's `at`, and for one born
    /// this tick its birth point carried back by its emitter's travel over the tick, so a picture
    /// blended between the two ticks draws it where the blended bike draws its emitter.
    pub was: V3,
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
        self.emit_over(index, emt, frames, at, last, turn, velocity, moving, wind, dt, rng, out, &mut |_| None);
    }

    /// [`Source::emit`] over a ground: `ground` is the height of the ground under a point, `None`
    /// where there is none, which a `MatchTerrainContour` emitter puts its particles on.
    #[allow(clippy::too_many_arguments)]
    pub fn emit_over(
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
        ground: &mut dyn FnMut(V3) -> Option<f32>,
    ) {
        let from = last.or(self.last).unwrap_or(at);
        self.last = Some(at);
        if dt <= 0.0 {
            return;
        }
        let distance = at.sub(from).len();
        let speed = distance / dt;
        if speed < emt.min_speed {
            return;
        }
        // The frame's share (P153): by the distance and speed where the file asks for that and the
        // emitter moves, held between the rate's ends, else a random rate between them.
        let (low, high) = (emt.rate.0 * dt, emt.rate.1 * dt);
        let share = if emt.min_movement_speed < speed && (emt.distance_ratio > 0.0 || emt.speed_ratio > 0.0) {
            (emt.distance_ratio * distance + emt.speed_ratio * speed).max(low).min(high)
        } else {
            rng.between(emt.rate) * dt
        };
        // Whole particles leave only once the carry is past one: exactly one stays carried.
        self.carry += share;
        let mut whole = 0.0;
        if self.carry > 1.0 {
            whole = self.carry.trunc();
            self.carry -= whole;
        }
        let n = (whole.max(emt.min_per_frame.trunc())) as usize;
        let turned = |d: V3| turn.map_or(d, |m| m.dir(d));
        let travel = at.sub(from);
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
            let extent = turned(rng.spread(emt.extent));
            let mut at = along.add(extent);
            if emt.matches_terrain && let Some(y) = ground(at) {
                at.y = y + extent.y;
            }
            out.push(Particle {
                emitter: index,
                at,
                was: at.sub(travel),
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
    /// On its node, by its tire's contact and compression.
    Dust(DustKeys),
    /// On its node, by its height over the ground.
    HelicopterDust { begin: f32, end: f32 },
    /// On its node, by its tire's spin and side force and the throttle, faded out over a quarter
    /// second when they stop (P74: the bike's roost and chunks).
    Spray(SprayKeys),
    /// At each of the rider's collision spheres whose bit `mask` has (P74: the wreck's and the
    /// feet's dust; the feet's off the riding body too, P152).
    Constraint { mask: u32 },
}

/// A spray's own keys, over the original's constructor defaults (P74).
#[derive(Clone, Debug, PartialEq)]
pub struct SprayKeys {
    /// `SideForceScale`: the tire's side force times this, negated, is the particles' sideways
    /// velocity.
    pub side_scale: f32,
    /// `SideForceThreshold`: a side force past this sprays with the throttle shut.
    pub side_threshold: f32,
    /// `MinVelocityNoiseYScale`, `MaxVelocityNoiseYScale`: added to the noise's low and high y.
    pub min_y: f32,
    pub max_y: f32,
    /// `VelocityNoiseYRamp`: how fast the speed takes the y noise and the extra rate away.
    pub ramp: f32,
}

impl Default for SprayKeys {
    fn default() -> SprayKeys {
        SprayKeys { side_scale: 10.0, side_threshold: 3.0, min_y: 1.0, max_y: 2.0, ramp: 0.0 }
    }
}

impl SprayKeys {
    /// The keys off a list entry.
    pub fn read(e: &Emitter) -> SprayKeys {
        let d = SprayKeys::default();
        let number = |key: &str, or: f32| e.key(key).and_then(|v| v.trim().parse().ok()).unwrap_or(or);
        SprayKeys {
            side_scale: number("SideForceScale", d.side_scale),
            side_threshold: number("SideForceThreshold", d.side_threshold),
            min_y: number("MinVelocityNoiseYScale", d.min_y),
            max_y: number("MaxVelocityNoiseYScale", d.max_y),
            ramp: number("VelocityNoiseYRamp", d.ramp),
        }
    }

    /// The `.emt` a frame sprays with, off the file's `base`: sideways at `side`'s force, the speed
    /// `speed` taking the extra off as `ramp` says.
    pub fn apply(&self, base: &Emt, side: f32, speed: f32, emt: &mut Emt) {
        let vx = -self.side_scale * side;
        emt.velocity.x = vx;
        let f = (1.0 - (speed + 1.0) * self.ramp).clamp(0.0, 1.0);
        let (lo, hi) = base.noise;
        emt.noise = (v3(lo.x - vx.abs() * 0.5, lo.y + f * self.min_y, lo.z), v3(hi.x + vx.abs() * 0.5, hi.y + f * self.max_y, hi.z));
        emt.rate = (base.rate.0 + base.rate.0 * f * 0.5, base.rate.1 + base.rate.1 * f * 0.5);
    }
}

/// How long a spray runs on once its tire stops throwing, seconds.
pub const SPRAY_FADE: f32 = 0.25;

/// One of the rider's collision spheres as the manager reads it (P74).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConstraintFrame {
    /// It touches the ground, and something else.
    pub ground: bool,
    pub object: bool,
    /// Where it touches, and its bone's velocity.
    pub at: V3,
    pub vel: V3,
    /// The surface it touches.
    pub zone: u8,
}

impl ConstraintFrame {
    /// The manager's flags for it.
    pub fn flags(&self) -> u32 {
        TireFrame { ground: self.ground, object: self.object, ..TireFrame::default() }.flags()
    }
}

/// A tire dust's own keys, over the original's constructor defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct DustKeys {
    /// `MinCompressionThreshold`: the strut must be compressed past this to raise dust.
    pub min_compression: f32,
    /// `MinBurstCompressionThreshold` and `UseBurstDust`: past this it bursts.
    pub burst_compression: f32,
    pub burst: bool,
    /// `GrowthMultiplier`, `VelocityMultiplier`: the burst's growth and noise.
    pub growth: f32,
    pub velocity: f32,
    /// `ModifierBase`, `ModifierScale`, `BurstModifierScale`: the rate's factor on the compression.
    pub base: f32,
    pub scale: f32,
    pub burst_scale: f32,
    /// `EmitterOffset`, in the node's frame.
    pub offset: V3,
}

impl Default for DustKeys {
    fn default() -> DustKeys {
        DustKeys {
            min_compression: 0.3,
            burst_compression: 0.85,
            burst: true,
            growth: 2.0,
            velocity: 5.0,
            base: 0.5,
            scale: 5.0,
            burst_scale: 5.0,
            offset: V3::default(),
        }
    }
}

impl DustKeys {
    /// The keys off a list entry.
    pub fn read(e: &Emitter) -> DustKeys {
        let d = DustKeys::default();
        let number = |key: &str, or: f32| e.key(key).and_then(|v| v.trim().parse().ok()).unwrap_or(or);
        let offset = e
            .key("EmitterOffset")
            .and_then(|v| v.split(',').map(|p| p.trim().parse::<f32>().ok()).collect::<Option<Vec<_>>>())
            .filter(|n| n.len() == 3)
            .map_or(d.offset, |n| v3(n[0], n[1], n[2]));
        DustKeys {
            min_compression: number("MinCompressionThreshold", d.min_compression),
            burst_compression: number("MinBurstCompressionThreshold", d.burst_compression),
            burst: e.key("UseBurstDust").and_then(|v| v.trim().parse::<i32>().ok()).map_or(d.burst, |v| v != 0),
            growth: number("GrowthMultiplier", d.growth),
            velocity: number("VelocityMultiplier", d.velocity),
            base: number("ModifierBase", d.base),
            scale: number("ModifierScale", d.scale),
            burst_scale: number("BurstModifierScale", d.burst_scale),
            offset,
        }
    }

    /// The `.emt` a frame emits with, off the file's `base`, at compression `c` (past the threshold).
    pub fn apply(&self, base: &Emt, c: f32, emt: &mut Emt) {
        let mut k = self.base + self.scale * c;
        if self.burst {
            if self.burst_compression < c {
                k = k + self.burst_scale * c;
                // The low end is scaled and the span is not recomputed: the file's span stands.
                let low = base.growth.0 * self.growth;
                emt.growth = (low, low + (base.growth.1 - base.growth.0));
                let (lo, hi) = base.noise;
                emt.noise = (
                    v3(lo.x * self.velocity, lo.y, lo.z),
                    // The top y is the file's top z scaled, as the original writes it.
                    v3(hi.x * self.velocity, hi.z * self.velocity, hi.z),
                );
            } else {
                emt.growth = base.growth;
                emt.noise = base.noise;
            }
        }
        emt.rate = (base.rate.0 * k, base.rate.1 * k);
    }
}

/// What a zone of a zone descriptor draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Zone {
    /// A group of the sheet, its first and last frame.
    Group(usize, usize),
    /// `none`: nothing.
    Off,
}

/// A zone descriptor's sixteen zones out of the zones file's `text`, each `Zone<n>` read against
/// `atlas`; a zone not named is `own`, the `.emt`'s group. A group the sheet does not have is the
/// sheet's first group, as the original's lookup answers 0 for it.
pub fn zones(text: Option<&str>, descriptor: &str, atlas: &Atlas, own: (usize, usize)) -> Vec<Zone> {
    let sections = text.map(ini::parse).unwrap_or_default();
    let section = sections.iter().find(|s| s.name.eq_ignore_ascii_case(descriptor));
    (1..=ZONE_COUNT)
        .map(|n| match section.and_then(|s| s.get(&format!("Zone{n}"))).map(|v| v.trim().trim_matches('"')) {
            None => Zone::Group(own.0, own.1),
            Some(v) if v.eq_ignore_ascii_case("none") => Zone::Off,
            Some(v) => {
                let (a, b) = atlas.group(v).or_else(|| atlas.groups.first().map(|g| (g.1, g.2))).unwrap_or(own);
                Zone::Group(a, b)
            }
        })
        .collect()
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
    /// Its group's frames in the sheet: the `.emt`'s, or its zone's.
    pub frames: (usize, usize),
    /// One source for a node's entry; one per contact for the collision model's.
    pub sources: Vec<Source>,
    /// `AttachedToTire`: the tire whose flags it also answers to.
    pub tire: Option<usize>,
    /// Its descriptor's zones ([`zones`]), and the `.emt` as the file wrote it, which a tire dust
    /// scales afresh every frame.
    pub zones: Vec<Zone>,
    pub file: Emt,
    /// A tire dust's latch: on from a ground contact until an object contact or switched off.
    pub latched: bool,
    /// The manager's flag byte for it (P74): [`Entry::LIVE`] while none of its `Disable` bits are
    /// set, [`Entry::ZONE_OFF`] while its tire is on a surface its zones say `none` to. It is run
    /// only while it is exactly `LIVE`.
    pub flag: u8,
    /// Its emitter was on after the last frame: an emitter switched on emits from the frame after.
    pub lit: bool,
    /// A spray's fade: a quarter second when its tire throws, run down while it does not.
    pub clock: f32,
    /// Its `.emt`'s switch and timers (V71). Only an entry `held` reads it: a vehicle's entries
    /// are switched by the manager's flags and every `.emt` they name on the disc is mode 0 with no
    /// delay, for which the gate is `lit` (the frame after it goes on is its first).
    pub gate: Gate,
    /// Switched by its toy, not by the flags (V71: a track's emitter): `gate` says when it emits.
    pub held: bool,
}

impl Entry {
    pub const LIVE: u8 = 1;
    pub const ZONE_OFF: u8 = 2;
}

/// A tire as the manager reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TireFrame {
    /// It touches the ground, and something else.
    pub ground: bool,
    pub object: bool,
    /// How far its strut is compressed, feet.
    pub compression: f32,
    /// The surface under it.
    pub zone: u8,
    /// It spins against the ground (P74: the slip's `spinning`), and its side force.
    pub spinning: bool,
    pub side: f32,
}

impl TireFrame {
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

/// What the plane is doing this frame, as the particles read it.
#[derive(Clone, Debug, Default)]
pub struct PlaneFrame {
    /// Each entry's node, in the world, by the node's name: the plane's placement times the node's
    /// bind transform. An entry whose node is missing is off.
    pub nodes: Vec<(String, M4)>,
    /// The throttle `0..1` and the airspeed in feet a second (a bike's: its chassis' speed).
    pub throttle: f32,
    pub airspeed: f32,
    /// A bike's clutch `0..1` (P74): pulled in, its sprays stop throwing.
    pub clutch: f32,
    /// The plane is wrecked (its crash reason is not 0).
    pub wrecked: bool,
    /// The body's velocity, feet a second.
    pub vel: V3,
    /// Where the collision model touches the ground and where it touches anything else.
    pub ground_contacts: Vec<V3>,
    pub object_contacts: Vec<V3>,
    /// The velocity a `CollisionModel` entry's points come from (P63): the collision object's
    /// relative velocity at the last contact it resolved, which is what the original's entry reads.
    /// `None` uses [`PlaneFrame::vel`].
    pub contact_vel: Option<V3>,
    /// The surface under each ground contact, where it is known (P74): a contact on a surface the
    /// entry's zones say `none` to emits nothing. Empty for the plane: every contact draws.
    pub ground_zones: Vec<u8>,
    /// The rider's collision spheres, by the bit of a `Constraint` entry's mask (P74).
    pub constraints: Vec<ConstraintFrame>,
    /// The three tires, front left, front right and tail, by `AttachedToTire`'s index.
    pub tires: Vec<TireFrame>,
}

impl PlaneFrame {
    /// A vehicle's collision object as its `CollisionModel` entries read it (P63): every contact of
    /// the frame's list, as the ground's when the search found the ground and as an object's when it
    /// found an object (the manager's two flags; the original emits once per contact of the one list),
    /// each contact's surface, and `relative`, the object's relative velocity at the last contact it
    /// resolved, which the entries' points come from.
    pub fn touch(&mut self, record: &game_sim::contact_search::Record, relative: ps2_float::Vf) {
        let points: Vec<V3> = record.contacts.iter().map(|c| v3(c.point.x(), c.point.y(), c.point.z())).collect();
        self.ground_contacts.clear();
        self.ground_zones.clear();
        self.object_contacts.clear();
        if record.on_ground {
            self.ground_zones = record.contacts.iter().map(|c| c.surface.clamp(0, 255) as u8).collect();
            self.ground_contacts = points.clone();
        }
        if record.on_object {
            // Both at once: the list is already the ground's, and one point raises the object's flag
            // (every list on the disc is off on an object contact, so it never emits).
            self.object_contacts = if record.on_ground { points.into_iter().take(1).collect() } else { points };
        }
        self.contact_vel = Some(v3(relative.x(), relative.y(), relative.z()));
    }
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
        let zone_text = read(ZONES).map(|b| ini::text(&b));
        for e in list {
            let number = |key: &str, or: f32| e.key(key).and_then(|v| v.trim().parse().ok()).unwrap_or(or);
            let kind = match e.kind.to_ascii_lowercase().as_str() {
                "generic" => Kind::Generic,
                "exhaust" => Kind::Exhaust { max_speed: e.max_speed },
                "collisionmodel" => Kind::CollisionModel,
                "dust" => Kind::Dust(DustKeys::read(e)),
                // The constructor's 0 and 50 where the entry does not say.
                "helicopterdust" => Kind::HelicopterDust { begin: number("BeginFadeHeight", 0.0), end: number("EndFadeHeight", 50.0) },
                "spray" => Kind::Spray(SprayKeys::read(e)),
                "particlecollisionconstraint" => Kind::Constraint {
                    mask: e
                        .key("AttachToParticleCollisionConstraints")
                        .map(str::trim)
                        .and_then(|v| match v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
                            Some(hex) => u32::from_str_radix(hex, 16).ok(),
                            None => v.parse().ok(),
                        })
                        .unwrap_or(0),
                },
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
            // So do a spray's and a sphere's that say `HasCollision` (P74: the tire's collision
            // chunks, the rider's wreck chunks).
            emt.collides |= matches!(kind, Kind::Spray(_) | Kind::Constraint { .. }) && e.has_collision;
            let descriptor = e.key("ZoneDescriptor").unwrap_or("Default");
            entries.push(Entry {
                name: e.name.clone(),
                kind,
                node: e.node.clone(),
                disable: Disable::parse(&e.disable),
                file: emt.clone(),
                emt,
                frames,
                sources: Vec::new(),
                tire: e.key("AttachedToTire").and_then(|v| v.trim().parse().ok()),
                zones: zones(zone_text.as_deref(), descriptor, &atlas, frames),
                latched: false,
                flag: 0,
                lit: false,
                clock: 0.0,
                gate: Gate::default(),
                held: false,
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
    /// this frame, as the original's manager runs them. `ground` answers the terrain's height under a
    /// point and its surface zone, `None` where there is none; a particle that collides meets it on
    /// odd frames ([`Particle::land`]), and the prop wash, which asks for it, is off without it.
    pub fn step_over(&mut self, plane: &PlaneFrame, dt: f32, ground: &mut dyn FnMut(V3) -> Option<(f32, u8)>) {
        self.tick = self.tick.wrapping_add(1);
        let entries = &self.entries;
        let tested = self.tick & 1 == 1;
        self.particles.retain_mut(|p| {
            let emt = &entries[p.emitter].emt;
            p.was = p.at;
            if !p.step(emt, dt) {
                return false;
            }
            if tested && emt.collides && !p.landed {
                // No ground under it leaves the point where it is: not under anything.
                let under = ground(p.at).map_or(p.at.y, |(y, _)| y);
                p.land(emt, under);
            }
            true
        });
        let wind = self.wind;
        let global = if plane.wrecked { Disable::WRECKING } else { Disable::NOT_WRECKING };
        for (index, entry) in self.entries.iter_mut().enumerate() {
            // An entry on a tire answers to the tire's flags as well as the plane's.
            let tire = entry.tire.and_then(|t| plane.tires.get(t)).copied();
            let tire_off = tire.is_some_and(|t| entry.disable & t.flags() != 0);
            // The manager's flag byte (P74): live on the flags alone, and zone-off on a tire whose
            // surface its zones say `none` to — which also picks the tire's group.
            let zone = tire.and_then(|t| entry.zones.get(usize::from(t.zone))).copied();
            if let Some(Zone::Group(a, b)) = zone {
                entry.frames = (a, b);
            }
            entry.flag = if entry.disable & global == 0 && !tire_off { Entry::LIVE } else { 0 };
            if zone == Some(Zone::Off) {
                entry.flag |= Entry::ZONE_OFF;
            }
            match &entry.kind {
                Kind::Dust(keys) => {
                    let node = entry.node.as_ref().and_then(|n| plane.nodes.iter().find(|(name, _)| name.eq_ignore_ascii_case(n)));
                    entry.sources.resize_with(1, Source::default);
                    let (Some(t), Some((_, m)), false) = (tire, node, tire_off || entry.disable & global != 0) else {
                        entry.latched = false;
                        entry.sources[0].off();
                        continue;
                    };
                    if let Some(Zone::Group(a, b)) = entry.zones.get(usize::from(t.zone)) {
                        entry.frames = (*a, *b);
                    }
                    if t.ground {
                        entry.latched = true;
                    } else if t.object {
                        entry.latched = false;
                    }
                    if !entry.latched || !(keys.min_compression < t.compression) {
                        entry.sources[0].off();
                        continue;
                    }
                    keys.apply(&entry.file, t.compression, &mut entry.emt);
                    let at = m.point(keys.offset);
                    let velocity = entry.emt.velocity;
                    entry.sources[0].emit_over(index, &entry.emt, entry.frames, at, None, Some(*m), velocity, plane.vel, wind, dt, &mut self.rng, &mut self.particles, &mut |p| ground(p).map(|(y, _)| y));
                }
                Kind::HelicopterDust { begin, end } => {
                    let (begin, end) = (*begin, *end);
                    let node = entry.node.as_ref().and_then(|n| plane.nodes.iter().find(|(name, _)| name.eq_ignore_ascii_case(n)));
                    entry.sources.resize_with(1, Source::default);
                    let (Some((_, m)), false) = (node, tire_off || entry.disable & global != 0) else {
                        entry.sources[0].off();
                        continue;
                    };
                    let at = m.point(v3(0.0, 0.0, 0.0));
                    let Some((y, zone)) = ground(at) else {
                        entry.sources[0].off();
                        continue;
                    };
                    match entry.zones.get(usize::from(zone)) {
                        Some(Zone::Off) => {
                            entry.sources[0].off();
                            continue;
                        }
                        Some(Zone::Group(a, b)) => entry.frames = (*a, *b),
                        None => {}
                    }
                    let h = at.y - y;
                    let base = entry.file.rate;
                    if !(begin <= h) {
                        entry.emt.rate = base;
                    } else if h < end {
                        let f = 1.0 - (h - begin) / (end - begin);
                        entry.emt.rate = (base.0 * f, base.1 * f);
                    } else {
                        entry.sources[0].off();
                        continue;
                    }
                    let velocity = entry.emt.velocity;
                    entry.sources[0].emit_over(index, &entry.emt, entry.frames, at, None, Some(*m), velocity, plane.vel, wind, dt, &mut self.rng, &mut self.particles, &mut |p| ground(p).map(|(y, _)| y));
                }
                _ => {}
            }
            match entry.kind {
                Kind::Generic | Kind::Exhaust { .. } | Kind::Spray(_) => {
                    let node = entry.node.as_ref().and_then(|n| plane.nodes.iter().find(|(name, _)| name.eq_ignore_ascii_case(n)));
                    // Only an entry that is exactly live is run; the rest are switched off, and a
                    // spray's fade is dropped.
                    let mut on = entry.flag == Entry::LIVE;
                    match &entry.kind {
                        Kind::Exhaust { max_speed } if on => {
                            let f = plane.throttle * 0.5 * (max_speed - plane.airspeed) / max_speed.max(1e-6);
                            on = f > 0.0;
                            // The puffs blow back along the pipe faster with the throttle (P74: the
                            // capture's velocity z; y stays the file's).
                            entry.emt.velocity.z = entry.file.velocity.z * (f + 0.5);
                        }
                        Kind::Spray(keys) if on => match tire {
                            Some(t) if (plane.throttle == 0.0 || plane.clutch != 0.0 || !t.spinning) && t.side.abs() <= keys.side_threshold => {
                                // Not throwing: it runs on for what is left of its fade, thinning.
                                on = false;
                                if entry.clock > 0.0 {
                                    entry.clock -= dt;
                                    if entry.clock > 0.0 {
                                        let k = entry.clock / SPRAY_FADE;
                                        entry.emt.rate = (entry.file.rate.0 * k, entry.file.rate.1 * k);
                                        on = true;
                                    }
                                }
                                if !on {
                                    entry.clock = 0.0;
                                }
                            }
                            Some(t) => {
                                keys.apply(&entry.file, t.side, plane.airspeed, &mut entry.emt);
                                entry.clock = SPRAY_FADE;
                            }
                            None => on = false,
                        },
                        Kind::Spray(_) => entry.clock = 0.0,
                        _ => {}
                    }
                    entry.sources.resize_with(1, Source::default);
                    let source = &mut entry.sources[0];
                    if entry.held {
                        // A toy's emitter: its gate says (V71). Not emitting it still takes its point,
                        // which the original's does whenever it is on a node, and a fixed one never moves.
                        match node {
                            Some((_, m)) => {
                                let at = m.point(v3(0.0, 0.0, 0.0));
                                let rng = &mut self.rng;
                                if entry.gate.pass(dt, &mut || rng.rand()) {
                                    let before = self.particles.len();
                                    let velocity = entry.emt.velocity;
                                    source.emit_over(index, &entry.emt, entry.frames, at, None, Some(*m), velocity, plane.vel, wind, dt, &mut self.rng, &mut self.particles, &mut |p| ground(p).map(|(y, _)| y));
                                    entry.gate.emitted(self.particles.len() - before);
                                } else {
                                    source.last = Some(at);
                                }
                            }
                            None => source.off(),
                        }
                        entry.lit = entry.gate.is_on();
                        continue;
                    }
                    let lit = entry.lit;
                    entry.lit = on && node.is_some();
                    match node {
                        // Switched on this frame, it starts from here and emits from the next.
                        Some((_, m)) if on && !lit => source.last = Some(m.point(v3(0.0, 0.0, 0.0))),
                        Some((_, m)) if on => {
                            let at = m.point(v3(0.0, 0.0, 0.0));
                            let velocity = entry.emt.velocity;
                            source.emit_over(index, &entry.emt, entry.frames, at, None, Some(*m), velocity, plane.vel, wind, dt, &mut self.rng, &mut self.particles, &mut |p| ground(p).map(|(y, _)| y));
                        }
                        _ => source.off(),
                    }
                }
                Kind::Constraint { mask } => {
                    // One emitter for every sphere: on by the plane's flags alone, then emitted by hand
                    // at each sphere of the mask that touches as the entry allows, on a surface its
                    // zones draw, as if it had come from its bone's velocity a frame back.
                    entry.sources.resize_with(1, Source::default);
                    entry.lit = entry.disable & global == 0;
                    if !entry.lit {
                        entry.sources[0].off();
                        continue;
                    }
                    for (i, c) in plane.constraints.iter().enumerate().take(32) {
                        if mask & (1 << i) == 0 || entry.disable & c.flags() != 0 {
                            continue;
                        }
                        match entry.zones.get(usize::from(c.zone)) {
                            Some(Zone::Off) => continue,
                            Some(Zone::Group(a, b)) => entry.frames = (*a, *b),
                            None => {}
                        }
                        // The first sphere after it went on only takes its point (P153).
                        if entry.sources[0].last.is_none() {
                            entry.sources[0].last = Some(c.at);
                            continue;
                        }
                        let from = c.at.sub(c.vel.scale(dt));
                        entry.sources[0].emit_over(index, &entry.emt, entry.frames, c.at, Some(from), None, entry.emt.velocity, c.vel, wind, dt, &mut self.rng, &mut self.particles, &mut |p| ground(p).map(|(y, _)| y));
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
                    entry.sources.resize_with(1, Source::default);
                    entry.lit = entry.disable & flags == 0;
                    if !entry.lit {
                        entry.sources[0].off();
                        continue;
                    }
                    // One emitter for every contact (P153: the capture's one carry), emitted at each
                    // in turn as if the point had come from the contact's velocity a frame back: its
                    // speed is the contact's, and its particles spread along that path.
                    let vel = plane.contact_vel.unwrap_or(plane.vel);
                    let source = &mut entry.sources[0];
                    for (n, &at) in plane.ground_contacts.iter().chain(&plane.object_contacts).enumerate() {
                        // A ground contact on a surface the zones say `none` to emits nothing (P74).
                        match plane.ground_zones.get(n).and_then(|&z| entry.zones.get(usize::from(z))) {
                            Some(Zone::Off) => continue,
                            Some(Zone::Group(a, b)) => entry.frames = (*a, *b),
                            None => {}
                        }
                        // The first contact after it went on only takes its point.
                        if source.last.is_none() {
                            source.last = Some(at);
                            continue;
                        }
                        let from = at.sub(vel.scale(dt));
                        source.emit(index, &entry.emt, entry.frames, at, Some(from), None, entry.emt.velocity, vel, wind, dt, &mut self.rng, &mut self.particles);
                    }
                }
                Kind::Dust(_) | Kind::HelicopterDust { .. } => {}
            }
        }
    }

    /// The particles as squares to draw, in the order they were made, where the last tick left them.
    pub fn drawn(&self) -> Vec<Drawn> {
        self.drawn_at(1.0)
    }

    /// [`PlaneFx::drawn`] for a picture `by` of the way from the tick before the last to the last
    /// (P162), as the shell blends the machines: each particle slid from [`Particle::was`] to where it
    /// is, so the roost leaves the drawn tire and not the tick's.
    pub fn drawn_at(&self, by: f32) -> Vec<Drawn> {
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
                let at = p.was.add(p.at.sub(p.was).scale(by));
                Some(Drawn { at, size: p.size, uv: [l, t, r, b], alpha: p.alpha(emt) })
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

/// The track's own emitters (V45): the `.toy`'s `[ParticleEmitterN]` — nat08's camp fires and
/// chimney, nat09's smoke and bonfire, nat11's and nat17's water mist, nat12's puddle fog and mist and
/// its geysers, the torches and smokes of nat14, nat15, nat17 and nat18, every sx track's fireworks.
///
/// Each is the plain emitter the plane's `Generic` entries build ([`game_render::toy::Emitter`]),
/// fixed where the toy stands and turned by nothing — or, one with `AttachToToy` (V71), on its toy's
/// `AttachToSubObject` node, `OffsetFromObject` from it, turned with it ([`TrackFx::place`]). They
/// run as [`PlaneFx`] entries on a node each with no plane: no `Disable`, no tire, no wreck, and each
/// `held`, so its [`Gate`] says when it emits.
///
/// **The toy** resets its emitter as it is read (its delay and its one-shot) and switches it on or
/// off by `StartOn`. One switched on as the track loads has the frame after for its first, and emits
/// from the one after, as any emitter switched on does. One that starts off waits for:
/// - a trigger's `particles` action ([`TrackFx::touch`], [`game_render::toy::Trigger`]): nat12's two
///   geyser triggers, each with a coin toss as a rider comes in, reset and switch on their geyser's
///   blast, spray and mist, one-shots of 15 seconds;
/// - the race's finish ([`TrackFx::finish`]): every `Fireworks%02d` toy, from 01 until one is
///   missing, switched on — not reset — so each waits its `.emt`'s `DelayTime` and makes its shot.
///
/// Left out, by name in [`TrackFx::skipped`]: the ones on a scene object (`AttachToSceneRef`, or
/// `AttachToObject` without a toy), which no track's list has, and any class but the plain one.
#[derive(Clone, Debug)]
pub struct TrackFx {
    pub fx: PlaneFx,
    /// Each entry's node: its emitter's position, or its toy node's place once placed.
    pub frame: PlaneFrame,
    /// The `.toy`'s emitters the port does not run, and why.
    pub skipped: Vec<(String, String)>,
    /// Each of [`TrackFx::fx`]'s entries' toy, in step with them.
    pub toys: Vec<game_render::toy::Emitter>,
    /// Every emitter toy's name in the `.toy`, run or not: what the finish looks its fireworks up in.
    pub listed: Vec<String>,
    /// The `.toy`'s triggers with `particles` actions, and what each has seen.
    pub triggers: Vec<Touch>,
    /// Actions fired with a `TriggerActionDelayTimer` still to run: seconds left, emitter, on.
    pub pending: Vec<(f32, String, bool)>,
    /// The triggers' coin tosses: their own numbers, not the particles' nor the sim's.
    pub rng: Rng,
}

/// A trigger and what it saw.
#[derive(Clone, Debug)]
pub struct Touch {
    pub trigger: game_render::toy::Trigger,
    /// It has fired (a `OneTimeOnly` one fires once).
    pub fired: bool,
    /// The riders in it last frame, by their place in [`TrackFx::touch`]'s list.
    pub inside: Vec<usize>,
}

impl TrackFx {
    /// The emitters of a `.toy`'s `list` that the port runs, with their `.emt`s out of `read` and
    /// their frames out of the track's `atlas`, and no triggers ([`TrackFx::with_triggers`]).
    pub fn new(list: &[game_render::toy::Emitter], atlas: Atlas, read: impl Fn(&str) -> Option<Vec<u8>>) -> TrackFx {
        let mut skipped = Vec::new();
        let mut entries = Vec::new();
        let mut nodes = Vec::new();
        for (i, e) in list.iter().enumerate() {
            let why = if !e.is_plain() {
                format!("class {}", e.kind)
            } else if e.attached && e.toy.is_none() {
                "on an object".to_string()
            } else {
                // Its node is named by its place in the list: two emitters may share a name. One
                // on a toy has none until it is placed.
                let node = format!("ParticleEmitter{}", i + 1);
                if e.toy.is_none() {
                    let mut at = M4::IDENTITY;
                    at.0[3] = [e.position.x, e.position.y, e.position.z, 1.0];
                    nodes.push((node.clone(), at));
                }
                entries.push(Emitter { name: e.name.clone(), kind: "Generic".into(), emitter: e.file.clone(), node: Some(node), ..Emitter::default() });
                continue;
            };
            skipped.push((e.name.clone(), why));
        }
        let mut fx = PlaneFx::new(&entries, atlas, read);
        fx.rng = Rng::new(0x5eed_0045);
        skipped.append(&mut fx.skipped);
        let toys = fx
            .entries
            .iter()
            .map(|entry| {
                let k: usize = entry.node.as_deref().and_then(|n| n.strip_prefix("ParticleEmitter")).and_then(|n| n.parse().ok()).unwrap_or(1);
                list[k - 1].clone()
            })
            .collect();
        let rng = &mut fx.rng;
        for entry in &mut fx.entries {
            entry.held = true;
            entry.gate = Gate::new(&entry.emt, &mut || rng.rand());
        }
        let mut out = TrackFx {
            fx,
            frame: PlaneFrame { nodes, ..PlaneFrame::default() },
            skipped,
            toys,
            listed: list.iter().map(|e| e.name.clone()).collect(),
            triggers: Vec::new(),
            pending: Vec::new(),
            rng: Rng::new(0x5eed_0071),
        };
        out.reset();
        out
    }

    /// [`TrackFx::new`] with the `.toy`'s triggers that have `particles` actions.
    pub fn with_triggers(mut self, triggers: Vec<game_render::toy::Trigger>) -> TrackFx {
        self.triggers = triggers.into_iter().map(|trigger| Touch { trigger, fired: false, inside: Vec::new() }).collect();
        self
    }

    /// The emitter toys' reset (the track's load, the race's restart): each emitter's timers reset,
    /// then on or off by its `StartOn`.
    pub fn reset(&mut self) {
        let rng = &mut self.fx.rng;
        for (entry, toy) in self.fx.entries.iter_mut().zip(&self.toys) {
            entry.gate.reset(&mut || rng.rand());
            if toy.start_on {
                entry.gate.on();
            } else {
                entry.gate.off();
            }
        }
        self.pending.clear();
    }

    /// The emitter toy's on/off message to every emitter named `name`: switched, its timers left.
    pub fn switch(&mut self, name: &str, on: bool) {
        for entry in self.fx.entries.iter_mut().filter(|e| e.name.eq_ignore_ascii_case(name)) {
            if on {
                entry.gate.on();
            } else {
                entry.gate.off();
            }
        }
    }

    /// A trigger's `particles` action on `name`: on, its timers reset first; off, just off.
    pub fn fire(&mut self, name: &str, on: bool) {
        let rng = &mut self.fx.rng;
        for entry in self.fx.entries.iter_mut().filter(|e| e.name.eq_ignore_ascii_case(name)) {
            if on {
                entry.gate.reset(&mut || rng.rand());
                entry.gate.on();
            } else {
                entry.gate.off();
            }
        }
    }

    /// The race's finish, when its winner crosses the line: every `Fireworks01`, `Fireworks02`… the
    /// `.toy` lists, until a number it does not, switched on. Nothing calls it until the port has a
    /// race to finish.
    #[allow(dead_code)]
    pub fn finish(&mut self) {
        for n in 1.. {
            let name = format!("Fireworks{n:02}");
            if !self.listed.iter().any(|l| l.eq_ignore_ascii_case(&name)) {
                break;
            }
            self.switch(&name, true);
        }
    }

    /// Who is in the triggers this frame: `riders` are the points the triggers test. A trigger that
    /// is enabled, and not a `OneTimeOnly` one that has fired, fires for each rider in it; one under
    /// 100 percent rolls for each only on a frame no rider was in it the frame before, and fires on
    /// `percent ≥ int(rand · 2⁻³¹ · 100) + 1`. Firing runs its actions for a rider not in it the
    /// frame before, each now or after its `TriggerActionDelayTimer`.
    pub fn touch(&mut self, riders: &[V3]) {
        use ps2_float::fpu::{from_i32, mul, to_i32};
        for t in &mut self.triggers {
            let now: Vec<usize> = (0..riders.len()).filter(|&i| t.trigger.holds(riders[i])).collect();
            for &i in &now {
                if !t.trigger.enabled || (t.trigger.one_time && t.fired) {
                    break;
                }
                if t.trigger.percent != 100 {
                    if !t.inside.is_empty() {
                        break;
                    }
                    let roll = to_i32(mul(mul(from_i32(self.rng.rand()), f32::from_bits(0x3000_0000)), 100.0)) + 1;
                    if t.trigger.percent < roll {
                        continue;
                    }
                }
                t.fired = true;
                if t.inside.contains(&i) {
                    continue;
                }
                for a in &t.trigger.actions {
                    self.pending.push((a.delay, a.emitter.clone(), a.on));
                }
            }
            t.inside = now;
        }
        self.run_pending(0.0);
    }

    /// The fired actions whose delay has run out by `dt` more, done.
    fn run_pending(&mut self, dt: f32) {
        let mut due = Vec::new();
        self.pending.retain_mut(|(left, name, on)| {
            *left -= dt;
            if *left <= 0.0 {
                due.push((name.clone(), *on));
                false
            } else {
                true
            }
        });
        for (name, on) in due {
            self.fire(&name, on);
        }
    }

    /// Put the emitters on toys where their nodes are: `node(toy, sub_object)` answers the world
    /// place of a toy's node by name (row vectors, its point the node's origin), `None` where the
    /// toy or the node is not there. One whose node is not found has none, and emits nothing.
    pub fn place(&mut self, node: &mut dyn FnMut(&str, &str) -> Option<M4>) {
        for (entry, toy) in self.fx.entries.iter().zip(&self.toys) {
            let (Some(name), Some(at)) = (&toy.toy, &entry.node) else { continue };
            let sub = toy.sub_object.as_deref().unwrap_or("");
            let placed = node(name, sub).map(|m| {
                let mut out = if toy.rotate { m } else { M4::IDENTITY };
                let p = m.point(toy.offset);
                out.0[3] = [p.x, p.y, p.z, 1.0];
                out
            });
            let slot = self.frame.nodes.iter().position(|(n, _)| n == at);
            match (placed, slot) {
                (Some(m), Some(k)) => self.frame.nodes[k].1 = m,
                (Some(m), None) => self.frame.nodes.push((at.clone(), m)),
                (None, Some(k)) => {
                    self.frame.nodes.remove(k);
                }
                (None, None) => {}
            }
        }
    }

    /// Whether any emitter rides a toy: the shell asks before looking its toys' nodes up.
    pub fn on_toys(&self) -> bool {
        self.toys.iter().any(|t| t.toy.is_some())
    }

    /// One frame of `dt` seconds: the fired actions whose delay runs out, the particles in the air,
    /// then what each emitter makes. `ground` as [`PlaneFx::step_over`]'s.
    pub fn step_over(&mut self, dt: f32, ground: &mut dyn FnMut(V3) -> Option<(f32, u8)>) {
        self.run_pending(dt);
        self.fx.step_over(&self.frame, dt, ground);
    }
}
