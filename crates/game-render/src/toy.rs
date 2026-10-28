//! `*.toy` — a track's moving scenery, and the path cars that drive around it.
//!
//! Every track archive carries a `.toy` beside its `.scn`, in the same INI format (`ini.rs`). The
//! scene places what stands still; the toy file lists what moves: `[Cars]`, the birds, the sun's
//! flares, the terrain's sound zones, a physics collection or two. This module reads the cars, and
//! of those only the ones the original drives along a clip. That is free06's train (thirteen cars on
//! `train_spline.mot`), its four dump trucks and two scrapers, free09's trailers, and the rest of the
//! freeride tracks' trucks: everything with `HasPhysics` off. A car with it on is another class in
//! the original, a vehicle with a body of its own, and [`Car::has_physics`] is only read so that a
//! caller can leave those out.
//!
//! **The section.** `[Cars] NumberOfCars = n`, then `[Car1]` … `[Carn]`; a `[Car24]` past the count
//! (free06 has one, naming a clip the disc does not carry) is never read. The keys and their defaults
//! are the original's reader's: `TimeScale` 1, `HeadingFilter` 0.5, `NormalFilter` 0.2,
//! `LagDistance` 0.
//!
//! **The path.** A car's `.mot` has one channel, `Body`, of position keys: a time and a point each.
//! Between two keys the car is on a cubic Hermite segment whose tangents are the Catmull-Rom ones,
//! scaled to the segment's share of time ([`Path::at`]). At load every segment's length is measured
//! as the sum of ten chords along it ([`Path::new`]); the last key's is zero.
//!
//! **The clock.** Each car's clock is its clip's: it starts at zero when the track loads and runs at
//! `TimeScale` seconds per second, wrapped at the last key's time. A car with a `LagDistance` is the
//! rest of a train: it is where the clock's point on the path was that many feet of path ago
//! ([`Path::lagged`]), which is how thirteen cars that all share one clock and one clip couple up.
//!
//! **Where the car is drawn.** The path's `x` and `z`, and the ground's height under them: every car
//! on the disc is snapped down (or up — the train's spline runs forty-six feet under its rails) onto
//! the surface by the world's ground query each frame, and the surface's normal is its up. A car with
//! `PresampleTerrain` does that once, to every key, on its first frame, and then rides the curve
//! through those keys with the two keys' normals blended. Its heading is the way it moved this frame
//! with the vertical dropped, filtered by `HeadingFilter`; its up is the ground normal, filtered by
//! `NormalFilter` and *not followed at all* when it turns by more than 45° in one frame. The frame is
//! then the engine's look-at, keeping the up exactly and bending the heading to it.
//!
//! **What is the port's own.** The original keeps the heading and the up as state from frame to frame;
//! [`Path::pose`] instead replays the last [`WARM`] frames from scratch each time it is asked. Both
//! filters forget their start geometrically (a half and a fifth per frame), so after that many frames
//! what is left of the start is below a float's last bit, and the car's pose is a function of its clock
//! alone. That is what a rollback wants, and it is a choice, not a finding. The clock itself is the
//! caller's: the ride's elapsed time in, [`clock`] out. A picture between two scene updates asks at
//! the clock between them ([`between`]), so the cars move every frame drawn, not every update.

use crate::ini::{self, Section};
use crate::mat::{M4, V3, v3};
use crate::skin;

/// One car as its `[CarN]` section has it.
#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    /// `FileName`: the model.
    pub model: String,
    /// `CollisionFileName`: the shape the bike hits. Not used here.
    pub collision: String,
    /// `MOTFileName`: the clip whose `Body` channel is the path.
    pub mot: String,
    /// `TimeScale`: clip seconds per second.
    pub time_scale: f32,
    /// `HeadingFilter`: how much of last frame's heading is kept.
    pub heading_filter: f32,
    /// `NormalFilter`: how much of last frame's up is kept.
    pub normal_filter: f32,
    /// `LagDistance`: feet of path behind the clock's point.
    pub lag: f32,
    /// `PathIsLevel`. Off for every car on the disc; the simulation rides it (S128), this module does not.
    pub path_is_level: bool,
    /// `PresampleTerrain`: snap the keys to the ground once instead of the car every frame.
    pub presample: bool,
    /// `DriveOnSoultree`. Off for every car on the disc; the ground ray it asks for is not ported.
    pub drive_on_soultree: bool,
    /// `HasPhysics`: another class altogether, a vehicle with a body. Not drawn by this module.
    pub has_physics: bool,
    /// `SteerFrontTires` (on where it is missing): the first two wheels turn into the bends ([`Wheels`]).
    pub steer_front: bool,
}

impl Car {
    /// The model's name as the renderer's assets know it: `FileName` without its `.psg`.
    pub fn stem(&self) -> &str {
        let m = self.model.as_str();
        match m.len().checked_sub(4) {
            Some(cut) if m.is_char_boundary(cut) && m[cut..].eq_ignore_ascii_case(".psg") => &m[..cut],
            _ => m,
        }
    }
}

/// The cars a `.toy` lists, `Car1` to `CarN` for the `NumberOfCars` its `[Cars]` says.
pub fn cars(text: &str) -> Vec<Car> {
    let sections = ini::parse(text);
    let find = |name: &str| sections.iter().find(|s| s.name.eq_ignore_ascii_case(name));
    let count = find("Cars").and_then(|s| s.number("NumberOfCars")).unwrap_or(0.0) as usize;
    (1..=count).filter_map(|i| find(&format!("Car{i}")).map(car)).collect()
}

fn car(s: &Section) -> Car {
    let text = |key: &str| s.get(key).unwrap_or("").trim().to_string();
    let flag = |key: &str| s.get(key).is_some_and(ini::switch);
    // The numbers as the original reads them (S81): its `strtod` and then its truncating
    // double-to-float, so `TimeScale=0.95` is the float just below 0.95.
    let number = |key: &str| s.get(key).map(|v| game_sim::phy::number(v).0);
    Car {
        model: text("FileName"),
        collision: text("CollisionFileName"),
        mot: text("MOTFileName"),
        time_scale: number("TimeScale").unwrap_or(1.0),
        heading_filter: number("HeadingFilter").unwrap_or(0.5),
        normal_filter: number("NormalFilter").unwrap_or(0.2),
        lag: number("LagDistance").unwrap_or(0.0),
        path_is_level: flag("PathIsLevel"),
        presample: flag("PresampleTerrain"),
        drive_on_soultree: flag("DriveOnSoultree"),
        has_physics: flag("HasPhysics"),
        steer_front: s.get("SteerFrontTires").is_none_or(ini::switch),
    }
}

/// One key of a path: its time, the length of the segment that starts at it, its point, and — once a
/// presampling car has put it on the ground — the ground's normal there.
#[derive(Clone, Copy, Debug)]
pub struct Key {
    pub t: f32,
    pub len: f32,
    pub pos: V3,
    pub normal: V3,
}

/// A car's path: the keys of its clip's `Body` channel.
#[derive(Clone, Debug)]
pub struct Path {
    pub keys: Vec<Key>,
    /// The clip's own key positions, before any presample moved them: what the simulation's car
    /// measures and presamples itself (S128).
    pub clip: Vec<V3>,
}

/// Where a car is and which way it faces.
#[derive(Clone, Copy, Debug)]
pub struct Pose {
    /// The point on the path, before the ground moved it.
    pub path: V3,
    /// Where the car is drawn: the path's `x` and `z` on the ground.
    pub pos: V3,
    /// The filtered heading, flat and not normalised (the original's own value).
    pub head: V3,
    /// The filtered up.
    pub normal: V3,
    /// Model to world.
    pub transform: M4,
}

/// The ground under a point: the point moved onto the surface, and the surface's normal.
pub type Ground<'a> = dyn FnMut(V3) -> (V3, V3) + 'a;

/// How many frames [`Path::pose`] replays to settle the two filters: see the module note.
pub const WARM: usize = 48;

/// A car's clock after `elapsed` seconds of the track: its clip's time, at its own rate, wrapped.
pub fn clock(elapsed: f64, time_scale: f32, last: f32) -> f32 {
    if last <= 0.0 {
        return 0.0;
    }
    (elapsed * time_scale as f64).rem_euclid(last as f64) as f32
}

/// The scene's clock as a picture between two scene updates shows it (V47): `before` is the clock
/// the update before the last left, `last` the last's, and `by` how far the picture is from one to
/// the other — the bike's blend, so the cars and the bike are drawn at the same moment. The cars are
/// then placed at that clock, their clip's curve and their lags evaluated there, rather than their
/// matrices slid between two placements: a car on a bend stays on its rails, and every car of the
/// train reads the one clock, so they stay coupled. A picture drawn at the update's own clock would
/// show each placement for as many frames as fall in an update, and the train would move in stutters.
pub fn between(before: f64, last: f64, by: f32) -> f64 {
    before + (last - before) * f64::from(by.clamp(0.0, 1.0))
}

impl Path {
    /// The path in a parsed clip: its `Body` position channel, with every segment measured.
    pub fn from_clip(clip: &skin::Clip) -> Option<Path> {
        let body = clip.names.iter().position(|n| n == "Body")? as i32;
        let channel = clip.position.iter().find(|c| c.name == body)?;
        let keys = channel.keys.iter().map(|k| (k.t, v3(k.p[0], k.p[1], k.p[2]))).collect();
        Path::new(keys)
    }

    /// A path through these keys. Each segment's length is the sum of the ten chords between eleven
    /// points along it, the last key's is zero.
    pub fn new(points: Vec<(f32, V3)>) -> Option<Path> {
        if points.len() < 2 {
            return None;
        }
        let clip = points.iter().map(|&(_, pos)| pos).collect();
        let keys = points.into_iter().map(|(t, pos)| Key { t, len: 0.0, pos, normal: v3(0.0, 1.0, 0.0) }).collect();
        let mut path = Path { keys, clip };
        path.measure();
        Some(path)
    }

    /// Measure every segment: the sum of the ten chords between eleven points along it. The last
    /// key's length is zero.
    pub fn measure(&mut self) {
        let path = self;
        for i in 0..path.keys.len() - 1 {
            let (t0, span) = (path.keys[i].t, path.keys[i + 1].t - path.keys[i].t);
            let mut before = path.at(t0);
            let mut len = 0.0;
            for k in 1..11 {
                let now = path.at(t0 + span * k as f32 * 0.1);
                len += now.sub(before).len();
                before = now;
            }
            path.keys[i].len = len;
        }
    }

    /// The last key's time: the clock's period.
    pub fn last(&self) -> f32 {
        self.keys.last().map_or(0.0, |k| k.t)
    }

    /// The segment a clip time is in: the last key at or before it, never the final key.
    fn segment(&self, s: f32) -> usize {
        let last = self.keys.len() - 2;
        let i = self.keys.partition_point(|k| k.t <= s);
        i.saturating_sub(1).min(last)
    }

    /// The point on the path at clip time `s`, and how far through its segment that is.
    fn point(&self, s: f32) -> (V3, usize, f32) {
        let k = &self.keys;
        let i = self.segment(s);
        let n = k.len();
        let (t0, t1) = (k[i].t, k[i + 1].t);
        let (p0, p1) = (k[i].pos, k[i + 1].pos);
        let chord = p1.sub(p0);
        let t0_tan = if i == 0 {
            chord
        } else {
            p0.sub(k[i - 1].pos).add(chord).scale((t1 - t0) / (t1 - k[i - 1].t))
        };
        let t1_tan = if i + 1 == n - 1 {
            chord
        } else {
            k[i + 2].pos.sub(p1).add(chord).scale((t1 - t0) / (k[i + 2].t - t0))
        };
        let t = (s - t0) / (t1 - t0);
        let (t2, t3) = (t * t, t * t * t);
        let h = 3.0 * t2 - 2.0 * t3;
        let p = p0
            .scale(1.0 - h)
            .add(p1.scale(h))
            .add(t0_tan.scale(t3 - 2.0 * t2 + t))
            .add(t1_tan.scale(t3 - t2));
        (p, i, t)
    }

    /// The point on the path at clip time `s`.
    pub fn at(&self, s: f32) -> V3 {
        self.point(s).0
    }

    /// The clip time `lag` feet of path behind clip time `clock`, walking back over the segments'
    /// measured lengths (and round the end of the clip to its start's end). A segment of no length
    /// answers zero, as the original's does.
    pub fn lagged(&self, clock: f32, lag: f32) -> f32 {
        let k = &self.keys;
        let n = k.len() as isize;
        let mut i = self.segment(clock) as isize;
        let mut along = (clock - k[i as usize].t) / (k[i as usize + 1].t - k[i as usize].t) * k[i as usize].len;
        let mut guard = 0;
        while along < lag && guard < 4 * n {
            i -= 1;
            if i < 0 {
                i += n;
            }
            along += k[i as usize].len;
            guard += 1;
        }
        let key = &k[i as usize];
        if key.len == 0.0 {
            return 0.0;
        }
        let next = &k[((i + 1) % n) as usize];
        key.t + (along - lag) / key.len * (next.t - key.t)
    }

    /// A presampling car's first frame: every key put on the ground, with the ground's normal there.
    pub fn presample(&mut self, ground: &mut Ground) {
        for key in &mut self.keys {
            let (on, normal) = ground(key.pos);
            key.pos = on;
            key.normal = normal;
        }
    }

    /// Where the car is at its clock `clock`, replaying [`WARM`] frames of `step` clip seconds each
    /// to settle its filters.
    pub fn pose(&self, car: &Car, clock: f32, step: f32, ground: &mut Ground) -> Pose {
        let period = self.last();
        let wrap = |s: f32| if period > 0.0 { s.rem_euclid(period) } else { 0.0 };
        let mut head = v3(0.0, 0.0, 0.0);
        let mut normal = v3(0.0, 1.0, 0.0);
        let mut before: Option<V3> = None;
        let mut pose = None;
        for back in (0..=WARM).rev() {
            let s = wrap(clock - back as f32 * step);
            let s = if car.lag > 0.0 { self.lagged(s, car.lag) } else { s };
            let (path, i, t) = self.point(s);
            if let Some(before) = before {
                let moved = path.sub(before);
                let flat = v3(moved.x, 0.0, moved.z);
                if flat.len() > 0.0 {
                    let dir = flat.norm();
                    head = head.scale(car.heading_filter).add(dir.scale(1.0 - car.heading_filter));
                }
            }
            before = Some(path);
            let (pos, up) = if car.presample {
                let (a, b) = (self.keys[i].normal, self.keys[i + 1].normal);
                (path, a.add(b.sub(a).scale(t)))
            } else {
                ground(path)
            };
            if normal.dot(up) > 0.707 {
                normal = normal.scale(car.normal_filter).add(up.scale(1.0 - car.normal_filter));
            }
            pose = Some((path, pos));
        }
        let (path, pos) = pose.expect("WARM frames ran");
        Pose { path, pos, head, normal, transform: upright(pos, head, normal) }
    }
}

/// The ground [`Path::pose_kept`] found at a car's warm-up steps (V96): one entry a step of the
/// clock's grid, from step `first` on, so the next picture asks only for the steps it has not seen.
#[derive(Clone, Debug, Default)]
pub struct Kept {
    first: i64,
    at: std::collections::VecDeque<(V3, V3)>,
}

impl Path {
    /// [`Path::pose`] for a car posed picture after picture (V96, a world's dressing): its [`WARM`]
    /// warm-up frames stand on the clock's grid of `step`s rather than back from `clock` itself, so
    /// one picture's are the last one's moved on by a step or none, and the ground under each is
    /// kept in `kept` rather than asked again; the last frame is at `clock`, on the ground asked
    /// there. The filters settle as [`Path::pose`]'s do, from frames up to a step apart from its: a
    /// pose differs from [`Path::pose`]'s by what the filters make of that, which is not drawn
    /// apart. A presampling car, or no step, is [`Path::pose`].
    pub fn pose_kept(&self, car: &Car, clock: f32, step: f32, ground: &mut Ground, kept: &mut Kept) -> Pose {
        if car.presample || !(step > 0.0) {
            return self.pose(car, clock, step, ground);
        }
        let period = self.last();
        let wrap = |s: f32| if period > 0.0 { s.rem_euclid(period) } else { 0.0 };
        let last = (clock / step).floor() as i64;
        let first = last - WARM as i64 + 1;
        // Steps before this picture's first go; a clock that went back, or jumped past them all, starts over.
        let end = kept.first + kept.at.len() as i64;
        if first < kept.first || end < first {
            kept.at.clear();
            kept.first = first;
        }
        while kept.first < first {
            kept.at.pop_front();
            kept.first += 1;
        }
        let mut head = v3(0.0, 0.0, 0.0);
        let mut normal = v3(0.0, 1.0, 0.0);
        let mut before: Option<V3> = None;
        let mut pose = None;
        for g in first..=last + 1 {
            let at = if g <= last { g as f32 * step } else { clock };
            let s = wrap(at);
            let s = if car.lag > 0.0 { self.lagged(s, car.lag) } else { s };
            let (path, _, _) = self.point(s);
            if let Some(before) = before {
                let moved = path.sub(before);
                let flat = v3(moved.x, 0.0, moved.z);
                if flat.len() > 0.0 {
                    let dir = flat.norm();
                    head = head.scale(car.heading_filter).add(dir.scale(1.0 - car.heading_filter));
                }
            }
            before = Some(path);
            let (pos, up) = match (g - kept.first) as usize {
                _ if g > last => ground(path),
                k if k < kept.at.len() => kept.at[k],
                _ => {
                    let on = ground(path);
                    kept.at.push_back(on);
                    on
                }
            };
            if normal.dot(up) > 0.707 {
                normal = normal.scale(car.normal_filter).add(up.scale(1.0 - car.normal_filter));
            }
            pose = Some((path, pos));
        }
        let (path, pos) = pose.expect("a frame ran");
        Pose { path, pos, head, normal, transform: upright(pos, head, normal) }
    }
}

/// The engine's look-at keeping the up: `forward` is bent square to `up`, the right axis is
/// `up × forward`, and the rows are right, up, forward, position.
pub fn upright(pos: V3, forward: V3, up: V3) -> M4 {
    let right = up.cross(forward);
    let forward = right.cross(up);
    let (up, forward) = (up.norm(), forward.norm());
    let right = up.cross(forward);
    M4([
        [right.x, right.y, right.z, 0.0],
        [up.x, up.y, up.z, 0.0],
        [forward.x, forward.y, forward.z, 0.0],
        [pos.x, pos.y, pos.z, 1.0],
    ])
}

/// Every path car a track drives, ready to be placed: what a ride loads once and asks each tick.
#[derive(Clone, Debug, Default)]
pub struct Traffic {
    pub cars: Vec<(Car, Path)>,
    /// The cars left out, each with why: the ones with a body of their own, and any whose clip is
    /// missing or has no `Body`.
    pub skipped: Vec<String>,
}

impl Traffic {
    /// The cars of the track whose `.toy` is `<track>.toy` in one of these archives. A track with no
    /// `.toy`, or none of whose cars drive a clip, has no traffic. Presampling cars are put on the
    /// ground here, which the original does on their first frame.
    pub fn load(archives: &[crate::disc::Res], track: &str, ground: &mut Ground) -> Traffic {
        let mut traffic = Traffic::default();
        let read = |name: &str| archives.iter().find_map(|a| a.read_suffix(name).ok());
        let Some(text) = read(&format!("{track}.toy")) else { return traffic };
        for car in cars(&ini::text(&text)) {
            if car.has_physics {
                traffic.skipped.push(format!("{}: HasPhysics", car.model));
                continue;
            }
            let clip = read(&car.mot).ok_or_else(|| "no such member".to_string()).and_then(|b| skin::Clip::parse(&b));
            match clip.map(|c| Path::from_clip(&c)) {
                Ok(Some(mut path)) => {
                    if car.presample {
                        path.presample(ground);
                    }
                    traffic.cars.push((car, path));
                }
                Ok(None) => traffic.skipped.push(format!("{}: {} has no Body", car.model, car.mot)),
                Err(e) => traffic.skipped.push(format!("{}: {}: {e}", car.model, car.mot)),
            }
        }
        traffic
    }

    /// The same cars for the simulation (S81), in the same order: each one's path as its clip's keys
    /// are, its numbers, and its `CollisionFileName`'s mesh out of these archives (`None`, and a car
    /// nothing hits, when the file is missing or does not parse; said once). A car whose path the
    /// simulation cannot walk is a car with one key, which the original does not drive either.
    pub fn sim(&self, archives: &[crate::disc::Res]) -> game_sim::traffic::Traffic {
        use ps2_float::{F, Vf};
        let specs = self
            .cars
            .iter()
            .filter_map(|(car, path)| {
                let mesh = archives
                    .iter()
                    .find_map(|a| a.read_suffix(&car.collision).ok())
                    .ok_or_else(|| "no such member".to_string())
                    .and_then(|b| game_sim::track::Mesh::parse(&b));
                let mesh = match mesh {
                    Ok(m) => Some(m),
                    Err(why) => {
                        println!("traffic: {} drives without its {}: {why}", car.model, car.collision);
                        None
                    }
                };
                // The clip's own keys: the simulation's car presamples them itself, on its first frame.
                let points: Vec<(F, Vf)> = path.keys.iter().zip(&path.clip).map(|(k, p)| (F(k.t), Vf::new(p.x, p.y, p.z, 1.0))).collect();
                let f = F;
                let spec = game_sim::traffic::Spec::new(&points, f(car.time_scale), f(car.heading_filter), f(car.normal_filter), f(car.lag), mesh);
                spec.map(|mut spec| {
                    spec.presample = car.presample;
                    spec.level = car.path_is_level;
                    spec
                })
            })
            .collect();
        game_sim::traffic::Traffic::new(specs)
    }

    /// Every car's model and where it is, `elapsed` seconds of the scene's own clock after the track
    /// loaded, its filters settled at the scene's [`SCENE_STEP`].
    pub fn placed(&self, elapsed: f64, ground: &mut Ground) -> Vec<(String, M4)> {
        self.cars
            .iter()
            .map(|(car, path)| {
                let at = clock(elapsed, car.time_scale, path.last());
                let pose = path.pose(car, at, SCENE_STEP * car.time_scale, ground);
                (car.stem().to_string(), pose.transform)
            })
            .collect()
    }
}

/// How long one scene update is: the game's 30 a second. The trucks' headings, which trail the way
/// they move by a filter's worth of updates, agree with the original's at this step and not at half
/// of it.
pub const SCENE_STEP: f32 = 1.0 / 30.0;

/// A path car's wheels (V82): the nodes of its model the original turns as the car drives, and what
/// it turns them by.
///
/// **Which nodes.** The car's init looks its model's tree up (depth first, names compared without
/// case) for `TireFrontL`, `TireFrontR`, `TireBackL`, `TireBackR`, `TireMidL` and `TireMidR`, in that
/// order, and stops at the first it does not find ([`TIRES`]). free06's dump trucks have the first
/// four, its scrapers' fronts and backs the first two; the train's cars have none, and nothing on
/// them turns.
///
/// **The spin.** One angle for every wheel of the car. Each update takes off it the flat length of
/// the step the car's path point made, `sqrt(z² + x²)`, times one over the radius, and wraps it into
/// ±π. The radius is the first wheel node's box: its half height. So a wheel rolls: over a second its
/// angle moves by the feet the car went over the wheel's radius, backwards about its `x` axis for a
/// car going forwards along its `+z`.
///
/// **The steer.** With `SteerFrontTires` (on unless the `.toy` turns it off), the first two wheels
/// also turn about their `y`: the update crosses last update's heading with this one's, takes twenty
/// times the cross's `y` clamped to ±30°, and blends that in by the car's `HeadingFilter`. The rest
/// only spin.
///
/// **The pose.** A wheel's local matrix keeps its file position and gets the basis of a forward
/// `(sin s·cos a, sin a, cos s·cos a)` and an up `(−sin s·sin a, cos a, −cos s·sin a)` (spin `a`,
/// steer `s`, zero for the wheels that only spin), which the original writes over the node's own
/// rotation each update.
///
/// **What is the port's own.** The original keeps the spin and the steer from update to update. Here
/// both come from the car's clock alone, as [`Path::pose`]'s heading does: the spin from the flat
/// length of path the car's point has run since the track loaded ([`Wheels::travelled`], every
/// segment's flat length measured once at load, the laps counted), the steer by replaying
/// [`STEER_WARM`] updates of heading. Neither feeds the simulation; both are drawn only.
#[derive(Clone, Debug)]
pub struct Wheels {
    /// The wheel nodes, in [`TIRES`]' order.
    pub nodes: Vec<usize>,
    /// One over the first wheel's radius (1 where its box has no height, as the original's init).
    pub inv_radius: f32,
    /// `SteerFrontTires`: the first two turn into the bends.
    pub steer: bool,
    /// The flat length of path from the clip's start to each key, and (last) once round the clip and
    /// back to its start: what [`Wheels::travelled`] counts in.
    run: std::sync::Arc<[f64]>,
    /// Every corner of [`skin::skin`]'s triangles a wheel node places — the triangle, the corner and
    /// the vertex as the file has it — and how many triangles there are: what
    /// [`Wheels::reskin`] moves.
    corners: std::sync::Arc<[(usize, usize, crate::psg::Vertex)]>,
    tris: usize,
}

/// The wheel nodes' names, in the order the car's init looks for them.
pub const TIRES: [&str; 6] = ["TireFrontL", "TireFrontR", "TireBackL", "TireBackR", "TireMidL", "TireMidR"];

/// How many updates [`Wheels::steer_at`] replays: the dump trucks' `HeadingFilter` is 0.9, which
/// keeps 0.9⁹⁶ ≈ 4·10⁻⁵ of where it started.
pub const STEER_WARM: usize = 96;

/// The steer's stop: 30°, as the original's float.
pub const STEER_STOP: f32 = 0.523_598_73;

/// How many chords a segment's flat length is measured in.
const RUN_CHORDS: usize = 10;

impl Wheels {
    /// The wheels of `car`'s `model` on `path`, or `None` for a model with no `TireFrontL`.
    pub fn new(model: &crate::psg::Model, car: &Car, path: &Path) -> Option<Wheels> {
        let mut nodes = Vec::new();
        for name in TIRES {
            match model.nodes.iter().position(|n| n.name.eq_ignore_ascii_case(name)) {
                Some(i) => nodes.push(i),
                None => break,
            }
        }
        let first = *nodes.first()?;
        let half = model.nodes[first].half_extents.y;
        let inv_radius = if half > 0.0 { 1.0 / half } else { 1.0 };
        let mut run = Vec::with_capacity(path.keys.len() + 1);
        let mut total = 0.0f64;
        run.push(0.0);
        for i in 0..path.keys.len() - 1 {
            total += path.flat_run(path.keys[i].t, path.keys[i + 1].t);
            run.push(total);
        }
        // Round the end of the clip: the step from its last point back to its first.
        total += f64::from(flat(path.at(path.last()).sub(path.at(path.keys[0].t))).len());
        run.push(total);
        // The corners, in the order `skin::skin` expands the model's first LOD.
        let mut corners = Vec::new();
        let mut tris = 0;
        for mesh in model.meshes.iter().filter(|m| m.group == 0) {
            for strip in &mesh.strips {
                for i in 2..strip.verts.len() {
                    if strip.verts[i].adc {
                        continue;
                    }
                    for (c, v) in strip.verts[i - 2..=i].iter().enumerate() {
                        if nodes.contains(&v.node) {
                            corners.push((tris, c, *v));
                        }
                    }
                    tris += 1;
                }
            }
        }
        Some(Wheels { nodes, inv_radius, steer: car.steer_front, run: run.into(), corners: corners.into(), tris })
    }

    /// The flat feet `car`'s point on the path has run, `elapsed` seconds of the scene's clock after
    /// the track loaded.
    pub fn travelled(&self, car: &Car, path: &Path, elapsed: f64) -> f64 {
        let last = path.last();
        if last <= 0.0 {
            return 0.0;
        }
        let scaled = elapsed * f64::from(car.time_scale);
        let mut laps = (scaled / f64::from(last)).floor();
        let at = clock(elapsed, car.time_scale, last);
        let s = if car.lag > 0.0 { path.lagged(at, car.lag) } else { at };
        // A wagon whose lag reaches back past the clip's start is a lap behind its clock.
        if s > at {
            laps -= 1.0;
        }
        let i = path.segment(s);
        let into = self.run[i] + path.flat_run(path.keys[i].t, s);
        laps * self.run[path.keys.len()] + into
    }

    /// The spin `elapsed` seconds in: minus the flat feet run over the radius, in ±π.
    pub fn spin_at(&self, car: &Car, path: &Path, elapsed: f64) -> f32 {
        let a = -self.travelled(car, path, elapsed) * f64::from(self.inv_radius);
        let a = a.rem_euclid(std::f64::consts::TAU);
        (if a > std::f64::consts::PI { a - std::f64::consts::TAU } else { a }) as f32
    }

    /// The front wheels' steer at the car's clock `clock`, replaying [`STEER_WARM`] updates of `step`
    /// clip seconds: zero for a car that does not steer.
    pub fn steer_at(&self, car: &Car, path: &Path, clock: f32, step: f32) -> f32 {
        if !self.steer {
            return 0.0;
        }
        let period = path.last();
        let wrap = |s: f32| if period > 0.0 { s.rem_euclid(period) } else { 0.0 };
        let hf = car.heading_filter;
        let (mut head, mut was) = (v3(0.0, 0.0, 0.0), v3(0.0, 0.0, 0.0));
        let mut before: Option<V3> = None;
        let mut steer = 0.0f32;
        for back in (0..=STEER_WARM).rev() {
            let s = wrap(clock - back as f32 * step);
            let s = if car.lag > 0.0 { path.lagged(s, car.lag) } else { s };
            let point = path.at(s);
            if let Some(before) = before {
                head = head.scale(hf).add(flat(point.sub(before)).norm().scale(1.0 - hf));
            }
            before = Some(point);
            let target = (20.0 * was.cross(head).y).clamp(-STEER_STOP, STEER_STOP);
            steer = (1.0 - hf) * target + hf * steer;
            was = head;
        }
        steer
    }

    /// A wheel's local matrix at spin `spin` and steer `steer`, standing at `at` in its parent.
    pub fn local(at: V3, spin: f32, steer: f32) -> M4 {
        let (sa, ca) = spin.sin_cos();
        let (ss, cs) = steer.sin_cos();
        let forward = v3(ss * ca, sa, cs * ca);
        let up = v3(-ss * sa, ca, -cs * sa);
        upright(at, forward, up)
    }

    /// Every node of `model` placed in the model's space with the wheels at `spin` and the front pair
    /// at `steer`: the palette both renderers skin the car with.
    pub fn palette(&self, model: &crate::psg::Model, spin: f32, steer: f32) -> Vec<M4> {
        let locals: Vec<M4> = model
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| match self.nodes.iter().position(|&w| w == i) {
                Some(k) => {
                    let t = n.local.0[3];
                    Wheels::local(v3(t[0], t[1], t[2]), spin, if self.steer && k < 2 { steer } else { 0.0 })
                }
                None => n.local,
            })
            .collect();
        let mut out: Vec<Option<M4>> = vec![None; locals.len()];
        for i in 0..locals.len() {
            place_node(model, &locals, &mut out, i, 0);
        }
        out.into_iter().map(|m| m.unwrap_or(M4::IDENTITY)).collect()
    }

    /// `skin::skin(model, palette, 0)` where `tris` already holds it for another palette that differs
    /// only at the wheels: the wheels' corners placed again, the rest left. Skins the whole model when
    /// `tris` is not this model's.
    pub fn reskin(&self, model: &crate::psg::Model, palette: &[M4], tris: &mut Vec<crate::psg::Tri>) {
        if tris.len() != self.tris {
            *tris = skin::skin(model, palette, 0);
            return;
        }
        for &(t, c, v) in self.corners.iter() {
            if let Some(m) = palette.get(v.node) {
                let out = &mut tris[t].v[c];
                out.pos = m.point(v.pos);
                out.normal = m.dir(v.normal);
            }
        }
    }

    /// The palette `elapsed` seconds in, the car's steer settled at the scene's [`SCENE_STEP`].
    pub fn palette_at(&self, model: &crate::psg::Model, car: &Car, path: &Path, elapsed: f64) -> Vec<M4> {
        let at = clock(elapsed, car.time_scale, path.last());
        let steer = self.steer_at(car, path, at, SCENE_STEP * car.time_scale);
        self.palette(model, self.spin_at(car, path, elapsed), steer)
    }
}

/// A node's place in its model: its local under its parent's place, parents first.
fn place_node(model: &crate::psg::Model, locals: &[M4], out: &mut [Option<M4>], i: usize, depth: usize) -> M4 {
    if let Some(m) = out[i] {
        return m;
    }
    let m = match model.nodes[i].parent {
        Some(p) if p < locals.len() && p != i && depth <= locals.len() => {
            locals[i].mul(place_node(model, locals, out, p, depth + 1))
        }
        _ => locals[i],
    };
    out[i] = Some(m);
    m
}

/// `v` with its height dropped.
fn flat(v: V3) -> V3 {
    v3(v.x, 0.0, v.z)
}

impl Path {
    /// The flat length of the path between clip times `from` and `to` in one segment, in
    /// [`RUN_CHORDS`] chords.
    fn flat_run(&self, from: f32, to: f32) -> f64 {
        if to <= from {
            return 0.0;
        }
        let mut before = self.at(from);
        let mut run = 0.0f64;
        for k in 1..=RUN_CHORDS {
            let now = self.at(from + (to - from) * k as f32 / RUN_CHORDS as f32);
            run += f64::from(flat(now.sub(before)).len());
            before = now;
        }
        run
    }
}

/// One `[ParticleEmitterN]` of a `.toy` (V45): a smoke, a fire or a mist the track makes on its own,
/// as the original's particle emitter toy reads it.
///
/// **The list.** `[ParticleEmitters] NumberOfParticleEmitters = n`, then `ParticleEmitter1` to `n`.
/// The count is all the reader goes by: free07 lists 27 emitters, its fires and smokes, under a count
/// of 0, and so has none.
///
/// **The section.** `ParticleType` names the emitter's class, ignoring case: `SMOKE` 0 (and the
/// default), then `DUST`, `SPARKS`, `STEAM`, `DIRTSPRAY`, `DIRTCHUNKS`, `WATERSPRAY`, `WATERROOST`,
/// `FIRE`, `EXPLOSION`, `BLOOD`, `FIREWORKS`, `FOG`, `SNOW` and `RING` 1 to 14. A name that is none of
/// them is 0 as well — which is what the disc's `Generic` is. Types 0, 1, 2, 8 and 9 build the plain
/// emitter the plane's and the bike's `Generic` entries build; 11, 12 and 14 classes of their own; the
/// rest none. Every emitter on the disc is `Generic` or `Smoke`. `StartOn` (on where it is missing)
/// switches it on as it is read. Where it is: on the object `AttachToSceneRef` names, else on the
/// toy `AttachToToy` names (its `AttachToObject`, its `AttachToSubObject`), else on the scene object
/// `AttachToObject` names, else at its own `Position` — the toy's, read by every toy's base. The
/// `.emt` is `EmitterFilename`.
#[derive(Clone, Debug, PartialEq)]
pub struct Emitter {
    /// `Name`, which a trigger switches it by.
    pub name: String,
    /// `ParticleType` as the original's class number.
    pub kind: u8,
    /// `StartOn`.
    pub start_on: bool,
    /// `Position`, in the track's feet.
    pub position: V3,
    /// Any of the `AttachTo` keys: it rides an object, not its `Position`.
    pub attached: bool,
    /// `EmitterFilename`.
    pub file: String,
}

impl Emitter {
    /// The class numbers that build the plain emitter.
    pub const PLAIN: [u8; 5] = [0, 1, 2, 8, 9];

    /// It builds the plain emitter ([`Emitter::PLAIN`]).
    pub fn is_plain(&self) -> bool {
        Emitter::PLAIN.contains(&self.kind)
    }
}

/// `ParticleType`'s class number, in the original's order of tests ([`Emitter`]).
pub fn particle_type(name: &str) -> u8 {
    const NAMES: [&str; 15] = [
        "SMOKE", "DUST", "SPARKS", "STEAM", "DIRTSPRAY", "DIRTCHUNKS", "WATERSPRAY", "WATERROOST", "FIRE", "EXPLOSION", "BLOOD",
        "FIREWORKS", "FOG", "SNOW", "RING",
    ];
    NAMES.iter().position(|n| n.eq_ignore_ascii_case(name.trim())).unwrap_or(0) as u8
}

/// The emitters a `.toy` lists, `ParticleEmitter1` to the count its `[ParticleEmitters]` says.
pub fn emitters(text: &str) -> Vec<Emitter> {
    let sections = ini::parse(text);
    let find = |name: &str| sections.iter().find(|s| s.name.eq_ignore_ascii_case(name));
    let count = find("ParticleEmitters").and_then(|s| s.number("NumberOfParticleEmitters")).unwrap_or(0.0).max(0.0) as usize;
    (1..=count).filter_map(|i| find(&format!("ParticleEmitter{i}")).map(emitter)).collect()
}

fn emitter(s: &Section) -> Emitter {
    let text = |key: &str| s.get(key).unwrap_or("").trim().to_string();
    let position = s.numbers("Position").filter(|n| n.len() >= 3).map_or(v3(0.0, 0.0, 0.0), |n| v3(n[0], n[1], n[2]));
    Emitter {
        name: text("Name"),
        kind: s.get("ParticleType").map_or(0, particle_type),
        start_on: s.get("StartOn").is_none_or(ini::switch),
        position,
        attached: ["AttachToSceneRef", "AttachToToy", "AttachToObject"].iter().any(|k| s.get(k).is_some()),
        file: text("EmitterFilename"),
    }
}
