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
//!    0.707), and kept as it was past that.
//! 5. **The pose**: the engine's look-at ([`math::basis`], up kept, forward made square to it) at
//!    the point with the ground's height, and the car's collision mesh moved there.
//!
//! The cars are state ([`Traffic::cars`], hashed and sent) so a rollback or a replay moves them with
//! everything else; their paths and numbers are a constant of the track ([`Traffic::specs`]).
//!
//! Not here, each the original's and none of free06's (every car there sets them `FALSE`):
//! `PathIsLevel`, `PresampleTerrain` and `DriveOnSoultree`'s ray down onto the props. A car that
//! sets them is moved the way free06's are. Nor the `HasPhysics` cars, which are another class.

#![deny(clippy::float_arithmetic)] // every sum below is the PS2's, through ps2-float

use std::sync::Arc;

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
    /// The `Body` channel's position, `w` 1.
    pub pos: Vf,
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
        let keys = points.iter().map(|&(t, p)| Key { t, len: F::ZERO, pos: Vf::new(p.x(), p.y(), p.z(), 1.0) }).collect();
        let mut spec = Spec { keys, time_scale, heading_filter, normal_filter, lag, mesh, cursor: 0 };
        spec.cursor = spec.measure(0);
        Some(spec)
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
        let n = self.keys.len();
        let mut i = (seg as usize).min(n - 2);
        let mut j = i + 1;
        while j < n && !(s <= self.keys[j].t) {
            i += 1;
            j += 1;
        }
        // Past the last key the original reads one key beyond its array; a clock wrapped by the last
        // key's time never gets there.
        if j >= n {
            i = n - 2;
            j = n - 1;
        }
        while i != 0 && s < self.keys[i].t {
            i -= 1;
            j -= 1;
        }
        Cursor { i, j }
    }

    /// The point on the path at `s`, and the segment's own parameter there, from the cursor at `seg`,
    /// which moves onto `s`'s segment: the cubic Hermite through the segment's two keys with
    /// Catmull-Rom tangents, each the chord and its neighbour summed and scaled by the time spans (the
    /// chord alone at either end of the path).
    pub fn curve(&self, seg: &mut u32, s: F) -> (Vf, F) {
        let n = self.keys.len();
        let c = self.find(*seg, s);
        *seg = c.i as u32;
        let (ki, kj) = (self.keys[c.i], self.keys[c.j]);
        let t = (s - ki.t) / (kj.t - ki.t);
        let t2 = t * t;
        let t3 = t * t2;
        let h = (F(3.0) * t2 - t3) - t3;
        let one_h = F::ONE - h;
        let w1 = t3 - t2;
        let w0 = t + (w1 - t2);
        let chord = sub3(kj.pos, ki.pos);
        let m0 = if c.i == 0 {
            chord
        } else {
            let kh = self.keys[c.i - 1];
            let f = (kj.t - ki.t) / (kj.t - kh.t);
            scale3(add3(sub3(ki.pos, kh.pos), chord), f)
        };
        let m1 = if c.j == n - 1 {
            chord
        } else {
            let kk = self.keys[c.j + 1];
            let f = (kj.t - ki.t) / (kk.t - ki.t);
            scale3(add3(chord, sub3(kk.pos, kj.pos)), f)
        };
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
    pub fn lagged(&self, seg: u32, s: F, lag: F) -> F {
        let n = self.keys.len();
        let c = self.find(seg, s);
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
        let s = if self.lag <= F::ZERO { car.clock } else { self.lagged(car.seg, car.clock, self.lag) };
        let (point, _) = self.curve(&mut car.seg, s);
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
        let (ground, up) = world.ground(point);
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
        let at = Vf::new(point.x(), ground.y(), point.z(), 1.0);
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
}

impl Traffic {
    /// The cars of a track, as its load leaves them.
    pub fn new(specs: Vec<Spec>) -> Traffic {
        let cars = specs.iter().map(Car::new).collect();
        Traffic { specs: Arc::new(specs), cars, first: None, toys: Default::default() }
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
        if self.cars.is_empty() {
            return;
        }
        let specs = Arc::clone(&self.specs);
        for (spec, car) in specs.iter().zip(&mut self.cars) {
            spec.step(car, dt, world);
        }
        self.place(world);
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
