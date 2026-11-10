//! **SLUS-21104's rider ragdoll world, on RAM** (S280): the world the rider's ragdoll parts hang in,
//! stepped whole — its two forces, its bodies' integrate and its constraints, three passes a substep
//! — with [`super::bone`]'s collisions in their places. Most of it is MX Unleashed's ragdoll in MX vs.
//! ATV's layout: the pin is [`crate::ragdoll::Pin`] (with the first body's inverse mass and inertia
//! the pin's own copies in the two-body solve), the box is [`crate::ragdoll_contact::BikeBox`] and
//! [`crate::ragdoll_contact::NodeBox`] with one more way to point it, and the rider's force is
//! [`crate::ragdoll::Ragdoll::forces`] with the pose's points out of the parts' nodes. Where the
//! original keeps them: research/oracle.txt `# S280`.
//!
//! - [`step`]: the world's step ([`crate::biplane::World::step`]) over the lists in RAM, the
//!   constraints and forces sorted by the caller's [`Rig`].
//! - [`pin`], [`frame_box`]: the two kinds of constraint that are not bone collisions.
//! - [`rider_force`], [`directed_force`]: the two forces.
//! - [`add_force`], [`add_moment`], [`add_moment_world`]: a body's three accumulators, as the forces
//!   reach them.
//!
//! A pin's words: on (`+0x4`), type (`+0x0`, 2 for a joint) and owner (`+0xc`); the first body's
//! inverse mass (`+0x14`) and inverse inertia (`+0x20`) as the pin keeps them; the two anchors
//! (`+0x30`, `+0x40`); the target and its velocity (`+0x50`, `+0x60`) and the two words that point
//! at what they follow (`+0x70`, `+0x74`); the two bodies (`+0x78`, `+0x7c`); limited (`+0x80`), the
//! limits (`+0x90`, `+0xa0`), hit and sticky (`+0xb0`, `+0xc0`); the joint's max angle (`+0xd4`),
//! spring (`+0xe0`) and critical damping (`+0x100`); and the gains (`+0x110`).
//!
//! A box's words: on (`+0x4`); the node it may be in (`+0x14`); its own frame (`+0x20`, four rows,
//! the origin last), centre (`+0x70`) and half-size (`+0x80`); the words that point at a frame
//! (`+0x60`), a centre (`+0x90`), a half-size (`+0x94`) and a carried velocity (`+0x98`); its body
//! (`+0x9c`) and the body it may be in (`+0xa0`).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::bone::{B_INV_INERTIA, B_INV_MASS, B_POSITION, B_QUAT, B_RATES, B_VELOCITY, ORIGIN};
use super::node::{self, Ram};
use super::seed;
use crate::biplane::{self, Dynamics, List, unturn};
use crate::chassis::{normalize, turn};
use crate::math::Mat4;
use crate::ragdoll::{Bone, Pin, Tables, axis_angle, product, rel};
use crate::ragdoll_contact::BikeBox;
use core::cell::RefCell;
use ps2_float::fpu;
use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf};

/// The world's iterations and rate, and its lists (each a count, then the array two words on):
/// every body (`+0x08`), the joints the rider's force walks (`+0x38`), and the step's bodies, forces,
/// constraints, preforce and post listeners.
pub const W_ITERATIONS: u32 = 0x00;
pub const W_RATE: u32 = 0x04;
pub const W_ALL: u32 = 0x08;
pub const W_JOINTS: u32 = 0x38;
pub const W_BODIES: u32 = 0x50;
pub const W_FORCES: u32 = 0x68;
pub const W_CONSTRAINTS: u32 = 0x80;
pub const W_PREFORCE: u32 = 0x98;
pub const W_POST: u32 = 0xb0;
/// A list's array, past its count.
const ARRAY: u32 = 0x08;

/// A pin's words (the module's doc lists them).
pub const ON: u32 = 0x04;
pub const TYPE: u32 = 0x00;
pub const JOINT: u32 = 2;
pub const OWNER: u32 = 0x0c;
pub const OWN_INV_MASS: u32 = 0x14;
pub const OWN_INV_INERTIA: u32 = 0x20;
pub const ANCHOR: u32 = 0x30;
pub const ANCHOR2: u32 = 0x40;
pub const TARGET: u32 = 0x50;
pub const TARGET_VELOCITY: u32 = 0x60;
pub const FOLLOW: u32 = 0x70;
pub const FOLLOW_VELOCITY: u32 = 0x74;
pub const FIRST: u32 = 0x78;
pub const SECOND: u32 = 0x7c;
pub const LIMITED: u32 = 0x80;
pub const LOWER: u32 = 0x90;
pub const UPPER: u32 = 0xa0;
pub const HIT: u32 = 0xb0;
pub const STICKY: u32 = 0xc0;
pub const MAX_ANGLE: u32 = 0xd4;
pub const SPRING: u32 = 0xe0;
pub const CRITICAL: u32 = 0x100;
pub const GAINS: u32 = 0x110;

/// A box's words (the module's doc lists them).
pub const X_NODE: u32 = 0x14;
pub const X_FRAME: u32 = 0x20;
pub const X_CENTRE: u32 = 0x70;
pub const X_HALF: u32 = 0x80;
pub const X_FRAME_AT: u32 = 0x60;
pub const X_CENTRE_AT: u32 = 0x90;
pub const X_HALF_AT: u32 = 0x94;
pub const X_CARRIED_AT: u32 = 0x98;
pub const X_BODY: u32 = 0x9c;
pub const X_IN: u32 = 0xa0;

/// A body's words past [`super::bone`]'s: its holder's index (`+0x14`), mass (`+0x18`), force
/// (`+0xa0`), moment (`+0x1b0`), the orientation a hold limits it against (`+0x180`), its point in
/// the pose (`+0x1a0`, in its node's frame) and its listener list (`+0xe8`).
pub const B_HOLDER: u32 = 0x14;
pub const B_MASS: u32 = 0x18;
pub const B_FORCE: u32 = 0xa0;
pub const B_REFERENCE: u32 = 0x180;
pub const B_POSE_POINT: u32 = 0x1a0;
pub const B_MOMENT: u32 = 0x1b0;
pub const B_LISTENERS: u32 = 0xe8;
/// How many words of a body [`step`]'s integrate reads and writes.
const BODY_WORDS: u32 = 0x230 / 4;

/// The rider's words the force reads: the rests table (`+0x1fc`, `0x40` an entry, the quaternion at
/// `+0x20`), riding (`+0x254`), the two torque switches (`+0x264`, `+0x268`), the world (`+0x27c`),
/// the gravity scale (`+0x280`), the holders (`+0x290` count, `+0x2a4` array, `0xb0` each), the
/// mode (`+0x368`), the bike's record (`+0x378`), the wreck fade's length and what is left of it
/// (`+0x5d4`, `+0x5d8`) and the pose's spring and damping (`+0x5e0`, `+0x5f0`).
pub const R_RESTS: u32 = 0x1fc;
pub const R_RIDING: u32 = 0x254;
pub const R_TORQUES: u32 = 0x264;
pub const R_STIFF: u32 = 0x268;
pub const R_WORLD: u32 = 0x27c;
pub const R_GRAVITY: u32 = 0x280;
pub const R_HOLDERS: u32 = 0x290;
pub const R_HOLDER_AT: u32 = 0x2a4;
pub const R_MODE: u32 = 0x368;
pub const R_RECORD: u32 = 0x378;
pub const R_FADE_LENGTH: u32 = 0x5d4;
pub const R_FADE_LEFT: u32 = 0x5d8;
pub const R_POSE_SPRING: u32 = 0x5e0;
pub const R_POSE_DAMPING: u32 = 0x5f0;
/// The mode the torques fade in, halved, with the first body taking them back.
pub const WRECK_MODE: u32 = 2;
/// A holder's stride and words: its rest's index (`+0x40`), its node (`+0xa0`) and its body (`+0xa4`).
pub const H_STRIDE: u32 = 0xb0;
pub const H_REST: u32 = 0x40;
pub const H_NODE: u32 = 0xa0;
pub const H_BODY: u32 = 0xa4;
/// A rest's stride and its quaternion.
const REST_STRIDE: u32 = 0x40;
const REST_QUAT: u32 = 0x20;
/// The record's rows (`+0x20`, three), velocity (`+0xa0`), spin (`+0xb8`) and centre (`+0x1b0`).
pub const C_ROWS: u32 = 0x20;
pub const C_VELOCITY: u32 = 0xa0;
pub const C_SPIN: u32 = 0xb8;
pub const C_CENTRE: u32 = 0x1b0;

/// A directed force's words: on (`+0x0`), the vector it points at (`+0xc`, else its own at `+0x10`),
/// the world it pushes every enabled body of (`+0x20`) and the one body it pushes instead (`+0x24`).
pub const D_VECTOR_AT: u32 = 0x0c;
pub const D_VECTOR: u32 = 0x10;
pub const D_WORLD: u32 = 0x20;
pub const D_BODY: u32 = 0x24;
/// A wrapping force's listener.
pub const F_LISTENER: u32 = 0x0c;

/// Gravity, feet a second a second.
const GRAVITY: f32 = 32.2;

/// What a constraint is, as [`step`] solves it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint {
    Pin,
    /// A bone collision: [`Rig::bone`].
    Bone,
    Box,
    /// Anything else: [`Rig::other_constraint`].
    Other,
}

/// What a force is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Force {
    /// A wrapper round a listener ([`F_LISTENER`]) that is called when it is set.
    Wrapper,
    Directed,
    Other,
}

/// What a wrapped listener is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Listener {
    /// The rider's force, for the rider at this address.
    Rider(u32),
    Other,
}

/// **The caller's side of the world**: what each list entry is (the original goes by its vtable),
/// the bone collision (which needs the scene) and whatever is not ported.
pub trait Rig<R: Ram> {
    fn constraint(&mut self, m: &mut R, c: u32) -> Constraint;
    fn force(&mut self, m: &mut R, f: u32) -> Force;
    fn listener(&mut self, m: &mut R, l: u32) -> Listener;
    /// The bone collision at `part`, its step `h`.
    fn bone(&mut self, m: &mut R, part: u32, h: F);
    fn other_constraint(&mut self, m: &mut R, c: u32, h: F, first: bool);
    fn other_force(&mut self, m: &mut R, f: u32, h: F);
    fn other_listener(&mut self, m: &mut R, l: u32, h: F);
    fn preforce(&mut self, m: &mut R, e: u32, dt: F);
    fn post(&mut self, m: &mut R, e: u32, dt: F);
}

/// A quadword as `lqc2` loads it into the VU (each lane read as an operand).
fn quad(m: &mut impl Ram, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| m.word((at & !0xf) + k)))
}

fn put_quad(m: &mut impl Ram, at: u32, v: Vf) {
    for (k, w) in v.to_bits().into_iter().enumerate() {
        m.set((at & !0xf) + 4 * k as u32, w);
    }
}

/// Four words copied as they are (`ld`/`sd`, `lw`/`sw`).
fn copy(m: &mut impl Ram, from: u32, to: u32) {
    for k in [0, 4, 8, 12] {
        let w = m.word(from + k);
        m.set(to + k, w);
    }
}

/// A float word as `lwc1` loads it into the FPU.
fn f(m: &mut impl Ram, at: u32) -> F {
    F::from_bits(m.word(at))
}

/// Entry `i` of the list at `list`.
fn entry(m: &mut impl Ram, list: u32, i: u32) -> u32 {
    let array = m.word(list + ARRAY);
    m.word(array.wrapping_add(4 * i))
}

fn count(m: &mut impl Ram, list: u32) -> i32 {
    m.word(list) as i32
}

/// `a.xyz + b.xyz`, `w` from `a`.
fn plus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `a.xyz - b.xyz`, `w` from `a`.
fn minus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `a.xyz * b.xyz`, `w` from `a`.
fn times(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.mul(b))
}

/// `a.xyz` times `s`, `w` from `a`.
fn scaled(a: Vf, s: f32) -> Vf {
    a.merge(Dest::XYZ, a.mul(Vf::splat(s)))
}

/// `a × b` into `a`'s `xyz` (`vopmula a, b` then `vopmsub.xyz a, b, a`), `w` from `a`.
fn cross(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, Vf::opmsub(Vf::opmula(a, b), b, a))
}

/// `-v`, all four lanes, off `vf0`.
fn negate(v: Vf) -> Vf {
    Vf::msub(Vf::VF0.mul(Vf::VF0.bc(Field::X)), v, Vf::VF0.bc(Field::W))
}

/// The words of a body a pin reads, as the plane's body.
fn pin_body(m: &mut impl Ram, at: u32) -> biplane::Body {
    biplane::Body {
        enabled: m.word(at) != 0,
        inv_mass: f(m, at + B_INV_MASS),
        position: quad(m, at + B_POSITION),
        velocity: quad(m, at + B_VELOCITY),
        inv_inertia: quad(m, at + B_INV_INERTIA),
        quat: quad(m, at + B_QUAT),
        rates: quad(m, at + B_RATES),
        ..biplane::Body::default()
    }
}

/// What a pin's solve writes of a body: its position, velocity, quaternion and rates.
fn put_pin_body(m: &mut impl Ram, at: u32, b: &biplane::Body) {
    put_quad(m, at + B_POSITION, b.position);
    put_quad(m, at + B_VELOCITY, b.velocity);
    put_quad(m, at + B_QUAT, b.quat);
    put_quad(m, at + B_RATES, b.rates);
}

/// **A pin's solve**, once per constraint pass: nothing unless it is on. With a second body and
/// both bodies enabled, the twist limit (when limited) and the two-body solve, the first body's
/// inverse mass and inertia in the latter the pin's own copies. Otherwise, the first body enabled,
/// the turn limit against the body's reference (when limited), the target set — what the pin
/// follows (and its velocity, when that is pointed at too), or the second anchor — and the one-body
/// solve. Either way the hit starts at `(0, 0, 0, 1)`.
pub fn pin(m: &mut impl Ram, at: u32, tables: &Tables) {
    if m.word(at + ON) == 0 {
        return;
    }
    let (first, second) = (m.word(at + FIRST), m.word(at + SECOND));
    if second != 0 && m.word(first) != 0 && m.word(second) != 0 {
        copy(m, ORIGIN, at + HIT);
        let mut p = read_pin(m, at);
        let (mut b1, mut b2) = (pin_body(m, first), pin_body(m, second));
        if m.word(at + LIMITED) != 0 {
            p.twist_limit(&mut b1, &mut b2, tables);
        }
        b1.inv_mass = f(m, at + OWN_INV_MASS);
        b1.inv_inertia = quad(m, at + OWN_INV_INERTIA);
        p.two_body(&mut b1, &mut b2, &tables.curve);
        put_pin_body(m, first, &b1);
        put_pin_body(m, second, &b2);
        put_pin(m, at, &p);
        m.set(at + GAINS + 4, p.gains.y().to_bits());
        return;
    }
    if m.word(first) == 0 {
        return;
    }
    copy(m, ORIGIN, at + HIT);
    let mut p = read_pin(m, at);
    let mut bone = Bone { body: pin_body(m, first), reference: quad(m, first + B_REFERENCE), ..Bone::default() };
    if m.word(at + LIMITED) != 0 {
        p.turn_limit(&mut bone);
    }
    let follow = m.word(at + FOLLOW);
    if follow == 0 {
        copy(m, at + ANCHOR2, at + TARGET);
    } else {
        copy(m, follow, at + TARGET);
        let velocity = m.word(at + FOLLOW_VELOCITY);
        if velocity != 0 {
            copy(m, velocity, at + TARGET_VELOCITY);
        }
    }
    p.target = quad(m, at + TARGET);
    p.target_velocity = quad(m, at + TARGET_VELOCITY);
    p.one_body(&mut bone.body, &tables.curve);
    put_pin_body(m, first, &bone.body);
    put_pin(m, at, &p);
}

fn read_pin(m: &mut impl Ram, at: u32) -> Pin {
    Pin {
        anchors: [quad(m, at + ANCHOR), quad(m, at + ANCHOR2)],
        lower: quad(m, at + LOWER),
        upper: quad(m, at + UPPER),
        hit: quad(m, at + HIT),
        sticky: quad(m, at + STICKY),
        gains: quad(m, at + GAINS),
        ..Pin::default()
    }
}

/// What both of a pin's solves write of the pin besides the target: the hit and sticky lanes
/// (rewritten whole, unchanged lanes as they were read) and the first gain.
fn put_pin(m: &mut impl Ram, at: u32, p: &Pin) {
    put_quad(m, at + HIT, p.hit);
    put_quad(m, at + STICKY, p.sticky);
    m.set(at + GAINS, p.gains.x().to_bits());
}

/// **A box's solve**: nothing unless it is on and its body is enabled. In another body, the box is
/// in that body's frame and the velocity measured against the box's point's on it ([`in_body`]).
/// Otherwise in a node ([`in_node`]), or in the frame, centre and half-size it points at (with the
/// velocity it carries, which is read off address zero when it points at none), or in its own.
pub fn frame_box(m: &mut impl Ram, at: u32) {
    if m.word(at + ON) == 0 {
        return;
    }
    let body = m.word(at + X_BODY);
    if m.word(body) == 0 {
        return;
    }
    if m.word(at + X_IN) != 0 {
        return in_body(m, at, body);
    }
    if m.word(at + X_NODE) != 0 {
        return in_node(m, at, body);
    }
    let (frame, centre, half) = (m.word(at + X_FRAME_AT), m.word(at + X_CENTRE_AT), m.word(at + X_HALF_AT));
    let b = if frame == 0 || half == 0 || centre == 0 {
        BikeBox { frame: mat4(m, at + X_FRAME), centre: quad(m, at + X_CENTRE), half: quad(m, at + X_HALF), carried: None }
    } else {
        let carried = m.word(at + X_CARRIED_AT);
        BikeBox { frame: mat4(m, frame), centre: quad(m, centre), half: quad(m, half), carried: Some(quad(m, carried)) }
    };
    let mut body_now = pin_body(m, body);
    b.hold(true, &mut body_now);
    put_quad(m, body + B_POSITION, body_now.position);
    put_quad(m, body + B_VELOCITY, body_now.velocity);
}

fn mat4(m: &mut impl Ram, at: u32) -> Mat4 {
    Mat4([0, 0x10, 0x20, 0x30].map(|k| quad(m, at + k)))
}

/// The box's centre `p` held inside `±half`, lane by lane; a lane held also zeroes `v`'s.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn hold(p: &mut Vf, v: &mut Vf, half: [F; 3]) {
    for (k, h) in half.into_iter().enumerate() {
        let at = F(p[k]);
        if !(at <= h) {
            p[k] = h.0;
            v[k] = 0.0;
        } else if at < -h {
            p[k] = (-h).0;
            v[k] = 0.0;
        }
    }
}

fn half(m: &mut impl Ram, at: u32) -> [F; 3] {
    [0, 4, 8].map(|k| f(m, at + k))
}

/// The box in another body: the box's point's velocity on that body (its rates across the box's
/// centre, plus its velocity) taken out of the body's, both the position (from the other body's
/// centre) and the velocity unturned into the other's frame, held, and turned back out.
fn in_body(m: &mut impl Ram, at: u32, body: u32) {
    let other = m.word(at + X_IN);
    let centre = quad(m, at + X_CENTRE);
    let carried = plus(cross(quad(m, other + B_RATES), centre), quad(m, other + B_VELOCITY));
    let q = quad(m, other + B_QUAT);
    let mut v = unturn(q, minus(quad(m, body + B_VELOCITY), carried));
    let mut p = minus(unturn(q, minus(quad(m, body + B_POSITION), quad(m, other + B_POSITION))), centre);
    hold(&mut p, &mut v, half(m, at + X_HALF));
    let p = plus(p, centre);
    put_quad(m, body + B_POSITION, turn(q, p));
    let moved = plus(quad(m, body + B_POSITION), quad(m, other + B_POSITION));
    put_quad(m, body + B_POSITION, moved);
    let q = quad(m, other + B_QUAT);
    put_quad(m, body + B_VELOCITY, turn(q, v));
    let v = plus(quad(m, body + B_VELOCITY), carried);
    put_quad(m, body + B_VELOCITY, v);
}

/// The box in a node: the body's centre into the node's frame, held (the velocity taken into the
/// frame too, for nothing), and back out — with the held centre, turned out of the frame, written
/// as the velocity, as MX Unleashed's does.
fn in_node(m: &mut impl Ram, at: u32, body: u32) {
    let n = m.word(at + X_NODE);
    let pos = [0, 4, 8, 12].map(|k| m.word(body + B_POSITION + k));
    let centre = quad(m, at + X_CENTRE);
    let mut p = minus(Vf::from_bits(node::unplace(m, n, pos)), centre);
    let vel = [0, 4, 8, 12].map(|k| m.word(body + B_VELOCITY + k));
    let mut v = Vf::from_bits(node::local(m, n, vel));
    hold(&mut p, &mut v, half(m, at + X_HALF));
    let p = plus(p, quad(m, at + X_CENTRE)).to_bits();
    let placed = node::place(m, n, p);
    for (k, w) in placed.into_iter().enumerate() {
        m.set(body + B_POSITION + 4 * k as u32, w);
    }
    let turned = node::rotate(m, n, p);
    for (k, w) in turned.into_iter().enumerate() {
        m.set(body + B_VELOCITY + 4 * k as u32, w);
    }
}

/// **A body's force added to** (`.xyz`, the force's `w` kept). The body's listeners are not called:
/// the rider's bodies have none.
pub fn add_force(m: &mut impl Ram, body: u32, v: Vf) {
    let sum = plus(quad(m, body + B_FORCE), v);
    put_quad(m, body + B_FORCE, sum);
}

/// **A body's moment added to**, in the body's frame (`.xyz`; no listeners, as [`add_force`]).
pub fn add_moment(m: &mut impl Ram, body: u32, v: Vf) {
    let sum = plus(quad(m, body + B_MOMENT), v);
    put_quad(m, body + B_MOMENT, sum);
}

/// [`add_moment`] of a moment in the world's frame: unturned into the body's first.
pub fn add_moment_world(m: &mut impl Ram, body: u32, v: Vf) {
    let q = quad(m, body + B_QUAT);
    add_moment(m, body, unturn(q, v));
}

/// **The directed force**: when on, the vector it points at (or its own) added to its one body's
/// force when it has one and that is enabled; with no body (or a disabled one), to every enabled
/// body of its world, when it has one. The vector is read again for each body.
pub fn directed_force(m: &mut impl Ram, at: u32) {
    if m.word(at) == 0 {
        return;
    }
    let pointed = m.word(at + D_VECTOR_AT) != 0;
    let vector = |m: &mut dyn FnMut(u32) -> u32| if pointed { m(at + D_VECTOR_AT) } else { at + D_VECTOR };
    let body = m.word(at + D_BODY);
    if body != 0 && m.word(body) != 0 {
        let v = vector(&mut |a| m.word(a));
        let v = quad(m, v);
        add_force(m, body, v);
        return;
    }
    let world = m.word(at + D_WORLD);
    if world == 0 {
        return;
    }
    let mut i = 0;
    while i < count(m, world + W_ALL) {
        let b = entry(m, world + W_ALL, i as u32);
        if m.word(b) != 0 {
            let v = vector(&mut |a| m.word(a));
            let v = quad(m, v);
            add_force(m, b, v);
        }
        i += 1;
    }
}

/// **The rider's force**, the listener of the world's first force. For each of the rider's holders,
/// gravity on its body (`y`, scaled by the rider's gravity scale); riding, the pose's spring and
/// damper pulling the body to its point of the pose (the body's pose point placed by its holder's
/// node), the target's velocity the bike's carried round by its spin about its centre. Then, with
/// both torque switches on, each of the rider's joints' torque springs ([`torque`]).
pub fn rider_force(m: &mut impl Ram, rider: u32, tables: &Tables) {
    let record = m.word(rider + R_RECORD);
    let s = Vf::new(0.0, 0.0, f(m, record + C_SPIN).0, 0.0);
    let rows = [0, 0x10, 0x20].map(|k| quad(m, record + C_ROWS + k));
    let spin = Vf::madd(Vf::madd(rows[0].mul(s.bc(Field::X)), rows[1], s.bc(Field::Y)), rows[2], s.bc(Field::Z));
    let mut i = 0;
    while i < count(m, rider + R_HOLDERS) {
        let holder = m.word(rider + R_HOLDER_AT).wrapping_add(i as u32 * H_STRIDE);
        let body = m.word(holder + H_BODY);
        let weight = fpu::mul(f(m, rider + R_GRAVITY).0, f(m, body + B_MASS).0);
        let y = fpu::msub(fpu::adda(0.0, f(m, body + B_FORCE + 4).0), GRAVITY, weight);
        m.set(body + B_FORCE + 4, y.to_bits());
        if m.word(rider + R_RIDING) != 0 {
            let n = m.word(holder + H_NODE);
            let point = [0, 4, 8, 12].map(|k| m.word(body + B_POSE_POINT + k));
            let x = Vf::from_bits(node::place(m, n, point));
            let record = m.word(rider + R_RECORD);
            let arm = minus(x, quad(m, record + C_CENTRE));
            let carried = plus(quad(m, record + C_VELOCITY), cross(spin, arm));
            let dv = minus(carried, quad(m, body + B_VELOCITY));
            let dp = minus(x, quad(m, body + B_POSITION));
            let q = quad(m, body + B_QUAT);
            let (dv, dp) = (unturn(q, dv), unturn(q, dp));
            let damped = times(quad(m, rider + R_POSE_DAMPING), dv);
            let sprung = times(quad(m, rider + R_POSE_SPRING), dp);
            let pull = turn(q, plus(sprung, damped));
            let sum = plus(quad(m, body + B_FORCE), pull);
            put_quad(m, body + B_FORCE, sum);
        }
        i += 1;
    }
    if m.word(rider + R_TORQUES) == 0 || m.word(rider + R_STIFF) == 0 {
        return;
    }
    let mut i = 0;
    loop {
        let world = m.word(rider + R_WORLD);
        if i >= count(m, world + W_JOINTS) {
            break;
        }
        let joint = entry(m, world + W_JOINTS, i as u32);
        if m.word(joint + TYPE) == JOINT && m.word(joint + OWNER) == rider {
            torque(m, rider, joint, tables);
        }
        i += 1;
    }
}

/// One joint's torque spring, [`crate::ragdoll::Ragdoll`]'s: the second body's turn away from where
/// its rest on the first would have it, as an angle about an axis in its own frame, each lane
/// clamped to the joint's max angle (a lane the pin has just stopped at its limit, still pushing
/// that way, let go), against the critically damped rates (clamped to `±1`). The second body takes
/// the torque; in the wreck mode faded in and halved, the first body taking it back. The sticky
/// lanes are cleared.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn torque(m: &mut impl Ram, rider: u32, joint: u32, tables: &Tables) {
    let (first, second) = (m.word(joint + FIRST), m.word(joint + SECOND));
    let rates = quad(m, second + B_RATES);
    let clamp = |v: f32| fpu::min(fpu::max(v, -1.0), 1.0);
    let wc = Vf::new(clamp(rates.x()), clamp(rates.y()), clamp(rates.z()), rates.w());
    let index = m.word(second + B_HOLDER);
    let holder = m.word(rider + R_HOLDER_AT).wrapping_add(H_REST).wrapping_add(index.wrapping_mul(H_STRIDE));
    let rest = m.word(holder);
    let rest = m.word(rider + R_RESTS).wrapping_add(rest.wrapping_mul(REST_STRIDE)).wrapping_add(REST_QUAT);
    let q1 = quad(m, first + B_QUAT);
    let qa = turn(q1, quad(m, rest));
    let qb = product(qa, q1);
    let q2 = quad(m, second + B_QUAT);
    let mut qr = rel(qb, q2);
    if F(qr.w()) < F::ZERO {
        qr = negate(qr);
    }
    let (angle, axis) = axis_angle(normalize(qr), tables);
    let mut a = unturn(q2, scaled(axis, angle.0));
    let max = f(m, joint + MAX_ANGLE).0;
    for k in 0..3 {
        let (s, v) = (f(m, joint + STICKY + 4 * k as u32), F(a[k]));
        a[k] = if (!(s <= F::ZERO) && !(v <= F::ZERO)) || (s < F::ZERO && v < F::ZERO) {
            0.0
        } else {
            fpu::min(fpu::max(a[k], fpu::neg(max)), max)
        };
    }
    let damped = times(quad(m, joint + CRITICAL), wc);
    let t = minus(times(quad(m, joint + SPRING), a), damped);
    if m.word(rider + R_MODE) == WRECK_MODE {
        let (length, left) = (f(m, rider + R_FADE_LENGTH).0, f(m, rider + R_FADE_LEFT).0);
        let fade = if length == 0.0 {
            1.0
        } else {
            let ratio = fpu::div(fpu::sub(left, 0.0), fpu::sub(length, 0.0));
            let f = fpu::madd(fpu::adda(0.0, 1.0), ratio, fpu::sub(0.0, 1.0));
            fpu::min(fpu::max(f, fpu::min(1.0, 0.0)), fpu::max(1.0, 0.0))
        };
        let t = scaled(t, fpu::mul(0.5, fade));
        add_moment(m, second, t);
        let q2 = quad(m, second + B_QUAT);
        let back = scaled(turn(q2, t), -1.0);
        add_moment_world(m, first, back);
    } else {
        add_moment(m, second, t);
    }
    copy(m, ORIGIN, joint + STICKY);
}

/// **The world's step** for the world at `world`, `dt` the frame's: [`crate::biplane::World::step`]
/// at the world's iterations and rate over its lists in RAM. The bodies are MX vs. ATV's rigid bodies
/// ([`crate::atv_body::Body::integrate`], [`crate::atv_body::Body::end_substep`]); the forces and
/// constraints are what `rig` says they are.
pub fn step<R: Ram>(m: &mut R, world: u32, dt: F, tables: &Tables, rig: &mut impl Rig<R>) {
    let w = biplane::World { iterations: m.word(world + W_ITERATIONS) as i32, rate: f(m, world + W_RATE) };
    w.step(dt, &mut Stepper { m: RefCell::new(m), world, tables, rig, h: F::ZERO });
}

struct Stepper<'a, R: Ram, G: Rig<R>> {
    m: RefCell<&'a mut R>,
    world: u32,
    tables: &'a Tables,
    rig: &'a mut G,
    /// The substep, as the constraints are handed it.
    h: F,
}

impl<R: Ram, G: Rig<R>> Stepper<'_, R, G> {
    fn list(list: List) -> u32 {
        match list {
            List::Bodies => W_BODIES,
            List::Force => W_FORCES,
            List::Constraints => W_CONSTRAINTS,
            List::Preforce => W_PREFORCE,
            List::Post => W_POST,
        }
    }

    fn at(&self, list: List, i: i32) -> u32 {
        entry(&mut **self.m.borrow_mut(), self.world + Self::list(list), i as u32)
    }

    /// The body at `at`, all of its words, as MX vs. ATV's rigid body; and back, only what changed.
    fn with_body(&mut self, at: u32, f: impl FnOnce(&mut crate::atv_body::Body)) {
        let m = self.m.get_mut();
        let words: Vec<u32> = (0..BODY_WORDS).map(|k| m.word(at + 4 * k)).collect();
        let mut b = seed::body(&|a: u32| words[((a - at) / 4) as usize], at);
        f(&mut b);
        for (a, w) in seed::body_words(&b, at) {
            if words[((a - at) / 4) as usize] != w {
                m.set(a, w);
            }
        }
    }
}

impl<R: Ram, G: Rig<R>> Dynamics for Stepper<'_, R, G> {
    fn count(&self, list: List) -> i32 {
        count(&mut **self.m.borrow_mut(), self.world + Self::list(list))
    }
    fn preforce(&mut self, i: i32, dt: F) {
        let e = self.at(List::Preforce, i);
        self.rig.preforce(self.m.get_mut(), e, dt);
    }
    fn force(&mut self, i: i32, h: F) {
        let e = self.at(List::Force, i);
        let m = &mut **self.m.get_mut();
        match self.rig.force(m, e) {
            Force::Wrapper => {
                let l = m.word(e + F_LISTENER);
                if l != 0 {
                    match self.rig.listener(m, l) {
                        Listener::Rider(rider) => rider_force(m, rider, self.tables),
                        Listener::Other => self.rig.other_listener(m, l, h),
                    }
                }
            }
            Force::Directed => directed_force(m, e),
            Force::Other => self.rig.other_force(m, e, h),
        }
    }
    fn enabled(&self, body: i32) -> bool {
        let at = self.at(List::Bodies, body);
        self.m.borrow_mut().word(at) != 0
    }
    fn integrate(&mut self, body: i32, h: F) {
        let at = self.at(List::Bodies, body);
        self.h = h;
        self.with_body(at, |b| b.integrate(h));
    }
    fn end_substep(&mut self, body: i32) {
        let at = self.at(List::Bodies, body);
        self.with_body(at, |b| b.end_substep());
    }
    fn constrain(&mut self, i: i32, first: bool) {
        let c = self.at(List::Constraints, i);
        let m = &mut **self.m.get_mut();
        match self.rig.constraint(m, c) {
            Constraint::Pin => pin(m, c, self.tables),
            Constraint::Bone => self.rig.bone(m, c, self.h),
            Constraint::Box => frame_box(m, c),
            Constraint::Other => self.rig.other_constraint(m, c, self.h, first),
        }
    }
    fn post(&mut self, i: i32, dt: F) {
        let e = self.at(List::Post, i);
        self.rig.post(self.m.get_mut(), e, dt);
    }
}
