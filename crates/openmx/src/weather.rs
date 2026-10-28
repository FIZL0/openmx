//! The track's weather (V73): nat01's rain. The `.toy`'s `[WeatherBoxToyN]` is a box of rain that
//! follows a camera, and its `[TriggerN]`s with `ActionType = Weather` (`RainBegins`, `RainEnds`)
//! turn the box's density up and down as a rider rides through them.
//!
//! **The box** (the weather box toy's read): `Box` is its whole size, centred on the camera with
//! `FollowCamera` (on nat01, camera 0's eye). It is never turned: its sides are the world's. It
//! holds `NumberOfParticles / 3 + 1` packets of three drops each. A drop starts at one of
//! `3 × NumberOfParticles` points drawn once at load, uniform across the box's width and depth and
//! `Box.y × MinStartingRangeFromTop / 100` down from its top, taken in turn. It falls at one of 16
//! speeds from `MaxParticleVelocity` to `MinParticleVelocity`, is one of 16 sizes from
//! `MinParticleSize` to `MaxParticleSize`, and is drawn `size × AspectRatio` long, both taken in turn.
//!
//! **The density** ([`Rain::update`], the toy's update, once a frame): the target is `StartDensity`
//! until an action changes it, and the density moves toward it by `StartDensityStep` a frame, or by
//! the action's `ChangeDensityStep`. It goes through three states, which lose a frame at each turn:
//! rising, steady and falling. While it rises, the packets live half as long.
//!
//! **The packets** ([`Rain::emit`], the toy's draw, once a frame): every packet whose time is up is
//! started again while fewer than `packets × density` are live. It lives `MinParticleLife` plus
//! one of 16 shares of `MaxParticleLife − MinParticleLife` drawn at load, taken in turn. Then the
//! packets from that count on are cut short, so a falling density puts out the last ones first.
//! A started drop falls on its own from where it started, in the world, and the box moves on.
//!
//! **The slant.** The streaks lie along the camera's last move plus `CameraTilt` feet up, which is
//! straight up for a camera that stands still and leans into the wind for one that rides. The drop
//! falls along the reverse of that, its fall speed in place of the up.
//!
//! **The actions** ([`Rain::act`]): `ChangeDensity` 0 or more sets the target, `ChangeDensityStep`
//! 0 or more the step, `NewAspectRatio` above 0 the streaks' length, and `NewMinParticleVelocity` or
//! `NewMaxParticleVelocity` above 0 both speeds. The triggers' contact is V71's (`particles.rs`):
//! one under 100 percent rolls as a rider comes in.
//!
//! **Its own random numbers.** It is presentation: nothing here reads or draws from the simulation's
//! `rand`, as the plane's and the track's particles do not.
//!
//! **Drawn** as glows ([`Rain::glows`]): each live drop a streak of the box's `Material` texture,
//! added to the frame (the material's blend is `Cs × As + Cd`), turned to face the view's eye.
//!
//! This file is std and `game_render`'s plain half only, so the tests compile it in by path.

use game_render::glow::{Glow, Vertex};
use game_render::ini;
use game_render::mat::{V3, v3};
use game_render::toy::Trigger;

/// A `[WeatherBoxToyN]`, its keys with the original's defaults where it is missing one.
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherBox {
    /// `Name`: what the actions' `WeatherBoxName` finds it by.
    pub name: String,
    /// `Position`: the box's centre where it does not follow a camera.
    pub position: V3,
    /// `Box`, whole sizes, or `Radius` for all three where above 0.
    pub size: V3,
    /// `IsRain` (on).
    pub rain: bool,
    /// `NumberOfParticles` (0).
    pub particles: usize,
    /// `FollowCamera` (on), and `CameraIndex` (0).
    pub follow_camera: bool,
    pub camera: usize,
    /// `MinParticleVelocity` (−180), `MaxParticleVelocity` (−220): feet a second, down negative.
    pub min_velocity: f32,
    pub max_velocity: f32,
    /// `MinParticleSize` (3), `MaxParticleSize` (5): feet.
    pub min_size: f32,
    pub max_size: f32,
    /// `MinParticleLife` (0.2), `MaxParticleLife` (1): seconds.
    pub min_life: f32,
    pub max_life: f32,
    /// `MinStartingRangeFromTop` (25), `MaxStartingRangeFromTop` (100): percent of the box's height.
    pub min_range: f32,
    pub max_range: f32,
    /// `AdjustLifeAndRangeToSpeed` (off): no track turns it on, and the port does not run it.
    pub adjust_to_speed: bool,
    /// `AspectRatio` (5): a streak's length over its size.
    pub aspect: f32,
    /// `CameraTilt` (4): feet added up to the camera's move to lean the streaks by.
    pub tilt: f32,
    /// `StartDensity` (1), `StartDensityStep` (0.1).
    pub start_density: f32,
    pub start_step: f32,
    /// `Material`: the texture's name.
    pub material: String,
}

/// The `.toy`'s weather boxes, `WeatherBoxToy1` to the count its `[WeatherBoxToys]` says.
pub fn boxes(text: &str) -> Vec<WeatherBox> {
    let sections = ini::parse(text);
    let find = |name: &str| sections.iter().find(|s| s.name.eq_ignore_ascii_case(name));
    let count = find("WeatherBoxToys").and_then(|s| s.number("NumberOfWeatherBoxToys")).unwrap_or(0.0).max(0.0) as usize;
    let mut out = Vec::new();
    for n in 1..=count {
        let Some(s) = find(&format!("WeatherBoxToy{n}")) else { continue };
        let num = |k: &str, d: f32| s.number(k).unwrap_or(d);
        let flag = |k: &str, d: bool| s.get(k).map_or(d, ini::switch);
        let vec = |k: &str| s.numbers(k).filter(|v| v.len() >= 3).map_or(V3::default(), |v| v3(v[0], v[1], v[2]));
        let size = match s.number("Radius").filter(|r| *r > 0.0) {
            Some(r) => v3(r, r, r),
            None => vec("Box"),
        };
        out.push(WeatherBox {
            name: s.get("Name").unwrap_or("").trim().to_string(),
            position: vec("Position"),
            size,
            rain: flag("IsRain", true),
            particles: num("NumberOfParticles", 0.0).max(0.0) as usize,
            follow_camera: flag("FollowCamera", true),
            camera: num("CameraIndex", 0.0).max(0.0) as usize,
            min_velocity: num("MinParticleVelocity", -180.0),
            max_velocity: num("MaxParticleVelocity", -220.0),
            min_size: num("MinParticleSize", 3.0),
            max_size: num("MaxParticleSize", 5.0),
            min_life: num("MinParticleLife", 0.2),
            max_life: num("MaxParticleLife", 1.0),
            min_range: num("MinStartingRangeFromTop", 25.0),
            max_range: num("MaxStartingRangeFromTop", 100.0),
            adjust_to_speed: flag("AdjustLifeAndRangeToSpeed", false),
            aspect: num("AspectRatio", 5.0),
            tilt: num("CameraTilt", 4.0),
            start_density: num("StartDensity", 1.0),
            start_step: num("StartDensityStep", 0.1),
            material: s.get("Material").unwrap_or("").trim().to_string(),
        });
    }
    out
}

/// A trigger's `Weather` action.
#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    /// `WeatherBoxName`.
    pub weather_box: String,
    /// `ChangeDensity` (−1: left), `ChangeDensityStep` (−1: left).
    pub density: f32,
    pub step: f32,
    /// `NewAspectRatio` (0: left).
    pub aspect: f32,
    /// `NewMinParticleVelocity`, `NewMaxParticleVelocity` (0: left).
    pub min_velocity: f32,
    pub max_velocity: f32,
}

/// The triggers a `.toy` lists, `Trigger1` to the count its `[Triggers]` says, that have a `Weather`
/// action, each with its volume and contact read as `particle_triggers` reads them.
pub fn triggers(text: &str) -> Vec<(Trigger, Vec<Action>)> {
    let sections = ini::parse(text);
    let find = |name: &str| sections.iter().find(|s| s.name.eq_ignore_ascii_case(name));
    let count = find("Triggers").and_then(|s| s.number("NumberOfTriggers")).unwrap_or(0.0).max(0.0) as usize;
    let mut out = Vec::new();
    for n in 1..=count {
        let Some(s) = find(&format!("Trigger{n}")) else { continue };
        let mut actions = Vec::new();
        for a in 1..=s.number("NumberOfActions").unwrap_or(0.0).max(0.0) as usize {
            let Some(act) = find(&format!("Trigger{n}Action{a}")) else { continue };
            if !act.get("ActionType").is_some_and(|t| t.trim().eq_ignore_ascii_case("Weather")) {
                continue;
            }
            let num = |k: &str, d: f32| act.number(k).unwrap_or(d);
            actions.push(Action {
                weather_box: act.get("WeatherBoxName").unwrap_or("").trim().to_string(),
                density: num("ChangeDensity", -1.0),
                step: num("ChangeDensityStep", -1.0),
                aspect: num("NewAspectRatio", 0.0),
                min_velocity: num("NewMinParticleVelocity", 0.0),
                max_velocity: num("NewMaxParticleVelocity", 0.0),
            });
        }
        if actions.is_empty() {
            continue;
        }
        let vec = |k: &str, d: V3| s.numbers(k).filter(|v| v.len() >= 3).map_or(d, |v| v3(v[0], v[1], v[2]));
        let look = vec("LookVector", v3(0.0, 0.0, 1.0)).norm();
        let right = vec("UpVector", v3(0.0, 1.0, 0.0)).cross(look).norm();
        let up = look.cross(right);
        let trigger = Trigger {
            name: s.get("Name").unwrap_or("").trim().to_string(),
            at: vec("Position", V3::default()),
            half: vec("Box", V3::default()),
            radius: s.number("Radius").filter(|r| *r > 0.0),
            right,
            up,
            look,
            enabled: s.get("StartEnabled").is_none_or(ini::switch),
            one_time: s.get("OneTimeOnly").is_some_and(ini::switch),
            percent: s.number("RandomTriggerPercent").unwrap_or(100.0) as i32,
            actions: Vec::new(),
        };
        out.push((trigger, actions));
    }
    out
}

/// Where the density is going.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ramp {
    Rising,
    Steady,
    Falling,
}

/// A started drop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Drop {
    /// Where it started, and when, on the box's clock.
    pub from: V3,
    pub born: f32,
    /// Its fall, feet a second.
    pub velocity: V3,
    /// Its streak: the way it lies (unit), its width and its length.
    pub axis: V3,
    pub size: f32,
    pub length: f32,
}

/// A packet: three drops and the time it is live until.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Packet {
    pub until: f32,
    pub drops: [Option<Drop>; 3],
}

/// One weather box's rain: its triggers, its density and its drops.
#[derive(Clone, Debug)]
pub struct Rain {
    pub weather: WeatherBox,
    /// The `.toy`'s triggers with `Weather` actions on this box, and the riders in each last frame.
    pub triggers: Vec<(Trigger, Vec<Action>, Vec<usize>, bool)>,
    /// The box's clock, seconds.
    pub clock: f32,
    pub ramp: Ramp,
    pub density: f32,
    pub target: f32,
    pub step: f32,
    /// A packet's least life now, and the box's own: halved while the density rises.
    pub life: f32,
    pub aspect: f32,
    pub min_velocity: f32,
    pub max_velocity: f32,
    /// The 16 fall speeds, sizes, extra lives and start depths, and which is next.
    pub speeds: [f32; 16],
    pub sizes: [f32; 16],
    pub lives: [f32; 16],
    pub turn: usize,
    /// The drops' start points from the box's centre, and which is next.
    pub points: Vec<V3>,
    pub point: usize,
    pub packets: Vec<Packet>,
    /// The box's centre at the last frame, and the way the streaks lie.
    pub centre: Option<V3>,
    pub axis: V3,
    pub rng: u32,
}

impl Rain {
    /// The box's rain with `triggers`' actions on it, its tables drawn as the box's read draws them.
    pub fn new(weather: WeatherBox, triggers: Vec<(Trigger, Vec<Action>)>) -> Rain {
        let triggers = triggers
            .into_iter()
            .filter_map(|(t, actions)| {
                let mine: Vec<Action> = actions.into_iter().filter(|a| a.weather_box.eq_ignore_ascii_case(&weather.name)).collect();
                (!mine.is_empty()).then_some((t, mine, Vec::new(), false))
            })
            .collect();
        let mut rain = Rain {
            triggers,
            clock: 0.0,
            ramp: Ramp::Steady,
            density: weather.start_density,
            target: weather.start_density,
            step: weather.start_step,
            life: weather.min_life,
            aspect: weather.aspect,
            min_velocity: weather.min_velocity,
            max_velocity: weather.max_velocity,
            speeds: [0.0; 16],
            sizes: [0.0; 16],
            lives: [0.0; 16],
            turn: 0,
            points: Vec::new(),
            point: 0,
            packets: vec![Packet { until: 0.0, drops: [None; 3] }; weather.particles / 3 + 1],
            centre: None,
            axis: v3(0.0, 1.0, 0.0),
            rng: 0x5eed_0073,
            weather,
        };
        rain.speed_table();
        let w = rain.weather.clone();
        let (size, spread) = (w.size, w.max_life - w.min_life);
        for i in 0..16 {
            rain.lives[i] = spread * rain.unit();
            // The start depths are drawn as the box's read draws them, for the speed-adjusted boxes
            // that use them; no track's box does.
            let _ = rain.unit();
            rain.sizes[i] = w.min_size + i as f32 * (w.max_size - w.min_size) / 15.0;
        }
        let range = w.min_range * 0.01;
        for _ in 0..w.particles * 3 {
            let (x, y, z) = (rain.unit(), rain.unit(), rain.unit());
            rain.points.push(v3(size.x * x - size.x * 0.5, (size.y - size.y * range * y) - size.y * 0.5, size.z * z - size.z * 0.5));
        }
        rain.reset();
        rain
    }

    /// The 16 fall speeds: the first `max_velocity`, the last `min_velocity`.
    fn speed_table(&mut self) {
        let span = self.max_velocity - self.min_velocity;
        for i in 0..16 {
            self.speeds[i] = self.min_velocity + ((15 - i) as f32 * span) / 15.0;
        }
    }

    /// A float in `[0, 1)` of the box's own numbers.
    fn unit(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// The box's reset (the track's load, the race's restart): its density, speeds and length back to
    /// the `.toy`'s, its clock to 0 and each packet's time spread over a life.
    pub fn reset(&mut self) {
        self.min_velocity = self.weather.min_velocity;
        self.max_velocity = self.weather.max_velocity;
        self.speed_table();
        self.aspect = self.weather.aspect;
        self.density = self.weather.start_density;
        self.target = self.weather.start_density;
        self.step = self.weather.start_step;
        self.ramp = Ramp::Steady;
        self.clock = 0.0;
        let n = self.packets.len();
        for (k, p) in self.packets.iter_mut().enumerate() {
            p.until = k as f32 * (self.life / n as f32);
            p.drops = [None; 3];
        }
        for t in &mut self.triggers {
            t.2.clear();
            t.3 = false;
        }
    }

    /// A `Weather` action on this box.
    pub fn act(&mut self, a: &Action) {
        if a.density >= 0.0 {
            self.target = a.density;
        }
        if a.step >= 0.0 {
            self.step = a.step;
        }
        if a.aspect > 0.0 {
            self.aspect = a.aspect;
        }
        if a.min_velocity > 0.0 || a.max_velocity > 0.0 {
            self.min_velocity = a.min_velocity;
            self.max_velocity = a.max_velocity;
            self.speed_table();
        }
    }

    /// Who is in the triggers this frame, `riders` the points they test, as `TrackFx::touch`: a
    /// trigger under 100 percent rolls for a rider only on a frame no rider was in it the frame
    /// before; firing runs its actions for a rider not in it the frame before.
    pub fn touch(&mut self, riders: &[V3]) {
        let mut fired = Vec::new();
        for k in 0..self.triggers.len() {
            let now: Vec<usize> = (0..riders.len()).filter(|&i| self.triggers[k].0.holds(riders[i])).collect();
            for &i in &now {
                let (t, _, inside, done) = &self.triggers[k];
                if !t.enabled || (t.one_time && *done) {
                    break;
                }
                if t.percent != 100 {
                    if !inside.is_empty() {
                        break;
                    }
                    let roll = (self.unit() * 100.0) as i32 + 1;
                    if self.triggers[k].0.percent < roll {
                        continue;
                    }
                }
                self.triggers[k].3 = true;
                if self.triggers[k].2.contains(&i) {
                    continue;
                }
                fired.extend(self.triggers[k].1.iter().cloned());
            }
            self.triggers[k].2 = now;
        }
        for a in &fired {
            self.act(a);
        }
    }

    /// The box's update, a frame of `dt` seconds: its clock, and the density a step toward its
    /// target, a frame lost at each turn of the ramp.
    pub fn update(&mut self, dt: f32) {
        self.clock += dt;
        let base = self.weather.min_life;
        match self.ramp {
            Ramp::Falling => {
                if self.target <= self.density {
                    self.density -= self.step;
                    if self.density < self.target {
                        self.density = self.target;
                        self.ramp = Ramp::Steady;
                    }
                } else {
                    self.ramp = Ramp::Steady;
                    self.life = base;
                }
            }
            Ramp::Steady => {
                if self.target < self.density {
                    self.ramp = Ramp::Falling;
                } else if self.density < self.target {
                    self.ramp = Ramp::Rising;
                }
            }
            Ramp::Rising => {
                if self.density <= self.target {
                    self.life = base * 0.5;
                    self.density += self.step;
                    if self.target < self.density {
                        self.density = self.target;
                        self.ramp = Ramp::Steady;
                        self.life = base;
                    }
                } else {
                    self.ramp = Ramp::Steady;
                    self.life = base;
                }
            }
        }
    }

    /// The box's draw, a frame: where it is (`eye`, the followed camera's, else its `Position`),
    /// the way the streaks lie, and the packets started and cut.
    pub fn emit(&mut self, eye: Option<V3>) {
        let centre = match (self.weather.follow_camera, eye) {
            (true, Some(eye)) => eye,
            _ => self.weather.position,
        };
        if self.weather.follow_camera {
            let moved = v3(centre.x, centre.y + self.weather.tilt, centre.z).sub(self.centre.unwrap_or(centre));
            self.axis = if moved.dot(moved) == 0.0 { v3(0.0, 1.0, 0.0) } else { moved.norm() };
        }
        self.centre = Some(centre);
        let n = self.packets.len();
        let cap = (n as f32 * self.density) as usize;
        let mut live = self.packets.iter().filter(|p| self.clock < p.until).count();
        for k in 0..n {
            if self.packets[k].until <= self.clock && live < cap {
                live += 1;
                self.packets[k].until = self.clock + self.life + self.lives[self.turn];
                for d in 0..3 {
                    let drop = self.drop(centre);
                    self.packets[k].drops[d] = Some(drop);
                }
            }
        }
        for p in self.packets.iter_mut().skip(live) {
            if self.clock < p.until {
                p.until = self.clock;
            }
        }
    }

    /// The next drop from the box at `centre`.
    fn drop(&mut self, centre: V3) -> Drop {
        let (speed, size) = (self.speeds[self.turn], self.sizes[self.turn]);
        self.turn = (self.turn + 1) % 16;
        let from = centre.add(self.points.get(self.point).copied().unwrap_or_default());
        self.point = if self.points.is_empty() { 0 } else { (self.point + 1) % self.points.len() };
        Drop {
            from,
            born: self.clock,
            velocity: v3(-self.axis.x, speed, -self.axis.z),
            axis: self.axis,
            size,
            length: size * self.aspect,
        }
    }

    /// The live drops `back` seconds before the box's clock (the picture's blend between ticks):
    /// each one's head and its streak.
    pub fn drawn(&self, back: f32) -> Vec<(V3, Drop)> {
        let at = self.clock - back;
        let mut out = Vec::new();
        for p in self.packets.iter().filter(|p| self.clock < p.until && at < p.until) {
            for d in p.drops.iter().flatten() {
                let age = at - d.born;
                if age >= 0.0 {
                    out.push((d.from.add(d.velocity.scale(age)), *d));
                }
            }
        }
        out
    }

    /// The live drops as `eye` sees them, `back` seconds before the clock: each a streak of the
    /// box's texture (`texture`, in the glow list) along its axis, `length` behind its head and its
    /// size across, facing the eye, added to the frame at `tint`.
    pub fn glows(&self, texture: usize, eye: V3, back: f32, tint: [f32; 4]) -> Glow {
        let mut tris = Vec::new();
        for (head, d) in self.drawn(back) {
            let tail = head.sub(d.axis.scale(d.length));
            let across = d.axis.cross(eye.sub(head)).norm().scale(d.size * 0.5);
            let vertex = |at: V3, u: f32, v: f32| Vertex { at: [at.x, at.y, at.z], uv: [u, v], tint };
            let (a, b) = (vertex(head.sub(across), 0.0, 0.0), vertex(head.add(across), 1.0, 0.0));
            let (c, e) = (vertex(tail.add(across), 1.0, 1.0), vertex(tail.sub(across), 0.0, 1.0));
            tris.push([a, b, c]);
            tris.push([a, c, e]);
        }
        Glow { texture, additive: true, tris }
    }
}
