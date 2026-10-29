//! The track's path cars (S81): the `.toy`'s cars that drive along a clip's `Body` channel — free06's
//! train, its dump trucks and scrapers, the other freeride tracks' trailers — moved in the simulation,
//! so that their collision meshes stand where the original's do on every tick.
//!
//! The original's car is a scene node (the path car class, `research/oracle.txt` S81) updated by the
//! scene tree's walk with the frame's `dt`, before the dynamics step: S99's trace has the plane's
//! search gather, in the step's post listeners, already seeing every path car at this tick's place.
//! Each update:
//!
//! 1. **The clock**: `fmodf(clock + dt·TimeScale, last key's time)`, and again if it is still past
//!    the last key. Zero when the clip is loaded, which is when the scene is.
//! 2. **The point**: the path's Catmull-Rom curve ([`Spec::curve`]) at the clock, or, for a car with a
//!    `LagDistance` (the train's wagons), at the time that lies that many feet of arc behind it
//!    ([`Spec::lagged`]) — measured on the segments' lengths, each the sum of ten chords
//!    ([`Spec::measure`]).
//! 3. **The heading**: the step from last tick's point, flattened and normalised, blended into the
//!    old heading by `HeadingFilter`.
//! 4. **The ground**: the terrain's height and normal under the point ([`World::ground`]); the normal
//!    blended into the old one by `NormalFilter` while the two are within 45° (a cosine above
//!    0.707), and kept as it was past that. Two `.toy` switches change where the ground comes from
//!    (S128):
//!    - `PresampleTerrain` (free09's and free11's trailers): on the car's first frame every key of
//!      its path is put on the ground once, with the ground's normal there ([`Spec::presample`]),
//!      and from then on the height is the curve's own and the normal the segment's two key normals
//!      blended at the curve's parameter. No ground query after the first frame;
//!    - `PathIsLevel`: the ground's height is written into the path each frame until two frames in a
//!      row find the same, and from then on the car rides at that height with straight up for its up
//!      ([`Level`]). The up is the ground's own, unfiltered, and the look-at keeps it as it is.
//! 5. **The pose**: the engine's look-at ([`math::basis`], up kept, forward made square to it) at
//!    the point with the ground's height, and the car's collision mesh moved there.
//!
//! The cars are state ([`Traffic::cars`], hashed and sent) so a rollback or a replay moves them with
//! everything else; their paths and numbers are a constant of the track ([`Traffic::specs`]).
//!
//! The curve keeps its two tangents between evaluations and makes them again only when its cursor
//! moves onto another segment. A presampled car's keys move after its path was measured, so until
//! its cursor first leaves the segment the measuring left it on, its curve rides that segment on the
//! tangents of the clip's own keys ([`Car::stale`], [`Spec::measured`]).
//!
//! Not here: `DriveOnSoultree`'s ray down onto the props, which every car on the disc sets `FALSE`
//! (the original's default is `TRUE`); a car that sets it rides the terrain alone.
//!
//! The `HasPhysics` cars (S127) are a subclass of the path car with a rigid body beside it. Until
//! something strikes the car, the subclass's update is the path car's own and nothing else, so they
//! are cars here like the rest, in the `.toy`'s order. What the original does once one is struck
//! (the body wakes with the car's motion, the car's own meshes stand down, and it goes back on its
//! path a `ResetTime` after it comes to rest) is not here.

#![deny(clippy::float_arithmetic)] // every sum below is the PS2's, through ps2-float

use std::sync::{Arc, OnceLock};

use ps2_float::{Acc, Dest, F, Vf, libm, vu};

use crate::hash::{State, Walk};
use crate::math::{self, Mat4};
use crate::track::Mesh;
use crate::world::{Prop, Props, World};

/// One key of a path: its time on the clip, the arc length to the next key, and where it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key {
    pub t: F,
    /// The segment's length to the next key ([`Spec::measure`]); the last key's is zero.
    pub len: F,
    /// The `Body` channel's position, `w` 1; a presampled key's is on the ground.
    pub pos: Vf,
    /// The ground's normal under a presampled key ([`Spec::presample`]); straight up otherwise,
    /// and read only by a presampled car.
    pub normal: Vf,
}

/// A car's constants: its path and its `.toy` numbers. Built once, when the track is.
#[derive(Clone, Debug)]
pub struct Spec {
    pub keys: Vec<Key>,
    /// `TimeScale`: the clip's seconds per second of the ride.
    pub time_scale: F,
    /// `HeadingFilter` (0.5 when the `.toy` leaves it out): how much of last tick's heading stays.
    pub heading_filter: F,
    /// `NormalFilter` (0.2): how much of last tick's up stays.
    pub normal_filter: F,
    /// `LagDistance`: feet of arc behind the clock. Only a positive one is walked.
    pub lag: F,
    /// The car's `CollisionFileName`, which stands where the car does. `None` drives a car nobody hits.
    pub mesh: Option<Mesh>,
    /// Which segment the measuring left the curve's cursor on: where a fresh car's cursor starts.
    pub cursor: u32,
    /// The curve's two tangents on that segment, as the measuring left them: what a car whose cursor
    /// has not moved yet rides on ([`Car::stale`]).
    pub measured: [Vf; 2],
    /// `PresampleTerrain`: put the keys on the ground on the first frame, and ride them.
    pub presample: bool,
    /// `PathIsLevel`: ride the path at one height, found on the ground ([`Level`]).
    pub level: bool,
    /// The presampled keys, made on the first frame that steps a car ([`Spec::presample`]). The
    /// terrain is the same for every seat and every frame, so once made they stand for good.
    sampled: OnceLock<Vec<Key>>,
}

/// A `PathIsLevel` car's height: the path's (`path+0x10`), the one before it (`+0x568`, the
/// float's most negative at the car's construction) and whether the two have met (`+0x4d4`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    pub y: F,
    pub last: F,
    pub held: bool,
}

impl Default for Level {
    fn default() -> Level {
        Level { y: F::ZERO, last: F(f32::from_bits(0xff7f_ffff)), held: false }
    }
}

/// What a car is doing: the state its update reads and writes.
#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    /// Seconds along the clip (the path's `+0xc`).
    pub clock: F,
    /// The curve's cursor: the segment the last evaluation found (`+0x4f4`). Both the next search and
    /// the lag's walk start from it, and at a key's own time the direction a search comes from picks
    /// the segment, so it is state.
    pub seg: u32,
    /// Last tick's point on the path (`+0x450`), which the heading is the step from.
    pub point: Vf,
    /// The heading (`+0x480`): the filtered direction of travel on the ground plane.
    pub head: Vf,
    /// The up (`+0x4a0`): the filtered ground normal.
    pub normal: Vf,
    /// Set when the next update takes the heading and the normal as they are, unfiltered (`+0x4e4`).
    /// Nothing on a ride sets it.
    pub first: bool,
    /// Where the car stands: its model node's world matrix after the update, which its mesh is at.
    pub pose: Mat4,
    /// The curve's cursor has not left the segment the measuring left it on, so the curve still
    /// rides the tangents [`Spec::measured`] made of the clip's own keys (the curve's `+0x4fc` not
    /// set since). Only a presampled car's keys move after the measuring, so only its can differ.
    pub stale: bool,
    /// A `PathIsLevel` car's height.
    pub level: Level,
}

impl Car {
    /// A car as the scene's load leaves it: clock 0, the point at the origin, heading nowhere, up up.
    pub fn new(spec: &Spec) -> Car {
        Car {
            clock: F::ZERO,
            seg: spec.cursor,
            point: Vf::new(0.0, 0.0, 0.0, 1.0),
            head: Vf::new(0.0, 0.0, 0.0, 1.0),
            normal: Vf::new(0.0, 1.0, 0.0, 1.0),
            first: false,
            pose: Mat4([
                Vf::new(1.0, 0.0, 0.0, 0.0),
                Vf::new(0.0, 1.0, 0.0, 0.0),
                Vf::new(0.0, 0.0, 1.0, 0.0),
                Vf::new(0.0, 0.0, 0.0, 1.0),
            ]),
            stale: true,
            level: Level::default(),
        }
    }
}

/// The curve's cached segment: the cursor and its keys.
struct Cursor {
    i: usize,
    j: usize,
}

impl Spec {
    /// A car on a clip's `Body` keys, `(time, position)`, measured. `None` with fewer than two keys:
    /// a path the original's curve cannot walk.
    pub fn new(points: &[(F, Vf)], time_scale: F, heading_filter: F, normal_filter: F, lag: F, mesh: Option<Mesh>) -> Option<Spec> {
        if points.len() < 2 {
            return None;
        }
        let up = Vf::new(0.0, 1.0, 0.0, 1.0);
        let keys = points.iter().map(|&(t, p)| Key { t, len: F::ZERO, pos: Vf::new(p.x(), p.y(), p.z(), 1.0), normal: up }).collect();
        let none = [Vf::ZERO; 2];
        let mut spec = Spec { keys, time_scale, heading_filter, normal_filter, lag, mesh, cursor: 0, measured: none, presample: false, level: false, sampled: OnceLock::new() };
        spec.cursor = spec.measure(0);
        let c = spec.find(spec.cursor, spec.keys[spec.cursor as usize].t);
        spec.measured = spec.tangents(&c);
        Some(spec)
    }

    /// The keys the curve rides: the presampled ones once they are made, the clip's before.
    pub fn keys(&self) -> &[Key] {
        self.sampled.get().map_or(&self.keys, |k| k)
    }

    /// `PresampleTerrain`'s first frame: every key put on the ground with the ground's normal there
    /// (the ground query on the key's position and normal, in place). Their lengths stay the
    /// measured ones. Done once; a later call, or a car that does not presample, changes nothing.
    pub fn presample(&self, world: &mut dyn World) {
        if !self.presample || self.sampled.get().is_some() {
            return;
        }
        let keys = self
            .keys
            .iter()
            .map(|k| {
                let (on, normal) = world.ground(k.pos);
                Key { pos: on, normal, ..*k }
            })
            .collect();
        let _ = self.sampled.set(keys);
    }

    /// The spec with its keys already presampled to these, their lengths and times as given: a
    /// presampling car caught after its first frame, as a RAM image holds it.
    pub fn presampled(mut self, keys: Vec<Key>) -> Spec {
        self.presample = true;
        self.sampled = OnceLock::from(keys);
        self
    }

    /// The arc length of every segment, from a cursor at `seg`, which it answers where it leaves:
    /// per segment, eleven points of the curve a tenth of the segment's time apart, and the lengths
    /// of the ten chords between them summed (`sqrt.s` of `mula`/`madda`/`madd`). The last key's
    /// length is zero.
    pub fn measure(&mut self, seg: u32) -> u32 {
        let n = self.keys.len();
        let mut seg = seg;
        let tenth = F(0.1);
        for i in 0..n - 1 {
            self.keys[i].len = F::ZERO;
            let t0 = self.keys[i].t;
            let span = self.keys[i + 1].t - t0;
            let mut before = self.curve(&mut seg, t0).0;
            for k in 1..=10i32 {
                let now = self.curve(&mut seg, t0 + span * (F::from(k) * tenth)).0;
                let d = now.merge(Dest::XYZ, now.sub(before));
                let chord = Acc::mula(d.x(), d.x()).madda(d.y(), d.y()).madd(d.z(), d.z()).sqrt();
                self.keys[i].len = self.keys[i].len + chord;
                before = now;
            }
        }
        self.keys[n - 1].len = F::ZERO;
        seg
    }

    /// The last key's time: where the clock wraps.
    pub fn last(&self) -> F {
        self.keys[self.keys.len() - 1].t
    }

    /// Move the cursor onto the segment `s` falls in: forward while `s` is past the segment's end
    /// (and there is a key after it), then back while it is before the segment's start.
    fn find(&self, seg: u32, s: F) -> Cursor {
        let keys = self.keys();
        let n = keys.len();
        let mut i = (seg as usize).min(n - 2);
        let mut j = i + 1;
        while j < n && !(s <= keys[j].t) {
            i += 1;
            j += 1;
        }
        // Past the last key the original reads one key beyond its array; a clock wrapped by the last
        // key's time never gets there.
        if j >= n {
            i = n - 2;
            j = n - 1;
        }
        while i != 0 && s < keys[i].t {
            i -= 1;
            j -= 1;
        }
        Cursor { i, j }
    }

    /// [`Spec::lagged`]'s walk to `s`'s segment from the cursor at `seg`: forward as the curve's, back
    /// while `s` is before the time of the key at `seg`.
    fn lag_find(&self, seg: u32, s: F) -> Cursor {
        let n = self.keys.len();
        let from = (seg as usize).min(n - 2);
        let mut i = from;
        let mut j = i + 1;
        while j < n && !(s <= self.keys[j].t) {
            i += 1;
            j += 1;
        }
        // Past the last key the original reads one key beyond its array, as the curve's walk does.
        if j >= n {
            i = n - 2;
            j = n - 1;
        }
        while i != 0 && s < self.keys[from].t {
            i -= 1;
            j -= 1;
        }
        Cursor { i, j }
    }

    /// The curve's two tangents on the cursor's segment: Catmull-Rom, each the chord and its
    /// neighbour summed and scaled by the time spans (the chord alone at either end of the path).
    fn tangents(&self, c: &Cursor) -> [Vf; 2] {
        let keys = self.keys();
        let n = keys.len();
        let (ki, kj) = (keys[c.i], keys[c.j]);
        let chord = sub3(kj.pos, ki.pos);
        let m0 = if c.i == 0 {
            chord
        } else {
            let kh = keys[c.i - 1];
            let f = (kj.t - ki.t) / (kj.t - kh.t);
            scale3(add3(sub3(ki.pos, kh.pos), chord), f)
        };
        let m1 = if c.j == n - 1 {
            chord
        } else {
            let kk = keys[c.j + 1];
            let f = (kj.t - ki.t) / (kk.t - ki.t);
            scale3(add3(chord, sub3(kk.pos, kj.pos)), f)
        };
        [m0, m1]
    }

    /// The point on the path at `s`, and the segment's own parameter there, from the cursor at `seg`,
    /// which moves onto `s`'s segment: the cubic Hermite through the segment's two keys with
    /// Catmull-Rom tangents ([`Spec::tangents`]).
    pub fn curve(&self, seg: &mut u32, s: F) -> (Vf, F) {
        self.curve_kept(seg, s, &mut false)
    }

    /// [`Spec::curve`] as a car's curve runs it, its tangents kept between evaluations: while
    /// `stale`, a cursor that does not move rides [`Spec::measured`]; a cursor that moves makes them
    /// again and clears it.
    pub fn curve_kept(&self, seg: &mut u32, s: F, stale: &mut bool) -> (Vf, F) {
        let keys = self.keys();
        let c = self.find(*seg, s);
        if c.i as u32 != *seg {
            *stale = false;
        }
        *seg = c.i as u32;
        let (ki, kj) = (keys[c.i], keys[c.j]);
        let t = (s - ki.t) / (kj.t - ki.t);
        let t2 = t * t;
        let t3 = t * t2;
        let h = (F(3.0) * t2 - t3) - t3;
        let one_h = F::ONE - h;
        let w1 = t3 - t2;
        let w0 = t + (w1 - t2);
        let [m0, m1] = if *stale { self.measured } else { self.tangents(&c) };
        let a = scale3(m1, w1);
        let b = scale3(m0, w0);
        let cc = scale3(kj.pos, h);
        let d = scale3(ki.pos, one_h);
        (add3(add3(add3(d, cc), b), a), t)
    }

    /// The clip time `lag` feet of arc behind `s`: from the cursor's segment (which this does not
    /// move), the arc already covered in it, then whole segments back (wrapping past the first key
    /// onto the last) until the lag is used up, and the rest as a fraction of that segment's time.
    /// Zero when the segment it ends in has no length.
    ///
    /// Its own walk to `s`'s segment is not the curve's: walking back it tests `s` against the time
    /// of the key the cursor started on, not the key it has reached, so a step back goes all the way
    /// to the first key, and the arc covered is that segment's, extrapolated past its end.
    pub fn lagged(&self, seg: u32, s: F, lag: F) -> F {
        let n = self.keys.len();
        let c = self.lag_find(seg, s);
        let mut i = c.i;
        let mut along = ((s - self.keys[i].t) / (self.keys[c.j].t - self.keys[i].t)) * self.keys[i].len;
        while along < lag {
            i = if i == 0 { n - 1 } else { i - 1 };
            along = along + self.keys[i].len;
        }
        let rest = along - lag;
        let len = self.keys[i].len;
        if len == F::ZERO {
            return F::ZERO;
        }
        let next = (i + 1) % n;
        self.keys[i].t + (rest / len) * (self.keys[next].t - self.keys[i].t)
    }

    /// One update of a car (the path car's update, `research/oracle.txt` S81): the module's five
    /// steps. A `dt` that is not positive moves nothing.
    pub fn step(&self, car: &mut Car, dt: F, world: &mut dyn World) {
        if dt <= F::ZERO {
            return;
        }
        let last = self.last();
        car.clock = F(libm::fmodf((car.clock + dt * self.time_scale).0, last.0));
        if !(car.clock <= last) {
            car.clock = F(libm::fmodf(car.clock.0, last.0));
        }
        // The first frame's presample comes after the clock and before the curve.
        self.presample(world);
        let s = if self.lag <= F::ZERO { car.clock } else { self.lagged(car.seg, car.clock, self.lag) };
        let (point, t) = self.curve_kept(&mut car.seg, s, &mut car.stale);
        // The step since last tick on the ground plane, normalised by VU0's `vrsqrt` (a car that did
        // not move gets a zero direction, not a NaN: 0 times the divider's max).
        let moved = sub3(point, car.point);
        car.point = point;
        let flat = Vf::new(moved.x(), 0.0, moved.z(), moved.w());
        let q = vu::rsqrt(1.0, vu::dot3(flat.0, flat.0));
        let dir = flat.merge(Dest::XYZ, flat.mul_q(q));
        car.head = if car.first {
            dir
        } else {
            let hf = self.heading_filter;
            add3(scale3(car.head, hf), scale3(dir, F::ONE - hf))
        };
        let (y, up) = if self.presample {
            // The curve's own height, and the segment's two key normals blended at its parameter
            // lane by lane on the FPU (`sub.s`, `mul.s`, `add.s`), the ends taken as they are. The
            // `w` is whatever the original's stack held: the up's own is kept.
            let keys = self.keys();
            let (a, b) = (keys[car.seg as usize].normal, keys[car.seg as usize + 1].normal);
            let lane = |a: F, b: F| if t <= F::ZERO { a } else if t < F::ONE { a + t * (b - a) } else { b };
            let lane = |a: f32, b: f32| lane(F(a), F(b)).0;
            (F(point.y()), Vf::new(lane(a.x(), b.x()), lane(a.y(), b.y()), lane(a.z(), b.z()), car.normal.w()))
        } else if car.level.held {
            (car.level.y, Vf::new(0.0, 1.0, 0.0, 1.0))
        } else {
            // A `PathIsLevel` car on a path that does not loop skips the query (and rides an up the
            // original never set); every path car the port drives loops.
            let (ground, up) = world.ground(point);
            if self.level {
                car.level.y = F(ground.y());
                if car.level.y.0 == car.level.last.0 {
                    car.level.held = true;
                } else {
                    car.level.last = car.level.y;
                }
            }
            (F(ground.y()), up)
        };
        let at = Vf::new(point.x(), y.0, point.z(), 1.0);
        if self.level {
            // The level car's up is the ground's, unfiltered and not kept, and the look-at takes the
            // two axes as they are.
            car.pose = math::basis(at, car.head, up, false, true);
            return;
        }
        if car.first {
            car.normal = up;
        } else {
            let old = car.normal;
            let cos = Acc::mula(old.x(), up.x()).madda(old.y(), up.y()).madd(old.z(), up.z());
            if !(cos <= F(f32::from_bits(0x3f34_fdf4))) {
                let nf = self.normal_filter;
                car.normal = add3(scale3(old, nf), scale3(up, F::ONE - nf));
            }
        }
        car.pose = math::basis(at, car.head, car.normal, true, false);
    }
}

/// The track's path cars: their constants, their state, and where their meshes are in the world.
#[derive(Clone, Default)]
pub struct Traffic {
    /// Every car's path and numbers: a constant of the track, shared by every snapshot, not hashed.
    pub specs: Arc<Vec<Spec>>,
    /// Every car's state, in the `.toy`'s order.
    pub cars: Vec<Car>,
    /// Where the cars' meshes begin in the world's props ([`Traffic::attach`]), the cars with a mesh
    /// one after another in order. A constant like the specs.
    pub first: Option<usize>,
    /// The track's animated toys (V43), stepped with the cars.
    pub toys: crate::toys::Toys,
    /// The track's knockable props (S131), ticked with the cars.
    pub props: crate::prop_set::PropSet,
}

impl Traffic {
    /// The cars of a track, as its load leaves them.
    pub fn new(specs: Vec<Spec>) -> Traffic {
        let cars = specs.iter().map(Car::new).collect();
        Traffic { specs: Arc::new(specs), cars, first: None, toys: Default::default(), props: Default::default() }
    }

    /// Put the cars' meshes at the end of the world's props, each where its car stands now, and
    /// remember where they begin: the props [`Traffic::frame`] moves.
    pub fn attach(&mut self, props: &mut Props) {
        self.first = Some(props.props.len());
        for (spec, car) in self.specs.iter().zip(&self.cars) {
            if let Some(mesh) = &spec.mesh {
                props.props.push(Prop::on(mesh.clone(), &car.pose));
            }
        }
        self.toys.attach(props);
    }

    /// One tick of every car, in order, and their meshes moved to where they now stand.
    pub fn frame(&mut self, dt: F, world: &mut dyn World) {
        self.toys.frame(dt, world);
        if self.props.collections.iter().any(|c| c.objects.iter().any(|o| o.filed)) {
            let (field, props, trees) = world.wooded();
            self.props.frame_among(dt, field.as_deref(), props, trees);
        } else {
            self.props.frame(dt);
        }
        if self.cars.is_empty() {
            return;
        }
        let specs = Arc::clone(&self.specs);
        let before: Vec<Mat4> = self.cars.iter().map(|car| car.pose).collect();
        for (spec, car) in specs.iter().zip(&mut self.cars) {
            spec.step(car, dt, world);
        }
        self.place(world);
        self.mark(&before, dt, world);
    }

    /// Tell each car's mesh how its car moved this tick ([`Moving`]), for a wheel on it to ride with
    /// it (S136). `before` is every car's pose before the step.
    ///
    /// The original's `1/dt` is the frame's own (`app+0x348`, made by one `div.s` from the frame's
    /// step at `app+0x344` and left alone by a step that is not positive). A step that is not
    /// positive moves no car here, so its velocity is zero whatever the divider: zero is used.
    fn mark(&self, before: &[Mat4], dt: F, world: &mut dyn World) {
        let (Some(first), Some(props)) = (self.first, world.moving()) else {
            return;
        };
        let inv_dt = if dt > F::ZERO { F::ONE / dt } else { F::ZERO };
        let mut k = first;
        for ((spec, car), was) in self.specs.iter().zip(&self.cars).zip(before) {
            if spec.mesh.is_some() {
                if let Some(prop) = props.props.get_mut(k) {
                    prop.moving = Some(Moving::of(was, &car.pose, inv_dt));
                }
                k += 1;
            }
        }
    }

    /// Where the cars' meshes lie in the world's props, `[first, end)`: [`Traffic::first`] and one
    /// prop a car with a mesh. `None` before [`Traffic::attach`].
    pub fn span(&self) -> Option<[usize; 2]> {
        let first = self.first?;
        Some([first, first + self.specs.iter().filter(|s| s.mesh.is_some()).count()])
    }

    /// Stand the cars' meshes where the cars are: after a step, and after a state is put back.
    pub fn place(&self, world: &mut dyn World) {
        let (Some(first), Some(props)) = (self.first, world.moving()) else {
            return;
        };
        let mut k = first;
        for (spec, car) in self.specs.iter().zip(&self.cars) {
            if spec.mesh.is_some() {
                if let Some(prop) = props.props.get_mut(k) {
                    prop.stand(&car.pose);
                }
                k += 1;
            }
        }
    }
}

/// **The cars a ghost's frame does not see** (S215): their meshes, taken out of the world's props by
/// [`hide`] and put back by [`show`].
///
/// The original's put-back adds the vehicle category to the ignore lists of the bike's body rays and
/// sweep (S63), and the pair filter drops every candidate whose category holds it: the path cars'
/// meshes are category `0x48` (`research/oracle.txt` S129), so a ghost's search gathers no car and its
/// rays, sweep and wheels never meet one. The port's searches read the cars off the world's props,
/// so for the ghost's frame each car's prop is left standing where it is with no mesh in it, no
/// reach and no motion: every sweep, ray probe, broad phase and the bones' search finds nothing
/// there, and the props after them keep their places. The wreck bike and the ragdoll miss them too,
/// which the original's do not (their lists are not the ghost's): the port's own, as the user asked.
pub struct Hidden {
    first: usize,
    cars: Vec<(Mesh, F, Option<Moving>)>,
}

/// Take the cars at `span` ([`Traffic::span`]) out of the world's props for one rider's frame.
pub fn hide(world: &mut dyn World, span: [usize; 2]) -> Hidden {
    let mut hidden = Hidden { first: span[0], cars: Vec::new() };
    let Some(props) = world.moving() else { return hidden };
    for prop in props.props.iter_mut().take(span[1]).skip(span[0]) {
        let empty = Mesh {
            version: prop.mesh.version,
            surface: prop.mesh.surface,
            transform: prop.mesh.transform,
            vertices: crate::track::Points::Wide(Vec::new()),
            normals: None,
            nodes: Vec::new(),
            triangles: Vec::new(),
        };
        let mesh = std::mem::replace(&mut prop.mesh, empty);
        hidden.cars.push((mesh, std::mem::replace(&mut prop.radius, F::ZERO), prop.moving.take()));
    }
    hidden
}

/// Put back what [`hide`] took, as it was.
pub fn show(world: &mut dyn World, hidden: Hidden) {
    let Some(props) = world.moving() else { return };
    for (prop, (mesh, radius, moving)) in props.props.iter_mut().skip(hidden.first).zip(hidden.cars) {
        prop.mesh = mesh;
        prop.radius = radius;
        prop.moving = moving;
    }
}

/// What a car's motion is to a wheel standing on it (S136): the car's own velocity and spin, as the
/// original's path car answers its two vehicle virtuals (`research/oracle.txt` S136), and the pose
/// they are about.
///
/// The original keeps the car's world matrix from before its update (copied at the update's top,
/// before the `dt` test) and makes both out of it on demand:
///
/// - **velocity**: `(position − last position) · (1/dt)`, `vsub.xyz` then `vmulx.xyz`, with the
///   position row's `w`;
/// - **spin**: `(last z × z + last y × y) · (1/dt)` — each cross a `vopmula`/`vopmsub` pair, the sum a
///   `vadd.xyz` with the first cross's `w` — and then put through the car's *current* rotation, as a
///   local vector would be.
///
/// Neither is state: both come from the poses either side of [`Traffic::frame`]'s step, which runs
/// before the rider on every tick, so a rollback that puts the cars back has them made again from the
/// cars it put back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Moving {
    /// The car's world matrix now.
    pub pose: Mat4,
    pub velocity: Vf,
    pub spin: Vf,
}

/// `car+0x190`, the point the car spins about in its own frame: `(0, 0, 0, 1)` on every one of
/// s89-along's 21 cars, and nothing in the path car's class writes it.
pub const ORIGIN: Vf = Vf([0.0, 0.0, 0.0, 1.0]);

impl Moving {
    /// The car's motion from its pose `before` its step to its pose `after`, at the frame's `1/dt`.
    pub fn of(before: &Mat4, after: &Mat4, inv_dt: F) -> Moving {
        let velocity = scale3(sub3(after.0[3], before.0[3]), inv_dt);
        let turned = add3(cross(before.0[2], after.0[2]), cross(before.0[1], after.0[1]));
        let spin = rotate(after, scale3(turned, inv_dt));
        Moving { pose: *after, velocity, spin }
    }

    /// The car's velocity and its velocity at the world `point`, which is what the wheel's
    /// contact-velocity step stores at `+0x1b0` and `+0x1c0` when the wheel's contact is on a car:
    /// the point taken into the car's frame (the transform's "unpoint"), its arm from [`ORIGIN`], the
    /// spin crossed with the arm and put through the car's rotation, and the car's velocity added.
    pub fn at(&self, point: Vf) -> (Vf, Vf) {
        let local = unrotate(&self.pose, sub3(point, self.pose.0[3]));
        let arm = sub3(local, ORIGIN);
        let turning = rotate(&self.pose, cross(self.spin, arm));
        (self.velocity, add3(turning, self.velocity))
    }

    /// **The car as a bone's push-out sees it** (S142): a mover. The car's mesh collidable has no
    /// collision object (`+0x80` is 0) and its owner (`+0x7c`) is the car, a vehicle of kind 2, so
    /// the push-out takes the car's two virtuals for the spin and the velocity, its model node's
    /// world matrix for the frame and [`ORIGIN`] for the point it spins about
    /// ([`crate::ragdoll_contact::moving`]).
    pub fn mover(&self) -> crate::contact::Mover {
        let [x, y, z, at] = self.pose.0;
        crate::contact::Mover { spin: self.spin, velocity: self.velocity, node: crate::contact::Node { rows: [x, y, z], origin: at }, origin: ORIGIN }
    }
}

/// `a × b`, the `vopmula`/`vopmsub` pair: `w` is `a`'s.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

/// The transform's "rotate": the rotation's rows scaled by `v`'s lanes, `w` from `v`.
fn rotate(m: &Mat4, v: Vf) -> Vf {
    crate::chassis::rotate(&crate::chassis::Mat3::new(m.0[0], m.0[1], m.0[2]), v)
}

/// And back, through the rotation's transpose.
fn unrotate(m: &Mat4, v: Vf) -> Vf {
    crate::chassis::unrotate(&crate::chassis::Mat3::new(m.0[0], m.0[1], m.0[2]), v)
}

impl State for Car {
    fn walk(&self, w: &mut Walk<'_>) {
        w.f("clock", self.clock);
        w.u32("seg", self.seg);
        w.vf("point", self.point);
        w.vf("head", self.head);
        w.vf("normal", self.normal);
        w.bool("first", self.first);
        for (name, row) in ["pose0", "pose1", "pose2", "pose3"].into_iter().zip(self.pose.0) {
            w.vf(name, row);
        }
        w.bool("stale", self.stale);
        w.f("level_y", self.level.y);
        w.f("level_last", self.level.last);
        w.bool("level_held", self.level.held);
    }
}

/// `vsub.xyz`: the first operand's `w` stays.
fn sub3(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `vadd.xyz`.
fn add3(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `vmulx.xyz` by a scalar moved in with `qmtc2`.
fn scale3(a: Vf, s: F) -> Vf {
    a.merge(Dest::XYZ, a.mul_q(s.0))
}
