//! The collision object's contact resolution: what a rigid body does with the contacts its collision
//! object holds, once a substep.
//!
//! Every dynamics body that can hit things has a collision object beside it (the plane's is the one
//! its `+0x170` points at). The object keeps a list of contacts, filled before the world step by the
//! object's own collide pass (not this module's; see the journal), and the body's collision force
//! listener asks it to resolve them every substep ([`crate::biplane::collision_force`] hands that in
//! as a closure). For each contact, [`resolve`]:
//!
//! 1. works out where the contact is on each body and how fast the two points move apart along the
//!    contact's normal ([`relative_velocity`]);
//! 2. if they close faster than [`HARD`], and impulses are on and the contact is the body's own, it
//!    stops them dead with an impulse ([`impulse`]); otherwise, it pushes them apart with a damped
//!    spring ([`spring`]), and nothing at all when they close that fast without an impulse;
//! 3. rubs the sliding part of the motion off with friction ([`friction`]);
//! 4. and, if the object says so, lowers the body's linear and angular damping by how square the
//!    contact is to each axis ([`damp`]).
//!
//! Who the contact is with matters in three ways. The other side may be nothing (the scenery), another
//! collision object with a dynamics body of its own, another collision object on a vehicle's chassis,
//! or a moving object that is not a body (a "mover", which only has a velocity and a spin). Two
//! collision objects that both say they collide with each other share the impulse and the spring by
//! their objects' masses (the *mutual* case); otherwise the bodies' own masses are used. And an
//! other whose collidable is in this object's actor mask, or a mover, gets the actor coefficients
//! instead of the scenery's.
//!
//! The own side is a [`Collider`] on a [`Rigid`] body ([`resolve`]: the plane's case, the wreck
//! bike's), or on a vehicle's chassis through its transform ([`resolve_ridden`]: a riding bike's,
//! whose collision record is how one rider's bike pushes on another's). `w` lanes are kept as the
//! original leaves them, because it stores whole quadwords.

#![deny(clippy::float_arithmetic)]

use crate::biplane::Body;
use crate::chassis::{CLEAR, turn};
use ps2_float::fpu::Acc;
use ps2_float::{Dest, F, Field, Vf, vu};

/// The closing speed along the normal past which a contact is stopped with an impulse rather than
/// pushed apart by the spring.
pub const HARD: f32 = -10.0;
/// Friction: the sliding speed at which it reaches its full force, as a factor (`min(0.1·speed, 1)`),
/// and the gravity the friction coefficient is a fraction of the weight under.
const SLIDE: f32 = f32::from_bits(0x3dcc_cccd);
const WEIGHT: f32 = f32::from_bits(0x4200_cccd);

// --- the vector idioms -----------------------------------------------------------------------------

/// `v` turned into the body's frame by `q`: [`turn`] with every pair of operands swapped. `w` is `v`'s.
pub(crate) fn unturn(q: Vf, v: Vf) -> Vf {
    let acc = Vf::opmula(v, q);
    let acc = Vf::madd(acc, v, q.bc(Field::W));
    let t = Vf::opmsub(acc, q, v);
    let p = q.mul(v);
    let acc = Vf::opmula(t, q);
    let acc = Vf::madd(acc, q, p.bc(Field::X));
    let acc = Vf::madd(acc, q, p.bc(Field::Y));
    let acc = Vf::madd(acc, q, p.bc(Field::Z));
    let acc = Vf::madd(acc, t, q.bc(Field::W));
    v.merge(Dest::XYZ, Vf::opmsub(acc, q, t))
}

/// `vopmula a, b` / `vopmsub a ← b, a`, `.xyz`: `a × b`, `w` from `a`.
fn cross(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(a, b), b, a))
}

/// `a.xyz + b.xyz`, `w` from `a`.
pub(crate) fn plus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `a.xyz − b.xyz`, `w` from `a`.
pub(crate) fn minus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `a.xyz · b.xyz` lane by lane, `w` from `a`.
fn times(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.mul(b))
}

/// `v.xyz · s`, `w` kept (`vmulx.xyz` against a broadcast single).
pub(crate) fn scaled(v: Vf, s: F) -> Vf {
    v.merge(Dest::XYZ, v.mul(Vf::splat(s.to_f32())))
}

/// `-v.xyz`, `w` kept: three `neg.s`.
pub(crate) fn negated(v: Vf) -> Vf {
    Vf::new((-F(v.x())).to_f32(), (-F(v.y())).to_f32(), (-F(v.z())).to_f32(), v.w())
}

/// The FPU's three-term dot product, `mula.s` / `madda.s` / `madd.s`, in this operand order.
pub(crate) fn dot(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// `vmul.xyz`, `vadday.x`, `vmaddz.x` (by one), `vrsqrt`, `vmulq.xyz`: unit length, `w` kept.
pub(crate) fn unit(v: Vf) -> Vf {
    v.merge(Dest::XYZ, v.mul_q(vu::rsqrt(1.0, vu::dot3(v.0, v.0))))
}

// --- the transform ---------------------------------------------------------------------------------

/// A scene transform as the resolution reads it: three rows and the translation (`+0x80..+0xb0`),
/// already up to date. The original asks the transform to rebuild itself from its parents first when
/// it is dirty; the port is handed it clean.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Node {
    pub rows: [Vf; 3],
    pub origin: Vf,
}

impl Node {
    /// A direction into the world: the rows scaled by `v`'s lanes, `w` from `v`.
    pub fn rotate(&self, v: Vf) -> Vf {
        let acc = self.rows[0].mul(v.bc(Field::X));
        let acc = Vf::madd(acc, self.rows[1], v.bc(Field::Y));
        v.merge(Dest::XYZ, Vf::madd(acc, self.rows[2], v.bc(Field::Z)))
    }

    /// A point into the world: [`Node::rotate`] onto the translation (`vaddax` by `vf0.x` first).
    pub fn place(&self, v: Vf) -> Vf {
        let acc = self.origin.add(Vf::VF0.bc(Field::X));
        let acc = Vf::madd(acc, self.rows[0], v.bc(Field::X));
        let acc = Vf::madd(acc, self.rows[1], v.bc(Field::Y));
        v.merge(Dest::XYZ, Vf::madd(acc, self.rows[2], v.bc(Field::Z)))
    }

    /// A world direction into the transform's frame: each lane the direction dotted with a row. The
    /// original builds the columns in the integer registers (`pextlw`, `pexcw`, `pextuw`, `pcpyld`,
    /// `pcpyud`), and the third is a 64-bit `move` of the `pextuw` result over the third row, so its
    /// upper half (`r2.z`, `r2.w`) stays — the transpose comes out whole.
    pub fn unrotate(&self, v: Vf) -> Vf {
        let [r0, r1, r2] = self.rows;
        let c0 = Vf::new(r0.x(), r1.x(), r2.x(), r2.y());
        let c1 = Vf::new(r0.y(), r1.y(), r2.y(), r2.w());
        let c2 = Vf::new(r0.z(), r1.z(), r2.z(), r2.w());
        let acc = c0.mul(v.bc(Field::X));
        let acc = Vf::madd(acc, c1, v.bc(Field::Y));
        v.merge(Dest::XYZ, Vf::madd(acc, c2, v.bc(Field::Z)))
    }

    /// A world point into the transform's frame: the translation off (`.xyz`), then [`Node::unrotate`].
    pub fn unplace(&self, v: Vf) -> Vf {
        self.unrotate(minus(v, self.origin))
    }
}

// --- the parties -----------------------------------------------------------------------------------

/// A dynamics body, as the resolution reads it and pushes on it: the fields at the offsets the doc
/// gives, and its virtual entries, in the order the resolution calls them.
pub trait Rigid {
    /// `+0xc`, `+0x10`.
    fn mass(&self) -> F;
    fn inv_mass(&self) -> F;
    /// `+0x20`, `+0x130`: the linear and angular damping.
    fn damping(&self) -> Vf;
    fn ang_damping(&self) -> Vf;
    /// `+0x60`, `+0x70`.
    fn position(&self) -> Vf;
    fn velocity(&self) -> Vf;
    /// `+0x120`: a diagonal in the body's frame.
    fn inv_inertia(&self) -> Vf;
    /// `+0x170`, `+0x180`, `+0x1b0`.
    fn quat(&self) -> Vf;
    fn cofg(&self) -> Vf;
    fn rates(&self) -> Vf;
    /// The entry that overwrites the damping, whole quadword.
    fn set_damping(&mut self, d: Vf);
    fn set_ang_damping(&mut self, d: Vf);
    /// An instant change of velocity, `xyz` (also into the shock).
    fn add_velocity(&mut self, dv: Vf);
    /// An instant change of the body-frame rates, `xyz` (also into the twist).
    fn add_rates(&mut self, dw: Vf);
    /// A body-frame moment, `xyz`.
    fn add_moment_local(&mut self, m: Vf);
    /// A world-frame force through the centre of mass, `xyz`.
    fn push(&mut self, f: Vf);
}

/// S38's [`Body`] with the mass beside it (the port's body keeps only the inverse). Each entry is the
/// one the plane's body has; none of them tells listeners, of which the plane's body has none.
#[derive(Debug)]
pub struct Weighed<'a> {
    pub body: &'a mut Body,
    pub mass: F,
}

impl Rigid for Weighed<'_> {
    fn mass(&self) -> F {
        self.mass
    }
    fn inv_mass(&self) -> F {
        self.body.inv_mass
    }
    fn damping(&self) -> Vf {
        self.body.damping
    }
    fn ang_damping(&self) -> Vf {
        self.body.ang_damping
    }
    fn position(&self) -> Vf {
        self.body.position
    }
    fn velocity(&self) -> Vf {
        self.body.velocity
    }
    fn inv_inertia(&self) -> Vf {
        self.body.inv_inertia
    }
    fn quat(&self) -> Vf {
        self.body.quat
    }
    fn cofg(&self) -> Vf {
        self.body.cofg
    }
    fn rates(&self) -> Vf {
        self.body.rates
    }
    fn set_damping(&mut self, d: Vf) {
        self.body.damping = d;
    }
    fn set_ang_damping(&mut self, d: Vf) {
        self.body.ang_damping = d;
    }
    fn add_velocity(&mut self, dv: Vf) {
        self.body.velocity = plus(self.body.velocity, dv);
        self.body.shock = plus(self.body.shock, dv);
    }
    fn add_rates(&mut self, dw: Vf) {
        self.body.rates = plus(self.body.rates, dw);
        self.body.twist = plus(self.body.twist, dw);
    }
    fn add_moment_local(&mut self, m: Vf) {
        self.body.add_moment_local(m);
    }
    fn push(&mut self, f: Vf) {
        self.body.add_force(f);
    }
}

/// A vehicle's chassis as the other side of a contact: the fields the resolution reads and the four
/// it adds the impulse into, at the chassis offsets the doc gives (see [`crate::chassis::Chassis`] for
/// the ones it shares), and the chassis' transform.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vehicle {
    pub node: Node,
    /// `+0x190` the centre of mass in the chassis' frame.
    pub cofg: Vf,
    /// `+0x210`, `+0x214`.
    pub mass: F,
    pub inv_mass: F,
    /// `+0xf0`, `+0x104`, `+0x118`: the inverse inertia tensor's diagonal.
    pub inv_inertia: [F; 3],
    /// `+0x260` the velocity, `+0x290` the angular velocity in the chassis' frame.
    pub velocity: Vf,
    pub rates: Vf,
    /// `+0x170` and `+0x180`: what the impulse adds to the velocity and the rates is added here too.
    pub shock: Vf,
    pub twist: Vf,
}

/// A moving object that is not a body: what its two virtuals report and where it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Mover {
    /// The angular velocity (in its own frame) and the velocity.
    pub spin: Vf,
    pub velocity: Vf,
    /// `+0x254`'s transform, and `+0x190` the point its arm is measured from, in that frame.
    pub node: Node,
    pub origin: Vf,
}

/// How a collision object answers one kind of other: the spring, the impulse and the friction.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Response {
    /// The spring's stiffness and damping, and the impulse's restitution.
    pub stiffness: F,
    pub damping: F,
    pub restitution: F,
    /// Whether the impulse turns the bodies as well as moving them.
    pub angular: bool,
    /// The friction coefficient (the scenery's is only the default: see [`Collider::surfaces`]).
    pub friction: F,
    /// The lanes of the moment the spring and the friction put on the body, multiplied in.
    pub spring_mask: Vf,
    pub friction_mask: Vf,
}

/// A collision object: its tuning, the mass it shares with another object in a mutual contact, and the
/// three things the resolution writes into it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Collider {
    /// `+0x50`: lower the body's damping at a contact. `+0x58`, `+0x60`: what a square contact lowers
    /// the linear and the angular damping to.
    pub damp: bool,
    pub damp_linear: F,
    pub damp_angular: F,
    /// `+0x64`, `+0x68`, `+0x80`: the object's own mass, its inverse and inverse inertia, used instead
    /// of the bodies' in a mutual contact.
    pub mass: F,
    pub inv_mass: F,
    pub inv_inertia: Vf,
    /// `+0x90` the centre of mass in the body's frame; `+0xa0` the same turned into the world, written
    /// by [`relative_velocity`].
    pub cofg: Vf,
    pub cofg_world: Vf,
    /// `+0x110`: the collidable flags that make an other an actor. `+0x114`: the ones this object
    /// collides with.
    pub actors: u32,
    pub collides_with: u32,
    /// The object's own collidable's flags (`+0x1f8`'s `+0x84`), if it has one.
    pub collidable: Option<u32>,
    /// `+0x118`: the deepest the spring reads a contact.
    pub max_depth: F,
    /// `+0x124..`: against the scenery. `+0x13c..`: against actors and movers.
    pub scenery: Response,
    pub actor: Response,
    /// `+0x198`: the scenery's friction by surface; a contact with surface -1 uses the default.
    pub surfaces: [F; 16],
    /// `+0x1d8` the last contact's speed along its normal (negative: closing), `+0x1e0` the relative
    /// velocity it is from.
    pub normal_speed: F,
    pub relative: Vf,
}

/// One contact in a collision object's list (`0x80` bytes each).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Contact {
    /// `+0`: 0 when the contact is this body's own (the spring's anchor is on it), anything else when
    /// it is the other's.
    pub kind: u32,
    /// `+0x10` where the probe that found the contact started, in the anchoring body's model space
    /// (read by nothing here; the search writes it).
    pub from: Vf,
    /// `+0x20` the contact on the anchoring body, in its model space; `+0x30` the normal, pointing at
    /// this body; `+0x40` the contact point in the world.
    pub on_body: Vf,
    pub normal: Vf,
    pub point: Vf,
    /// `+0x50`, `+0x60`: the lever from each body's centre of mass to the point, in its own frame.
    /// Written by [`relative_velocity`] (the other's only when there is an other).
    pub arm: Vf,
    pub other_arm: Vf,
    /// `+0x70`: the surface, -1 for none.
    pub surface: i32,
    /// `+0x74`, `+0x78`: the other collision object and the mover, by index into what [`resolve`] is
    /// handed.
    pub other: Option<usize>,
    pub mover: Option<usize>,
}

/// A vehicle's chassis as the **own** side of a contact: a riding bike's collision object sits on its
/// chassis (`+0xf0`, with the transform at `+0xf8`) and has no dynamics body (`+0xf4` is 0). Every
/// step of the resolution has a branch for that, which reads [`Vehicle`]'s fields and three more.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ridden {
    pub chassis: Vehicle,
    /// `+0x60` the centre of gravity in the world: where a force's lever is measured from. (The
    /// relative velocity measures its arm in the chassis' frame, from `+0x190`, instead.)
    pub position: Vf,
    /// `+0x130` the force and `+0x140` the moment, in the chassis' frame, that the substep gathers.
    pub force: Vf,
    pub torque: Vf,
}

/// Whose collision object is resolving: one on a dynamics body (the plane's, the wreck bike's), or
/// one on a vehicle's chassis (a riding bike's).
pub enum Own<'a> {
    Body(&'a mut dyn Rigid),
    Vehicle(&'a mut Ridden),
}

impl Own<'_> {
    fn mass(&self) -> F {
        match self {
            Own::Body(b) => b.mass(),
            Own::Vehicle(r) => r.chassis.mass,
        }
    }

    fn inv_mass(&self) -> F {
        match self {
            Own::Body(b) => b.inv_mass(),
            Own::Vehicle(r) => r.chassis.inv_mass,
        }
    }

    /// The inverse inertia's diagonal. The chassis' is built on the stack from three floats, and its
    /// `w` is never read.
    fn inv_inertia(&self) -> Vf {
        match self {
            Own::Body(b) => b.inv_inertia(),
            Own::Vehicle(r) => {
                let i = r.chassis.inv_inertia;
                Vf::new(i[0].to_f32(), i[1].to_f32(), i[2].to_f32(), 0.0)
            }
        }
    }

    /// A world direction into the own side's frame.
    fn local(&self, v: Vf) -> Vf {
        match self {
            Own::Body(b) => unturn(b.quat(), v),
            Own::Vehicle(r) => r.chassis.node.unrotate(v),
        }
    }
}

/// The other side's body.
pub enum Side<'a> {
    Body(&'a mut dyn Rigid),
    Vehicle(&'a mut Vehicle),
}

/// Another collision object and its body.
pub struct Other<'a> {
    pub collider: &'a mut Collider,
    pub side: Side<'a>,
}

/// Do the two objects share the impulse and the spring by their objects' masses? When both have a
/// collidable, each must collide with the other's; when only the other has one, this one must collide
/// with it.
fn mutual(own: &Collider, other: Option<&Collider>) -> bool {
    let Some(o) = other else { return false };
    let Some(theirs) = o.collidable else { return false };
    match own.collidable {
        Some(mine) => theirs & own.collides_with != 0 && mine & o.collides_with != 0,
        None => theirs & own.collides_with != 0,
    }
}

/// Is the other an actor to this object?
fn actor(own: &Collider, other: Option<&Collider>) -> bool {
    other.and_then(|o| o.collidable).is_some_and(|f| f & own.actors != 0)
}

// --- the resolution --------------------------------------------------------------------------------

/// Resolve every contact in the list, in order. `impulses` is what the caller passes (the plane's
/// collision force passes `true`). The contacts' `other`s and `mover`s index `others` and `movers`.
pub fn resolve(
    own: &mut Collider,
    body: &mut dyn Rigid,
    contacts: &mut [Contact],
    others: &mut [Other<'_>],
    movers: &[Mover],
    impulses: bool,
) {
    resolve_on(own, &mut Own::Body(body), contacts, others, movers, impulses);
}

/// [`resolve`] for a collision object on a vehicle's chassis: a riding bike's. The damping step needs a
/// dynamics body, so it never runs here, whatever [`Collider::damp`] says.
pub fn resolve_ridden(
    own: &mut Collider,
    chassis: &mut Ridden,
    contacts: &mut [Contact],
    others: &mut [Other<'_>],
    movers: &[Mover],
    impulses: bool,
) {
    resolve_on(own, &mut Own::Vehicle(chassis), contacts, others, movers, impulses);
}

fn resolve_on(
    own: &mut Collider,
    side: &mut Own<'_>,
    contacts: &mut [Contact],
    others: &mut [Other<'_>],
    movers: &[Mover],
    impulses: bool,
) {
    for e in contacts.iter_mut() {
        let mut other = e.other.map(|k| &mut others[k]);
        let mover = e.mover.map(|k| &movers[k]);
        relative_velocity_on(own, side, e, other.as_deref_mut(), mover);
        let shared = mutual(own, other.as_deref().map(|o: &Other<'_>| &*o.collider));
        if own.normal_speed < HARD {
            if impulses && e.kind == 0 {
                impulse_on(own, side, e, other.as_deref_mut(), shared);
            }
        } else {
            spring_on(own, side, e, other.as_deref_mut(), mover.is_some(), shared);
        }
        friction_on(own, side, e, other.as_deref().map(|o: &Other<'_>| &*o.collider));
        if own.damp
            && let Own::Body(body) = side
        {
            damp(own, &mut **body, e);
        }
    }
}

/// Where the contact is on each body and how fast the two points move: writes the object's
/// [`Collider::cofg_world`], the contact's arms (the other's when there is an other or a mover, and
/// the other's own `cofg_world`), and the relative velocity and its speed along the normal.
pub fn relative_velocity(own: &mut Collider, body: &dyn Rigid, e: &mut Contact, other: Option<&mut Other<'_>>, mover: Option<&Mover>) {
    let mine = on_body(own, body, e);
    against(own, e, other, mover, mine);
}

fn relative_velocity_on(own: &mut Collider, side: &Own<'_>, e: &mut Contact, other: Option<&mut Other<'_>>, mover: Option<&Mover>) {
    let mine = match side {
        Own::Body(body) => on_body(own, &**body, e),
        Own::Vehicle(r) => on_chassis(own, &r.chassis, e),
    };
    against(own, e, other, mover, mine);
}

/// The own side's half on a dynamics body: its centre into the world, the arm, the point's velocity.
fn on_body(own: &mut Collider, body: &dyn Rigid, e: &mut Contact) -> Vf {
    let q = body.quat();
    own.cofg_world = turn(q, own.cofg);
    let centre = plus(body.position(), own.cofg_world);
    e.arm = unturn(q, minus(e.point, centre));
    plus(turn(q, cross(body.rates(), e.arm)), body.velocity())
}

/// The same on a chassis: the arm in the chassis' frame from its centre of mass (`+0x190`) moved by
/// the object's own, as the other side's chassis has it below.
fn on_chassis(own: &mut Collider, v: &Vehicle, e: &mut Contact) -> Vf {
    own.cofg_world = v.node.rotate(own.cofg);
    let local = v.node.unplace(e.point);
    e.arm = minus(local, plus(v.cofg, own.cofg_world));
    plus(v.node.rotate(cross(v.rates, e.arm)), v.velocity)
}

/// The other side's velocity at the point, and the relative velocity and its normal speed.
fn against(own: &mut Collider, e: &mut Contact, other: Option<&mut Other<'_>>, mover: Option<&Mover>, mine: Vf) {
    let theirs = match (other, mover) {
        (Some(o), _) => match &mut o.side {
            Side::Body(b) => {
                let q = b.quat();
                o.collider.cofg_world = turn(q, o.collider.cofg);
                let centre = plus(b.position(), o.collider.cofg_world);
                e.other_arm = unturn(q, minus(e.point, centre));
                plus(turn(q, cross(b.rates(), e.other_arm)), b.velocity())
            }
            Side::Vehicle(v) => {
                o.collider.cofg_world = v.node.rotate(o.collider.cofg);
                let local = v.node.unplace(e.point);
                e.other_arm = minus(local, plus(v.cofg, o.collider.cofg_world));
                plus(v.node.rotate(cross(v.rates, e.other_arm)), v.velocity)
            }
        },
        (None, Some(m)) => {
            e.other_arm = minus(m.node.unplace(e.point), m.origin);
            plus(m.node.rotate(cross(m.spin, e.other_arm)), m.velocity)
        }
        (None, None) => CLEAR,
    };
    own.relative = minus(mine, theirs);
    own.normal_speed = dot(own.relative, e.normal);
}

/// **What a wheel stands on, when it is another collision object** (S145): the wheel's
/// contact-velocity step's owner branch (`+0x1f0` set), which the step takes before the vehicle one.
/// The object's own centre of mass is put into the world first, as [`Collider::cofg_world`]
/// (`+0xa0`); then, on a dynamics body (`+0xf4`), the body's velocity and its velocity at the world
/// `point`, its arm from the body's centre turned into the body's frame — or, on a vehicle's chassis
/// (`+0xf0`, its transform `+0xf8`), the same through the transform, the arm from the chassis' centre
/// of mass moved by the object's. The two are what the step keeps at `+0x1b0` and `+0x1c0`: the
/// arithmetic the resolution's other side has ([`relative_velocity`]), with no contact to keep the
/// arm in.
pub fn other_at(o: &mut Other<'_>, point: Vf) -> (Vf, Vf) {
    match &o.side {
        Side::Body(b) => {
            let q = b.quat();
            o.collider.cofg_world = turn(q, o.collider.cofg);
            let centre = plus(b.position(), o.collider.cofg_world);
            let arm = unturn(q, minus(point, centre));
            (b.velocity(), plus(turn(q, cross(b.rates(), arm)), b.velocity()))
        }
        Side::Vehicle(v) => {
            o.collider.cofg_world = v.node.rotate(o.collider.cofg);
            let local = v.node.unplace(point);
            let arm = minus(local, plus(v.cofg, o.collider.cofg_world));
            (v.velocity, plus(v.node.rotate(cross(v.rates, arm)), v.velocity))
        }
    }
}

/// A force at a world point on the body: the moment about the centre of mass, into the body's frame and
/// masked lane by lane, then the force itself.
fn apply(own: &Collider, side: &mut Own<'_>, f: Vf, at: Vf, mask: Vf) {
    match side {
        Own::Body(body) => {
            let r = minus(at, plus(body.position(), own.cofg_world));
            let m = unturn(body.quat(), cross(r, f));
            body.add_moment_local(times(mask, m));
            body.push(f);
        }
        // Straight into the chassis' two accumulators, `.xyz`: no virtual, no listener.
        Own::Vehicle(c) => {
            let r = minus(at, plus(c.position, own.cofg_world));
            let m = c.chassis.node.unrotate(cross(r, f));
            c.torque = plus(c.torque, times(mask, m));
            c.force = plus(c.force, f);
        }
    }
}

/// The damped spring that pushes the body out of a contact it is not closing on fast:
/// `max(k·min(depth, max) − d·speed, 0)` along the normal, scaled down by the other's share of the
/// mass, at the anchor (the contact on whichever body the contact's kind says).
pub fn spring(own: &Collider, body: &mut dyn Rigid, e: &Contact, other: Option<&mut Other<'_>>, mover: bool, shared: bool) {
    spring_on(own, &mut Own::Body(body), e, other, mover, shared);
}

fn spring_on(own: &Collider, side: &mut Own<'_>, e: &Contact, other: Option<&mut Other<'_>>, mover: bool, shared: bool) {
    let r = if mover || actor(own, other.as_deref().map(|o| &*o.collider)) { own.actor } else { own.scenery };
    let n = e.normal;
    let (anchor, depth) = if e.kind == 0 {
        let anchor = match side {
            Own::Body(body) => plus(turn(body.quat(), minus(e.on_body, body.cofg())), body.position()),
            Own::Vehicle(c) => c.chassis.node.place(e.on_body),
        };
        (anchor, dot(minus(e.point, anchor), n))
    } else {
        let o = other.as_deref().expect("a contact of the other's kind always has the other");
        let anchor = match &o.side {
            Side::Body(b) => plus(turn(b.quat(), minus(e.on_body, b.cofg())), b.position()),
            Side::Vehicle(v) => v.node.place(e.on_body),
        };
        (anchor, -dot(minus(e.point, anchor), n))
    };
    let share = match other.as_deref() {
        None => F::ONE,
        Some(o) if shared => (o.collider.mass * own.inv_mass).min(F::ONE),
        Some(o) => {
            let theirs = match &o.side {
                Side::Body(b) => b.mass(),
                Side::Vehicle(v) => v.mass,
            };
            (theirs * side.inv_mass()).min(F::ONE)
        }
    };
    let push = r.stiffness * depth.min(own.max_depth);
    let f = share * (push - r.damping * dot(own.relative, n)).max(F::ZERO);
    apply(own, side, scaled(n, f), anchor, r.spring_mask);
}

/// `R · ((a × R)·I⁻¹ × a)`: the inverse-inertia term of the impulse for one body, with the normal
/// already in its frame.
fn turning(r: Vf, arm: Vf, inv_inertia: Vf) -> F {
    let a = times(cross(arm, r), inv_inertia);
    dot(r, cross(a, arm))
}

/// The impulse that stops a hard contact: `J = n · −(1 + e)·speed / (m₁⁻¹ + m₂⁻¹ + k₁ + k₂)`, `J·m₁⁻¹`
/// onto the body's velocity and `−J·m₂⁻¹` onto the other's, and the matching turn when the response is
/// angular.
pub fn impulse(own: &Collider, body: &mut dyn Rigid, e: &Contact, other: Option<&mut Other<'_>>, shared: bool) {
    impulse_on(own, &mut Own::Body(body), e, other, shared);
}

fn impulse_on(own: &Collider, side: &mut Own<'_>, e: &Contact, mut other: Option<&mut Other<'_>>, shared: bool) {
    let r = if actor(own, other.as_deref().map(|o| &*o.collider)) { own.actor } else { own.scenery };
    let (m1, i1) = if shared { (own.inv_mass, own.inv_inertia) } else { (side.inv_mass(), side.inv_inertia()) };
    let (m2, i2) = match other.as_deref() {
        None => (F::ZERO, CLEAR),
        Some(o) if shared => (o.collider.inv_mass, o.collider.inv_inertia),
        Some(o) => match &o.side {
            Side::Body(b) => (b.inv_mass(), b.inv_inertia()),
            // The original builds this quadword on the stack from the three diagonal floats; its `w`
            // is never read (every use is `.xyz`).
            Side::Vehicle(v) => (v.inv_mass, Vf::new(v.inv_inertia[0].to_f32(), v.inv_inertia[1].to_f32(), v.inv_inertia[2].to_f32(), 0.0)),
        },
    };
    let n = e.normal;
    let k1 = if r.angular { turning(side.local(n), e.arm, i1) } else { F::ZERO };
    let k2 = match other.as_deref() {
        Some(o) if r.angular => {
            let local = match &o.side {
                Side::Body(b) => unturn(b.quat(), n),
                Side::Vehicle(v) => v.node.unrotate(n),
            };
            turning(local, e.other_arm, i2)
        }
        _ => F::ZERO,
    };
    let size = (F::ONE / (k2 + (k1 + (m1 + m2)))) * (-(F::ONE + r.restitution) * own.normal_speed);
    let j = scaled(n, size);
    let dv = scaled(j, m1);
    let dw = r.angular.then(|| times(cross(e.arm, side.local(j)), i1));
    match side {
        Own::Body(body) => {
            body.add_velocity(dv);
            if let Some(dw) = dw {
                body.add_rates(dw);
            }
        }
        Own::Vehicle(c) => {
            let v = &mut c.chassis;
            v.shock = plus(v.shock, dv);
            v.velocity = plus(v.velocity, dv);
            if let Some(dw) = dw {
                v.twist = plus(v.twist, dw);
                v.rates = plus(v.rates, dw);
            }
        }
    }
    let Some(o) = other.as_deref_mut() else { return };
    let dv = scaled(negated(j), m2);
    match &mut o.side {
        Side::Body(b) => {
            b.add_velocity(dv);
            if r.angular {
                let dw = times(negated(cross(e.other_arm, unturn(b.quat(), j))), i2);
                b.add_rates(dw);
            }
        }
        Side::Vehicle(v) => {
            v.shock = plus(v.shock, dv);
            v.velocity = plus(v.velocity, dv);
            if r.angular {
                let dw = times(negated(cross(e.other_arm, v.node.unrotate(j))), i2);
                v.twist = plus(v.twist, dw);
                v.rates = plus(v.rates, dw);
            }
        }
    }
}

/// Friction against the sliding part of the relative velocity: `32.2·m·μ·min(0.1·slide, 1)` against
/// it, at the contact point. `μ` is the actors' when the other is an actor, otherwise the contact's
/// surface's (or the default for surface -1).
pub fn friction(own: &Collider, body: &mut dyn Rigid, e: &Contact, other: Option<&Collider>) {
    friction_on(own, &mut Own::Body(body), e, other);
}

fn friction_on(own: &Collider, side: &mut Own<'_>, e: &Contact, other: Option<&Collider>) {
    let (mu, mask) = if actor(own, other) {
        (own.actor.friction, own.actor.friction_mask)
    } else if e.surface == -1 {
        (own.scenery.friction, own.scenery.friction_mask)
    } else {
        (own.surfaces[e.surface as usize], own.scenery.friction_mask)
    };
    let n = e.normal;
    let slide = minus(scaled(n, dot(own.relative, n)), own.relative);
    let along = unit(slide);
    let speed = dot(slide, slide).sqrt();
    let f = F(WEIGHT) * (side.mass() * ((F(SLIDE) * speed).min(F::ONE) * mu));
    apply(own, side, scaled(along, f), e.point, mask);
}

/// One lane of the damping a contact asks for: `|c|` remapped from `0..1` onto `1..k` and clamped
/// between the two, as the original's remap does it (the `0..1` range is a constant one).
fn toward(c: F, k: F) -> F {
    let t = (c - F::ZERO) / (F::ONE - F::ZERO);
    let v = Acc::adda(F::ZERO, F::ONE).madd(t, k - F::ONE);
    v.max(F::ONE.min(k)).min(F::ONE.max(k))
}

/// Lower the body's damping by the contact: each linear lane toward [`Collider::damp_linear`] by how
/// much the normal lies along that world axis, each angular lane toward [`Collider::damp_angular`] by
/// how much the axis the contact would turn the body about (`arm × n`, in the body's frame, made unit)
/// lies along that body axis. Only ever lowered: the lower of the old and the new is kept.
///
/// The `w` lanes the original stores are what its stack held there, which the friction just before
/// left as the normal's `w`: so `normal.w` here, for both.
pub fn damp(own: &Collider, body: &mut dyn Rigid, e: &Contact) {
    let n = e.normal;
    let k = own.damp_linear;
    let (x, y, z) = (toward(F(n.x()).abs(), k), toward(F(n.y()).abs(), k), toward(F(n.z()).abs(), k));
    let d = body.damping();
    body.set_damping(Vf::new(F(d.x()).min(x).to_f32(), F(d.y()).min(y).to_f32(), F(d.z()).min(z).to_f32(), n.w()));
    let mut c = cross(e.arm, unturn(body.quat(), n));
    let len2 = dot(c, c);
    if len2 == F::ZERO {
        c = CLEAR;
    } else {
        let r = F::ONE.rsqrt(len2);
        c = Vf::new((F(c.x()) * r).to_f32(), (F(c.y()) * r).to_f32(), (F(c.z()) * r).to_f32(), c.w());
    }
    let k = own.damp_angular;
    let (x, y, z) = (toward(F(c.x()).abs(), k), toward(F(c.y()).abs(), k), toward(F(c.z()).abs(), k));
    let d = body.ang_damping();
    body.set_ang_damping(Vf::new(F(d.x()).min(x).to_f32(), F(d.y()).min(y).to_f32(), F(d.z()).min(z).to_f32(), n.w()));
}
