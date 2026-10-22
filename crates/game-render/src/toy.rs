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
    /// `PathIsLevel`. Off for every car on the disc; the path it takes when on is not ported.
    pub path_is_level: bool,
    /// `PresampleTerrain`: snap the keys to the ground once instead of the car every frame.
    pub presample: bool,
    /// `DriveOnSoultree`. Off for every car on the disc; the ground ray it asks for is not ported.
    pub drive_on_soultree: bool,
    /// `HasPhysics`: another class altogether, a vehicle with a body. Not drawn by this module.
    pub has_physics: bool,
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
    Car {
        model: text("FileName"),
        collision: text("CollisionFileName"),
        mot: text("MOTFileName"),
        time_scale: s.number("TimeScale").unwrap_or(1.0),
        heading_filter: s.number("HeadingFilter").unwrap_or(0.5),
        normal_filter: s.number("NormalFilter").unwrap_or(0.2),
        lag: s.number("LagDistance").unwrap_or(0.0),
        path_is_level: flag("PathIsLevel"),
        presample: flag("PresampleTerrain"),
        drive_on_soultree: flag("DriveOnSoultree"),
        has_physics: flag("HasPhysics"),
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
        let keys = points.into_iter().map(|(t, pos)| Key { t, len: 0.0, pos, normal: v3(0.0, 1.0, 0.0) }).collect();
        let mut path = Path { keys };
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
