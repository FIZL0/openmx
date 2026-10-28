//! **The props' tick** (S130): a knocked prop's collection world, its pins, how it falls asleep,
//! counts down and is put back, and where its model is drawn — the loader is [`crate::props`].
//!
//! Each collection is a dynamics world of its own ([`Collection::world`]: the `.pco`'s
//! `NumberOfIterations` constraint passes and `SimulationHz` substeps), a child of the world node the
//! wreck bike lives on. It sleeps until something wakes it: then the collection is **activated**
//! ([`Collection::activate`]: it ticks with the world node from now on, and its settle listener goes on
//! the node) and the struck object is **enabled** ([`Collection::enable`]: its body, force, preforce
//! and post listeners join the collection world, and its own listener goes on the node). Each frame
//! ([`Collection::tick`]) the active world steps — the preforce listener clears the body's shock peak,
//! the force listener adds the part's gravity, the bodies integrate, the pins solve, and the post
//! listener writes the body's pose to the model's node — and then the node's listeners run in the order
//! they were put on it, each taken off when it says so:
//!
//! * the **object listener** puts a body that is at rest ([`Collection::at_rest`]: its speed, rates,
//!   accelerations and shocks all under the part's thresholds) to sleep, and sets its countdown to the
//!   part's `ResetTime`; a sleeping body counts down by the frame's `dt`, and when the count goes
//!   below zero it is put back where it was made ([`Collection::reset`]) and the listener is done;
//! * the collection's **settle listener** deactivates the collection once every body is asleep and
//!   has no countdown left (or, for a collection with a `PinToBody`, counts its own down from the
//!   collection's `ResetTime`, then puts every body back).
//!
//! A body that leaves the world's bounds (`ResetOffEdgeOfWorld`) is put to sleep and back by its post
//! listener instead.
//!
//! The constraints are the ragdoll's pin ([`crate::ragdoll::Pin`]): a `Pin` holds the part's
//! `BodyPinPosition` to the world point its node puts `PinPosition` at, and a `PinToBody` two parts'
//! anchors together; both limit the turn lane by lane when `ConstrainAngles` (the curve sine of half
//! of each `MinAngles`/`MaxAngles` degree), a lone pin against the body's start turn.
//!
//! What is not here: the props' contacts (the collision object's force and its search, the struck
//! entries that wake a prop — S131), and `BodyTorsionSpring`, which [`Collection::new`] reports
//! rather than guesses (no free09 prop has one).

#![deny(clippy::float_arithmetic)]

use crate::biplane::{Body, Dynamics, List, World};
use crate::camera::CurveTable;
use crate::chassis::turn;
use crate::contact::Node;
use crate::math::{Mat4, basis, mat4_to_quat};
use crate::phy::Part;
use crate::props::{Constraint, NodeBox, Pco};
use crate::ragdoll::{Bone, Pin, Tables};
use crate::wreck_bike::node_of;
use ps2_float::fpu::Acc;
use ps2_float::{Dest, F, Field, Vf, fpu, vu};

/// `(0, 0, 0, 1)`: the engine's zero vector.
const ZERO: Vf = Vf::VF0;
/// Degrees to radians, as the constraint reader multiplies them.
const DEGREES: F = F(f32::from_bits(0x3c8e_fa34));
/// A pin's limits before any are set.
const NO_LIMIT: Vf = Vf::splat(-f32::MAX);
/// The origin a direction-only basis is built round.
const ORIGIN: Vf = Vf::new(0.0, 0.0, 0.0, 1.0);

/// One physics object's own state; its body is the collection's [`Bone`] of the same index.
#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    /// Where the object is put back to: the place (`w` 1), and the look and up axes, as the last
    /// placement left them ([`Collection::place`] squares and normalises them in place).
    pub place: Vf,
    pub look: Vf,
    pub up: Vf,
    /// The part's gravity force.
    pub gravity: Vf,
    /// `CollisionEnabled`, `DetectCollisionsWhenDisabled`, `ResetOffEdgeOfWorld`.
    pub collision_enabled: bool,
    pub detect_when_disabled: bool,
    pub reset_off_edge: bool,
    /// `ResetTime`, and the countdown to the put-back (below zero: none running).
    pub reset_time: F,
    pub countdown: F,
    /// The sleep thresholds: speed, angular rate, acceleration, angular acceleration, shock, twist.
    pub thresholds: [F; 6],
    /// Set by the enable, cleared by the first force: a body just woken is never at rest.
    pub fresh: bool,
    /// Whether the object listener goes on the node (always, for a part).
    pub listens: bool,
    /// The collision object is filed on the world node (struck, and searching when it is S131's).
    pub filed: bool,
    /// The model's node: the transform the post listener and the placement write.
    pub node: Node,
}

impl Object {
    /// A part's object at rest, placed by `place`, `look` and `up` (its model's frame).
    pub fn new(part: &Part, place: Vf, look: Vf, up: Vf) -> Object {
        Object {
            place,
            look,
            up,
            gravity: part.gravity,
            collision_enabled: part.collision_enabled,
            detect_when_disabled: part.detect_collisions_when_disabled,
            reset_off_edge: part.reset_off_edge_of_world,
            reset_time: part.reset_time,
            countdown: F(-1.0),
            thresholds: [
                part.speed_threshold,
                part.angular_rate_threshold,
                part.acceleration_threshold,
                part.angular_acceleration_threshold,
                part.speed_impulse_threshold,
                part.body_angular_speed_impulse_threshold,
            ],
            fresh: false,
            listens: true,
            filed: false,
            node: Node { rows: [Vf::ZERO; 3], origin: ORIGIN },
        }
    }
}

/// One constraint of a collection: the pin, its switch, and whether it limits the turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Joint {
    pub pin: Pin,
    pub on: bool,
    pub limited: bool,
}

/// What a listener on the world node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    /// The collection's settle listener.
    Settle,
    /// An object's listener.
    Object(usize),
}

/// The world's bounds a body must stay inside (`x` and `z`, open at both ends).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub min: [F; 2],
    pub max: [F; 2],
}

/// **One collection**: its world, its bodies and objects, its constraints, the world's lists, and the
/// node's listeners it has put on.
#[derive(Clone, Debug, PartialEq)]
pub struct Collection {
    pub world: World,
    pub bones: Vec<Bone>,
    pub objects: Vec<Object>,
    pub joints: Vec<Joint>,
    /// The world's lists, by object: bodies, force listeners, preforce listeners, post listeners.
    pub bodies: Vec<usize>,
    pub forces: Vec<usize>,
    pub preforce: Vec<usize>,
    pub posts: Vec<usize>,
    /// The collection's listeners on the world node, in the order they were put on.
    pub listeners: Vec<Listener>,
    /// Ticking with the world node.
    pub active: bool,
    /// A `PinToBody` was loaded: the bodies wake and sleep together.
    pub pin_to_body: bool,
    /// The collection's own `ResetTime` (`-1`, as the loader leaves it), and its countdown.
    pub reset_time: F,
    pub countdown: F,
    /// The world's bounds, when it has some.
    pub edge: Option<Edge>,
}

/// A constraint [`Collection::new`] does not make.
#[derive(Clone, Debug, PartialEq)]
pub enum Unmade {
    /// A `BodyTorsionSpring` (its force is not ported).
    TorsionSpring(String),
    /// A constraint naming a part the collection has none of.
    NoPart(String),
}

impl Collection {
    /// A collection's world as the loader leaves it: every body asleep where it was made, each object
    /// placed ([`Collection::place`]), then the constraints. `parts` are the members' parts and `bones`
    /// their bodies (the loader's); `names` the parts' `Name`s, which the constraints find them by.
    pub fn new(
        pco: &Pco,
        parts: &[(&Part, &str)],
        bones: Vec<Bone>,
        frames: &[Mat4],
        curve: &CurveTable,
    ) -> (Collection, Vec<Unmade>) {
        let objects = parts
            .iter()
            .zip(frames)
            .map(|(&(part, _), m)| Object::new(part, Vf::new(m.0[3].x(), m.0[3].y(), m.0[3].z(), 1.0), m.0[2], m.0[1]))
            .collect();
        let mut c = Collection {
            world: World { iterations: pco.iterations, rate: pco.rate },
            bones,
            objects,
            joints: Vec::new(),
            bodies: Vec::new(),
            forces: Vec::new(),
            preforce: Vec::new(),
            posts: Vec::new(),
            listeners: Vec::new(),
            active: false,
            pin_to_body: false,
            reset_time: F(-1.0),
            countdown: F(-1.0),
            edge: None,
        };
        for k in 0..c.objects.len() {
            c.bones[k].body.enabled = false;
            c.place(k);
        }
        let find = |name: &str| parts.iter().position(|(_, n)| n.eq_ignore_ascii_case(name));
        let mut unmade = Vec::new();
        for con in &pco.constraints {
            match con {
                Constraint::Pin { name, pin_position, body_pin_position, constrain_angles, min_angles, max_angles } => {
                    let Some(k) = find(name) else {
                        unmade.push(Unmade::NoPart(name.clone()));
                        continue;
                    };
                    let world = node_point(&c.objects[k].node, *pin_position);
                    let mut pin = new_pin(k, None, [*body_pin_position, world]);
                    limits(&mut pin, *min_angles, *max_angles, curve);
                    c.joints.push(Joint { pin, on: true, limited: *constrain_angles });
                }
                Constraint::PinToBody {
                    name1, name2, body_pin_position1, body_pin_position2, constrain_angles, min_angles, max_angles, ..
                } => {
                    let (Some(a), Some(b)) = (find(name1), find(name2)) else {
                        unmade.push(Unmade::NoPart(format!("{name1}/{name2}")));
                        continue;
                    };
                    let mut pin = new_pin(a, Some(b), [*body_pin_position1, *body_pin_position2]);
                    limits(&mut pin, *min_angles, *max_angles, curve);
                    c.joints.push(Joint { pin, on: true, limited: *constrain_angles });
                    c.pin_to_body = true;
                }
                Constraint::BodyTorsionSpring { name, .. } => unmade.push(Unmade::TorsionSpring(name.clone())),
                Constraint::Other(_) => {}
            }
        }
        (c, unmade)
    }

    /// **Wakes** the collection: it ticks from now on and its settle listener goes on the node. A
    /// collection with a `PinToBody` wakes all its bodies with it.
    pub fn activate(&mut self) {
        if self.active {
            return;
        }
        self.active = true;
        listen(&mut self.listeners, Listener::Settle);
        if self.pin_to_body {
            for k in 0..self.objects.len() {
                self.enable(k);
            }
            self.countdown = F(-1.0);
        }
    }

    /// **Wakes** one body: into the world's lists, its listener on the node, its collision object
    /// filed.
    pub fn enable(&mut self, k: usize) {
        if self.bones[k].body.enabled {
            return;
        }
        add(&mut self.bodies, k);
        add(&mut self.forces, k);
        add(&mut self.preforce, k);
        add(&mut self.posts, k);
        self.bones[k].body.enabled = true;
        let o = &mut self.objects[k];
        if o.listens {
            listen(&mut self.listeners, Listener::Object(k));
        }
        if o.collision_enabled {
            o.filed = true;
        }
        o.fresh = true;
    }

    /// Puts one body to sleep: out of the world's lists (but for its preforce listener), and its
    /// collision object unfiled unless it detects collisions asleep.
    pub fn disable(&mut self, k: usize) {
        if !self.bones[k].body.enabled {
            return;
        }
        self.bodies.retain(|&i| i != k);
        self.forces.retain(|&i| i != k);
        self.posts.retain(|&i| i != k);
        self.bones[k].body.enabled = false;
        let o = &mut self.objects[k];
        if o.filed && !o.detect_when_disabled {
            o.filed = false;
        }
    }

    /// **One frame** of the world node, for this collection: its world stepped while it is active,
    /// then its listeners on the node, each taken off when it is done.
    pub fn tick(&mut self, dt: F, tables: &Tables) {
        if self.active {
            let world = self.world;
            world.step(dt, &mut Lists { c: self, tables });
        }
        let mut i = 0;
        while i < self.listeners.len() {
            let done = match self.listeners[i] {
                Listener::Settle => self.settle(dt),
                Listener::Object(k) => self.listen_object(k, dt),
            };
            if done {
                self.listeners.remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// The object listener: says whether it is done.
    fn listen_object(&mut self, k: usize, dt: F) -> bool {
        if !self.objects[k].listens {
            return true;
        }
        if !self.bones[k].body.enabled {
            let o = &mut self.objects[k];
            if o.countdown < F::ZERO {
                return false;
            }
            o.countdown = o.countdown - dt;
            if !(o.countdown < F::ZERO) {
                return false;
            }
            self.reset(k);
            self.objects[k].filed = false;
            return true;
        }
        if !self.at_rest(k) {
            return false;
        }
        self.disable(k);
        let o = &mut self.objects[k];
        if !(o.reset_time < F::ZERO) {
            o.countdown = o.reset_time;
            return false;
        }
        !o.detect_when_disabled
    }

    /// The settle listener: says whether it is done.
    fn settle(&mut self, dt: F) -> bool {
        let n = self.objects.len();
        if !self.pin_to_body {
            if (0..n).all(|k| !self.bones[k].body.enabled) {
                if self.countdown < F::ZERO {
                    self.active = false;
                    return true;
                }
                self.countdown = self.countdown - dt;
                if self.countdown < F::ZERO {
                    for k in 0..n {
                        self.reset(k);
                    }
                    self.active = false;
                    return true;
                }
                return false;
            }
            if !(self.reset_time < F::ZERO) {
                self.countdown = self.reset_time;
            }
            return false;
        }
        if !(self.countdown < F::ZERO) {
            self.countdown = self.countdown - dt;
            if self.countdown < F::ZERO {
                for k in 0..n {
                    self.reset(k);
                }
                self.active = false;
                return true;
            }
            return false;
        }
        if !(0..n).all(|k| self.at_rest(k)) {
            return false;
        }
        for k in 0..n {
            self.disable(k);
        }
        if self.reset_time < F::ZERO {
            self.active = false;
            return true;
        }
        self.countdown = self.reset_time;
        false
    }

    /// Whether one body may sleep: not just woken, and each of its six magnitudes under its threshold.
    pub fn at_rest(&self, k: usize) -> bool {
        let b = &self.bones[k].body;
        let o = &self.objects[k];
        if o.fresh {
            return false;
        }
        let size = |v: Vf| Acc::mula(F(v.x()), F(v.x())).madda(F(v.y()), F(v.y())).madd(F(v.z()), F(v.z())).sqrt();
        [b.velocity, b.rates, b.accel, b.ang_accel, b.shock, b.twist]
            .iter()
            .zip(o.thresholds)
            .all(|(&v, t)| size(v) < t)
    }

    /// **Puts one object back**: its body reset to where it was made and its node with it
    /// ([`Collection::place`] on its own place, look and up).
    pub fn reset(&mut self, k: usize) {
        self.place(k);
    }

    /// The placement: the look and up squared to each other and normalised (written back), the
    /// body's start place and turn set off them and the body reset to its starts, and the node set to
    /// the body's origin and the axes.
    pub fn place(&mut self, k: usize) {
        let o = &mut self.objects[k];
        let (look, up) = (o.look, o.up);
        let side = Vf::opmsub(Vf::opmula(look, up), up, look).merge(Dest::W, look);
        let up = up.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(side, look), look, side));
        let unit = |v: Vf| v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, vu::dot3(v.0, v.0))));
        let (look, up) = (unit(look), unit(up));
        o.look = look;
        o.up = up;
        o.place = o.place.merge(Dest::W, Vf::splat(1.0));
        let quat = mat4_to_quat(&basis(ORIGIN, look, up, false, true));
        let bone = &mut self.bones[k];
        bone.reference = quat;
        reset_body(&mut bone.body, o.place, quat);
        let b = &bone.body;
        let origin = b.position.sub(turn(b.quat, b.cofg));
        let m = basis(origin, look, up, false, true);
        o.node = Node { rows: [m.0[0], m.0[1], m.0[2]], origin: m.0[3] };
    }

    /// The post listener: a woken body's pose written to its node — or, once it is past the world's
    /// bounds, the body put to sleep and back.
    fn post(&mut self, k: usize) {
        if !self.bones[k].body.enabled {
            return;
        }
        let b = &self.bones[k].body;
        let origin = b.last_position.sub(turn(b.last_quat, b.cofg));
        if self.inside(k, origin) {
            self.objects[k].node = node_of(b);
        } else {
            self.disable(k);
            self.reset(k);
        }
    }

    /// The edge check: `x` and `z` strictly inside the bounds, or no check at all.
    fn inside(&self, k: usize, at: Vf) -> bool {
        let Some(e) = self.edge.filter(|_| self.objects[k].reset_off_edge) else { return true };
        let (x, z) = (F(at.x()), F(at.z()));
        x < e.max[0] && x > e.min[0] && z < e.max[1] && z > e.min[1]
    }
}

/// The body's starts set and the body reset to them: place and turn, with no velocity and no rates;
/// the forces, the shocks, the moments and the accelerations cleared. The substep's step is kept.
fn reset_body(b: &mut Body, place: Vf, quat: Vf) {
    b.last_position = place;
    b.position = place;
    b.velocity = ZERO;
    b.force = ZERO;
    b.last_force = ZERO;
    b.shock_peak = ZERO;
    b.shock = ZERO;
    b.accel = ZERO;
    b.last_quat = quat;
    b.quat = quat;
    b.rates = ZERO;
    b.moment = ZERO;
    b.last_moment = ZERO;
    b.ang_accel = ZERO;
    b.twist_peak = ZERO;
    b.twist = ZERO;
}

/// A node's transform of a point: the origin, then each row times its lane, accumulated in that order.
pub fn node_point(node: &Node, p: Vf) -> Vf {
    let acc = node.origin.add(Vf::ZERO);
    let acc = Vf::madd(acc, node.rows[0], p.bc(Field::X));
    let acc = Vf::madd(acc, node.rows[1], p.bc(Field::Y));
    Vf::madd(acc, node.rows[2], p.bc(Field::Z))
}

fn new_pin(first: usize, second: Option<usize>, anchors: [Vf; 2]) -> Pin {
    Pin {
        first,
        second,
        anchors,
        target: ZERO,
        target_velocity: ZERO,
        lower: NO_LIMIT,
        upper: NO_LIMIT,
        hit: ZERO,
        sticky: ZERO,
        gains: Vf::new(0.0, 0.0, 1.0, 0.0),
    }
}

/// The turn limits: set only when every lane's minimum is at most its maximum, each the curve sine of
/// half the angle in radians; the lower's `w` is zeroed, the upper's left.
fn limits(pin: &mut Pin, min: Vf, max: Vf, curve: &CurveTable) {
    let (lo, hi) = (min.mul(Vf::splat(DEGREES.0)), max.mul(Vf::splat(DEGREES.0)));
    if !(0..3).all(|k| F(lo[k]) <= F(hi[k])) {
        return;
    }
    pin.lower[3] = 0.0;
    for k in 0..3 {
        pin.lower[k] = curve.sine(F(lo[k]) * F(0.5)).0;
        pin.upper[k] = curve.sine(F(hi[k]) * F(0.5)).0;
    }
}

fn add(list: &mut Vec<usize>, k: usize) {
    if !list.contains(&k) {
        list.push(k);
    }
}

fn listen(list: &mut Vec<Listener>, l: Listener) {
    if !list.contains(&l) {
        list.push(l);
    }
}

/// The collection's world as its step sees it.
struct Lists<'a> {
    c: &'a mut Collection,
    tables: &'a Tables,
}

impl Dynamics for Lists<'_> {
    fn count(&self, list: List) -> i32 {
        let c = &self.c;
        (match list {
            List::Bodies => c.bodies.len(),
            List::Force => c.forces.len(),
            List::Constraints => c.joints.len(),
            List::Preforce => c.preforce.len(),
            List::Post => c.posts.len(),
        }) as i32
    }
    fn preforce(&mut self, i: i32, _dt: F) {
        let k = self.c.preforce[i as usize];
        self.c.bones[k].body.clear_shock_peak();
    }
    fn force(&mut self, i: i32, _h: F) {
        let k = self.c.forces[i as usize];
        let b = &mut self.c.bones[k].body;
        if b.enabled {
            b.add_force(self.c.objects[k].gravity);
            self.c.objects[k].fresh = false;
        }
    }
    fn enabled(&self, i: i32) -> bool {
        self.c.bones[self.c.bodies[i as usize]].body.enabled
    }
    fn integrate(&mut self, i: i32, h: F) {
        let k = self.c.bodies[i as usize];
        self.c.bones[k].body.integrate(h);
    }
    fn end_substep(&mut self, i: i32) {
        let k = self.c.bodies[i as usize];
        self.c.bones[k].body.end_substep();
    }
    fn constrain(&mut self, i: i32, _first: bool) {
        let Joint { pin, on, limited } = &mut self.c.joints[i as usize];
        pin.solve(*on, *limited, None, &mut self.c.bones, self.tables);
    }
    fn post(&mut self, i: i32, _dt: F) {
        let k = self.c.posts[i as usize];
        self.c.post(k);
    }
}

// --- the node's bounds -------------------------------------------------------------------------------

/// A scene node as its bounds query walks it: its own box (if it has one), its world rows, and its
/// children.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundsNode {
    pub own: Option<NodeBox>,
    pub world: Mat4,
    pub children: Vec<BoundsNode>,
}

/// **The node's bounds**: its own box's corners `centre ∓ half` (or none), widened by every
/// descendant's box carried into this node's frame (each box's centre through the descendant's world
/// rows and this node's inverse, its half the absolute rows times the half, summed), as the
/// `(centre, half extent)` of the span. A node with no box anywhere spans `(0, 0, 0, 1)` to itself.
pub fn node_bounds(node: &BoundsNode) -> NodeBox {
    let (mut lo, mut hi) = (Vf::VF0, Vf::VF0);
    let mut any = false;
    if let Some(b) = node.own {
        lo = b.centre.merge(Dest::XYZ, b.centre.sub(b.half_extent));
        hi = b.centre.merge(Dest::XYZ, b.centre.add(b.half_extent));
        any = true;
    }
    let inverse = rigid_inverse(&node.world);
    for child in &node.children {
        merge(child, &inverse, &mut any, &mut lo, &mut hi);
    }
    let half = Vf::splat(0.5);
    // `.xyz` sums: the centre keeps the low corner's `w`, the half extent the high corner's.
    NodeBox {
        centre: lo.merge(Dest::XYZ, lo.add(hi).mul(half)),
        half_extent: hi.merge(Dest::XYZ, hi.sub(lo).mul(half)),
    }
}

/// The world rows' inverse as the bounds query takes it: the rotation transposed (each moved lane
/// added to zero) and the origin turned back through it.
fn rigid_inverse(m: &Mat4) -> [Vf; 4] {
    let [r0, r1, r2, r3] = m.0;
    let z = |v: f32| vu::add(0.0, v);
    let a = Vf::new(r0.x(), z(r1.x()), z(r2.x()), r0.w());
    let b = Vf::new(z(r0.y()), r1.y(), z(r2.y()), r1.w());
    let c = Vf::new(z(r0.z()), z(r1.z()), r2.z(), r2.w());
    let acc = Vf::ZERO.add(Vf::ZERO);
    let acc = Vf::msub(acc, a, r3.bc(Field::X));
    let acc = Vf::msub(acc, b, r3.bc(Field::Y));
    let d = r3.merge(Dest::XYZ, Vf::msub(acc, c, r3.bc(Field::Z)));
    [a, b, c, d]
}

/// One descendant's box into the span, then its children's.
fn merge(node: &BoundsNode, inv: &[Vf; 4], any: &mut bool, lo: &mut Vf, hi: &mut Vf) {
    if let Some(b) = node.own {
        let row = |r: Vf, base: Vf| {
            let acc = base;
            let acc = Vf::madd(acc, inv[0], r.bc(Field::X));
            let acc = Vf::madd(acc, inv[1], r.bc(Field::Y));
            Vf::madd(acc, inv[2], r.bc(Field::Z))
        };
        let c = node.world.0;
        let lin = |r: Vf| {
            let acc = inv[0].mul(r.bc(Field::X));
            let acc = Vf::madd(acc, inv[1], r.bc(Field::Y));
            Vf::madd(acc, inv[2], r.bc(Field::Z))
        };
        let (m0, m1, m2) = (lin(c[0]), lin(c[1]), lin(c[2]));
        let m3 = row(c[3], inv[3].add(Vf::ZERO));
        let h = b.half_extent;
        let a0 = m0.mul(h.bc(Field::X)).abs();
        let a1 = m1.mul(h.bc(Field::Y)).abs();
        let a2 = m2.mul(h.bc(Field::Z)).abs();
        let p = b.centre;
        let acc = m3.add(Vf::ZERO);
        let acc = Vf::madd(acc, m0, p.bc(Field::X));
        let acc = Vf::madd(acc, m1, p.bc(Field::Y));
        let centre = p.merge(Dest::XYZ, Vf::madd(acc, m2, p.bc(Field::Z)));
        let span = h.merge(Dest::XYZ, Vf::madd(a1.add(a0), a2, Vf::VF0.bc(Field::W)));
        let mut l = m0.merge(Dest::XYZ, centre.sub(span));
        let mut u = m1.merge(Dest::XYZ, centre.add(span));
        if *any {
            l = Vf(core::array::from_fn(|i| ordered(l[i], lo[i], true)));
            u = Vf(core::array::from_fn(|i| ordered(u[i], hi[i], false)));
        }
        *lo = l;
        *hi = u;
        *any = true;
    }
    for child in &node.children {
        merge(child, inv, any, lo, hi);
    }
}


/// `vmini` (`low`) or `vmax` on one lane with two zeros ordered by sign, `-0` under `+0`, as the
/// EE's own `min.s`/`max.s` do ([`vu::mini`] and [`vu::max`] keep the first of two zeros).
fn ordered(a: f32, b: f32, low: bool) -> f32 {
    if F(a) == F::ZERO && F(b) == F::ZERO {
        return if low { fpu::min(a, b) } else { fpu::max(a, b) };
    }
    if low { vu::mini(a, b) } else { vu::max(a, b) }
}
