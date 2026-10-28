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
//! **A body's own collision object** (S198, [`Collision`]) is the wreck bike's kind
//! ([`crate::wreck_bike`]): the part's tuning, a ray probe out of the model's `<node>m.col` and a sweep
//! probe. Waking a body files it on the world node, and the node's update searches every filed one
//! before anything steps ([`Collection::search`]: the sweep back, the rays against the terrain, the rays
//! against the world's objects); the body's force listener then resolves what it found
//! ([`crate::contact::resolve`]) after adding gravity, so a loose prop lands, bounces and rolls instead
//! of falling through the ground. A body put to sleep keeps its collision object filed when the part
//! says `DetectCollisionsWhenDisabled`: it goes on searching, its rays among the categories that wake
//! it only ([`crate::flight_search::WAKE_MASK`]).
//!
//! What is not here: the props struck (their `f.col` as a collidable others' searches meet, and the
//! wake that strike or the rays' own hit gives — S197), and `BodyTorsionSpring`, which
//! [`Collection::new`] reports rather than guesses (no free09 prop has one).

#![deny(clippy::float_arithmetic)]

use crate::biplane::{self, Body, Dynamics, List, World};
use crate::camera::CurveTable;
use crate::chassis::turn;
use crate::contact::{self, Collider, Mover, Node, Other, Side, Weighed};
use crate::contact_search::{self, Heightfield, Hit, Record, Search, Struck, Swept};
use crate::eco::Trees;
use crate::flight_search::{self, Rays, Scene};
use crate::world::Props;
use crate::wreck_bike::{self, NoObjects};
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
    /// The collision object (S198), when the loader made one.
    pub collision: Option<Collision>,
}

/// The flags of a prop's collidable (its `f.col`: the collision object's `+0x1f8`, its `+0x84`) and
/// its collides-with mask (`+0x114`), the same on every free09 prop. Read off the running game, as the
/// wreck bike's ([`wreck_bike::COLLIDABLE`]): they decide only what the object does against another
/// collision object, which nothing here meets yet (S197).
pub const COLLIDABLE: u32 = 0x18;
pub const COLLIDES_WITH: u32 = 0xffff_ffff;

/// The contact capacity of a collision object with no ray probe (one a ray otherwise).
pub const CAPACITY: usize = 8;

/// A body's collision object (S198): the wreck bike's kind, tuned by the same setters off the part's
/// keys ([`wreck_bike::collider`], [`wreck_bike::record`]), its ray probe the model's `m.col` rays (an
/// object with none — the signs — has no ray probe and no sweep probe either, so its search finds
/// nothing). Its transform is the model's node, [`Object::node`]: the record holds it only while it
/// searches (a sweep back moves it), and is left at the default between.
#[derive(Clone, Debug, PartialEq)]
pub struct Collision {
    pub collider: Collider,
    pub record: Record,
}

impl Collision {
    /// The part's collision object, `collider` the tuning the body's loader made with it, `rays` the
    /// model's ray file's segments (`[start, end]`, model space).
    pub fn new(part: &Part, mut collider: Collider, rays: Option<Vec<[Vf; 2]>>) -> Collision {
        collider.collides_with = COLLIDES_WITH;
        collider.collidable = Some(COLLIDABLE);
        let mut record = wreck_bike::record(part, Node::default());
        record.capacity = rays.as_ref().map_or(CAPACITY, Vec::len);
        record.nearby = rays.is_some();
        record.sweeps = rays.is_some();
        record.rays = rays;
        Collision { collider, record }
    }
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
            collision: None,
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
        let o = &self.objects[k];
        if o.filed && !o.detect_when_disabled {
            self.unfile(k);
        }
    }

    /// The collision object taken off the world node: its list and flags emptied first (its clear),
    /// so a put-back object touches nothing until a wake files it again.
    fn unfile(&mut self, k: usize) {
        let o = &mut self.objects[k];
        if !o.filed {
            return;
        }
        o.filed = false;
        if let Some(c) = o.collision.as_mut() {
            contact_search::clear(&mut c.record, Some(&mut self.bones[k].body));
        }
    }

    /// **One frame** of the world node, for this collection: its world stepped while it is active,
    /// then its listeners on the node, each taken off when it is done.
    pub fn tick(&mut self, dt: F, tables: &Tables) {
        self.step(dt, tables, &[]);
    }

    /// [`Collection::tick`] after a [`Collection::search`]: the contacts' `movers` are what the search's
    /// hits name.
    pub fn step(&mut self, dt: F, tables: &Tables, movers: &[Mover]) {
        if self.active {
            let world = self.world;
            world.step(dt, &mut Lists { c: self, tables, movers });
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

    /// **The node's collide pass** for this collection (S198): every filed collision object empties its
    /// list and searches ([`contact_search::clear`], [`contact_search::collide`]) — the sweep back from
    /// where the body was a little while ago, the rays against the terrain `field`, the rays against
    /// the world's objects: the scene's collision meshes `props` and the ecosystem's `trees`, through a
    /// nearby list gathered where the probe stands, as the wreck bike's ([`wreck_bike::WreckBike::tick_among`]).
    /// `rays` is each object's ray probe tree (the model's `m.col`), in object order.
    ///
    /// A sleeping body's object (filed still, `DetectCollisionsWhenDisabled`) searches too, the body
    /// standing still; its ray probe's category list is then the wake mask alone, so its rays keep only
    /// what that meets.
    pub fn search(&mut self, dt: F, rays: &[Option<Rays>], field: Option<&dyn Heightfield>, props: Option<&Props>, trees: Option<&Trees>) {
        for k in 0..self.objects.len() {
            let o = &mut self.objects[k];
            let (true, Some(c)) = (o.filed, o.collision.as_mut()) else { continue };
            let body = &mut self.bones[k].body;
            let asleep = !body.enabled;
            c.record.node = o.node;
            contact_search::clear(&mut c.record, Some(body));
            let world: &mut dyn Search;
            let mut none = NoObjects;
            let mut scene;
            match (rays.get(k).and_then(Option::as_ref), props.is_some() || trees.is_some()) {
                (Some(tree), true) => {
                    let bounds = field.map(|f| {
                        let (lo, hi) = (f.min(), f.max());
                        ([F(lo.x()), F(lo.z())], [F(hi.x()), F(hi.z())])
                    });
                    let mut nearby = flight_search::search();
                    scene = Scene::wooded(props, trees, None, bounds, tree, &c.record.node, &mut nearby);
                    world = &mut scene;
                }
                _ => world = &mut none,
            }
            let mut waking = Waking { world, asleep };
            contact_search::collide(std::slice::from_mut(&mut c.record), 0, Some(body), field, &mut waking, dt);
            o.node = std::mem::take(&mut c.record.node);
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
            self.unfile(k);
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

    /// **Body `k` as the other side of a contact** (S169), when it has a collision object: what a
    /// contact on the prop's `f.col` meets. The original's `f.col` names no mover (`+0x7c` is 0, so
    /// a prop gives no [`crate::traffic::Moving`] as a path car or a toy part does); it names its
    /// owner, the collision object (`+0x80`), whose body (`+0xf4`) is this one. A wheel or a body ray
    /// standing on a moving prop reads its motion through that object ([`contact::other_at`], the
    /// resolve's other side), not through a mover.
    pub fn other<R>(&mut self, k: usize, f: impl FnOnce(&mut Other<'_>) -> R) -> Option<R> {
        let bone = &mut self.bones[k];
        let c = self.objects[k].collision.as_mut()?;
        let mut rigid = Weighed { body: &mut bone.body, mass: bone.mass };
        Some(f(&mut Other { collider: &mut c.collider, side: Side::Body(&mut rigid) }))
    }

    /// **What a wheel standing on body `k` reads** (S169): the wheel contact-velocity step's owner
    /// branch on the prop's collision object — the body's velocity and its velocity at the world
    /// `point` ([`contact::other_at`]), the object's centre of mass put into the world on the way.
    /// `None` for a body with no collision object.
    pub fn under(&mut self, k: usize, point: Vf) -> Option<(Vf, Vf)> {
        self.other(k, |o| contact::other_at(o, point))
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
    movers: &'a [Mover],
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
    /// The object's force listener: gravity, then its collision object's contacts resolved.
    fn force(&mut self, i: i32, _h: F) {
        let k = self.c.forces[i as usize];
        let bone = &mut self.c.bones[k];
        let o = &mut self.c.objects[k];
        let movers = self.movers;
        let pushed = biplane::collision_force(&mut bone.body, o.gravity, |body| {
            if let Some(c) = o.collision.as_mut() {
                let mut rigid = Weighed { body, mass: bone.mass };
                contact::resolve(&mut c.collider, &mut rigid, &mut c.record.contacts, &mut [], movers, true);
            }
        });
        if pushed {
            o.fresh = false;
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

/// The world as a collision object's search sees it: a sleeping body's ray probe keeps only the hits on
/// what its category list (the wake mask) meets.
struct Waking<'a> {
    world: &'a mut dyn Search,
    asleep: bool,
}

impl Search for Waking<'_> {
    fn rays(&mut self, nearby: bool) -> Vec<Hit> {
        let mut hits = self.world.rays(nearby);
        if self.asleep {
            hits.retain(|h| flight_search::category(h.struck.object) & flight_search::WAKE_MASK != 0);
        }
        hits
    }
    fn sweep(&mut self, from: Vf, to: Vf, nearby: bool) -> Option<Swept> {
        self.world.sweep(from, to, nearby)
    }
    fn moved(&mut self, node: &Node) {
        self.world.moved(node);
    }
    fn velocity(&self, struck: &Struck) -> Vf {
        self.world.velocity(struck)
    }
    fn movers(&self) -> Vec<Mover> {
        self.world.movers()
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
