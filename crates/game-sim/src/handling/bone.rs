//! **SLUS-21104's rider ragdoll world's bone collision, on RAM** (S278a): the constraint that keeps
//! each of the rider's ragdoll parts out of the ground and the scene, and the source of the part's
//! touched word ([`super::rider::TOUCHED`]) the rider's flag is made from. It is MX Unleashed's bone
//! collision ([`crate::ragdoll_contact`]) in MX vs. ATV's layout, with three changes: the ground is
//! MX vs. ATV's terrain query ([`crate::atv_ground`]), the scene's contacts come out of the shape's
//! own hit record, and the sweep's back step is `dt` times a factor rather than a constant. Where the
//! original keeps them: research/oracle.txt `# S278a`.
//!
//! A part's words: on (`+0x4`); its four flags, touched (`+0x14`), ground (`+0x18`), scene (`+0x1c`)
//! and swept (`+0x20`); the sweep on (`+0x24`) and its back factor (`+0x28`); the unpenetration
//! (`+0x2c`), rotation scale (`+0x30`), restitution (`+0x34`) and friction (`+0x38`); the ground's
//! surface byte (`+0x3c`); the contact point (`+0x40`) and normal (`+0x50`); the velocity change
//! (`+0x60`); the collidable the contact is with (`+0x70`, `+0x74`); the terrain (`+0x78`), the body
//! it pushes (`+0x7c`) and the body the impulse turns (`+0x80`, the same one in the rider's parts);
//! the scene's pool (`+0x84`), its sphere (`+0x88`) and its sweep probe (`+0x8c`); and a scratch
//! quadword (`+0x90`) whose `x` the impulse puts the body's inverse mass into and whose `w` starts
//! each of its sums.
//!
//! - [`solve`]: once a world step per part — the ground, then the scene, then (with no scene contact)
//!   the sweep.
//! - [`push_out`]: one contact — touched set, the body moved out along the normal, its velocity's
//!   bounce and friction against the other side's ([`moving`]), straight onto the body or as an
//!   impulse at the arm ([`impulse`]).
//!
//! What is not here: the scene's shape calls (moving a sphere, setting a probe's segment, the search
//! that fills the shape's hit record) and a mover's two virtuals, which go through [`Scene`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::ground;
use super::node::{self, Ram};
use crate::biplane::unturn;
use crate::camera::CurveTable;
use crate::chassis::turn;
use crate::track;
use core::cell::RefCell;
use ps2_float::fpu::{self, Acc};
use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf, vu};

/// A part's words (the module's doc lists them).
pub const ON: u32 = 0x04;
pub const TOUCHED: u32 = 0x14;
pub const GROUND: u32 = 0x18;
pub const SCENE: u32 = 0x1c;
pub const SWEPT: u32 = 0x20;
pub const SWEEP: u32 = 0x24;
pub const SWEEP_BACK: u32 = 0x28;
pub const UNPENETRATION: u32 = 0x2c;
pub const ROTATION: u32 = 0x30;
pub const RESTITUTION: u32 = 0x34;
pub const FRICTION: u32 = 0x38;
pub const SURFACE: u32 = 0x3c;
pub const POINT: u32 = 0x40;
pub const NORMAL: u32 = 0x50;
pub const DV: u32 = 0x60;
pub const OTHER: u32 = 0x70;
pub const OTHER_HIT: u32 = 0x74;
pub const TERRAIN: u32 = 0x78;
pub const BODY: u32 = 0x7c;
pub const TURNED: u32 = 0x80;
pub const POOL: u32 = 0x84;
pub const SPHERE: u32 = 0x88;
pub const PROBE: u32 = 0x8c;
pub const SCRATCH: u32 = 0x90;

/// A body's words: enabled (`+0x0`), inverse mass, radius, position, velocity, inverse inertia (in
/// its frame), quaternion, rates (in its frame).
pub const B_ENABLED: u32 = 0x00;
pub const B_INV_MASS: u32 = 0x1c;
pub const B_RADIUS: u32 = 0x20;
pub const B_POSITION: u32 = 0x70;
pub const B_VELOCITY: u32 = 0x80;
pub const B_INV_INERTIA: u32 = 0x140;
pub const B_QUAT: u32 = 0x190;
pub const B_RATES: u32 = 0x1d0;

/// A shape's (the sphere's, the probe's) hit flag (bit 5 of the byte at `+0x126`) and hit record
/// (`+0x7c`); the record's contact count, normals, points and collidables (three of each at most,
/// in quadword and word steps), and the sweep's point and normal (each a pointer to a quadword).
pub const S_HIT_BYTE: u32 = 0x126;
pub const S_HIT_BIT: u32 = 0x20;
pub const S_RECORD: u32 = 0x7c;
pub const R_COUNT: u32 = 0x04;
pub const R_NORMALS: u32 = 0x30;
pub const R_POINTS: u32 = 0x60;
pub const R_OTHERS: u32 = 0x90;
pub const R_SWEPT_NORMAL: u32 = 0x94;
pub const R_SWEPT_POINT: u32 = 0x98;

/// A collidable's mover (taken only when the mover's class, `+0x1a0`, is 2) and its collision object.
pub const C_MOVER: u32 = 0x8c;
pub const C_OBJECT: u32 = 0x90;
pub const MOVER_CLASS: u32 = 0x1a0;
pub const MOVER_IS: u32 = 2;
/// A collision object's centre of mass in its frame and in the world's directions, its vehicle
/// chassis, its dynamics body and its transform node.
pub const O_COFG: u32 = 0x80;
pub const O_COFG_WORLD: u32 = 0x90;
pub const O_CHASSIS: u32 = 0xe0;
pub const O_BODY: u32 = 0xe4;
pub const O_NODE: u32 = 0xe8;
/// A chassis's centre of mass (in its node's frame), velocity and rates (in its node's frame).
pub const CH_COFG: u32 = 0x180;
pub const CH_VELOCITY: u32 = 0x250;
pub const CH_RATES: u32 = 0x280;
/// A mover's origin (in its node's frame) and transform node.
pub const MOVER_ORIGIN: u32 = 0x180;
pub const MOVER_NODE: u32 = 0x244;

/// The rest quadword `(0, 0, 0, 1)`: rodata.
pub const ORIGIN: u32 = 0x0039_6fc0;

/// Under this speed into the surface the bounce takes the restitution.
const BOUNCE_BELOW: F = F(5.0);

/// **The scene's calls the bone collision makes**: on the original's shapes in RAM, each leaving
/// what it found where the solve reads it (the shape's hit flag and hit record).
pub trait Scene<R: Ram> {
    /// The sphere moved to the point at `at` and brought up to date (two calls: the move, then the
    /// shape's own virtual).
    fn place(&mut self, m: &mut R, sphere: u32, at: u32);
    /// The probe's segment set from `from` to `to`, and brought up to date (two calls).
    fn segment(&mut self, m: &mut R, probe: u32, from: Vf, to: Vf);
    /// The shape tested against the collidables of `pool`.
    fn search(&mut self, m: &mut R, shape: u32, pool: u32);
    /// The shape tested against the whole scene (the part has no pool).
    fn search_all(&mut self, m: &mut R, shape: u32);
    /// A mover's spin and velocity: its two virtuals, in the order the push-out calls them.
    fn spin(&mut self, m: &mut R, mover: u32) -> Vf;
    fn velocity(&mut self, m: &mut R, mover: u32) -> Vf;
}

/// A quadword load or store, as `lq`/`sq` and `lqc2`/`sqc2` make it: the address's low four bits
/// dropped.
fn quad(m: &mut impl Ram, at: u32) -> Vf {
    Vf::from_bits([0, 4, 8, 12].map(|k| m.word((at & !0xf) + k)))
}

fn put_quad(m: &mut impl Ram, at: u32, v: Vf) {
    for (k, w) in v.to_bits().into_iter().enumerate() {
        m.set((at & !0xf) + 4 * k as u32, w);
    }
}

/// A float word as `lwc1` loads it.
fn f(m: &mut impl Ram, at: u32) -> F {
    F(f32::from_bits(m.word(at)))
}

fn byte(m: &mut impl Ram, at: u32) -> u32 {
    m.word(at & !3) >> (8 * (at & 3)) & 0xff
}

fn put_byte(m: &mut impl Ram, at: u32, b: u8) {
    let s = 8 * (at & 3);
    let w = m.word(at & !3);
    m.set(at & !3, w & !(0xff << s) | u32::from(b) << s);
}

/// `a.xyz + b.xyz`, `w` from `a`.
fn plus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.add(b))
}

/// `a.xyz - b.xyz`, `w` from `a`.
fn minus(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, a.sub(b))
}

/// `a.xyz` times `s`, `w` from `a` (`vmulx.xyz` of a moved-in word).
fn scaled(a: Vf, s: F) -> Vf {
    a.merge(Dest::XYZ, a.mul(Vf::splat(s.0)))
}

/// `a x b` as the impulse takes it: `vopmula a, b` then `vopmsub b, a`.
fn cross(a: Vf, b: Vf) -> Vf {
    Vf::opmsub(Vf::opmula(a, b), b, a)
}

/// [`cross`], `.xyz`, `w` from `a`.
fn cross_xyz(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, cross(a, b))
}

/// `a.xyz . b.xyz` the way the FPU takes it off three words each: one `mula`, one `madda`, one
/// `madd`.
fn dot(a: Vf, b: Vf) -> F {
    Acc::mula(F(a.x()), F(b.x())).madda(F(a.y()), F(b.y())).madd(F(a.z()), F(b.z()))
}

/// The `w` lane's sum of `s`'s three lanes, started off `base`: `base + x`, then `+ 1 * y`, `+ 1 * z`.
fn sum3(base: f32, s: Vf) -> f32 {
    vu::madd(vu::madd(vu::add(base, s.x()), 1.0, s.y()), 1.0, s.z())
}

/// **The bone collision's solve** for the part at `part`, its step `dt`: nothing unless the part is
/// on and has a body that is enabled. The other and swept words cleared and the point the body's
/// position; then
///
/// 1. the ground, with a terrain and the point strictly inside its box in `x` and `z`: the terrain's
///    query moves the point onto it, writes the normal and the surface byte; with the body's centre
///    lower than the point by less than its radius, ground set and a [`push_out`];
/// 2. the scene: the sphere placed at the body and searched (in the pool, with one); with a hit, scene
///    set and a [`push_out`] for each of the hit record's contacts, the point, normal and collidable
///    its (scene set again each time; the count read each time);
/// 3. else the sweep ([`sweep`]).
///
/// `map` is the terrain object's cell reader, the map's tree.
pub fn solve<R: Ram, S: Scene<R>>(m: &mut R, s: &mut S, part: u32, dt: F, map: Option<&track::Terrain<'_>>, curve: &CurveTable) {
    if m.word(part + ON) == 0 {
        return;
    }
    let body = m.word(part + BODY);
    if body == 0 || m.word(body + B_ENABLED) == 0 {
        return;
    }
    m.set(part + OTHER, 0);
    m.set(part + SWEPT, 0);
    let body = m.word(part + BODY);
    let p = quad(m, body + B_POSITION);
    put_quad(m, part + POINT, p);
    let t = m.word(part + TERRAIN);
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
    let inside = t != 0 && {
        let x = f(m, part + POINT);
        x < f(m, t + 0x40) && !(x <= f(m, t + 0x30)) && {
            let z = f(m, part + POINT + 8);
            z < f(m, t + 0x48) && !(z <= f(m, t + 0x38))
        }
    };
    if inside {
        let terrain = {
            let r = RefCell::new(&mut *m);
            ground::read_terrain(&|a: u32| r.borrow_mut().word(a), t, map)
        };
        let mut record = byte(m, part + SURFACE) as u8;
        let (at, normal) = (quad(m, part + POINT), quad(m, part + NORMAL));
        let (at, normal) = terrain.ground(at, normal, &mut record);
        put_quad(m, part + POINT, at);
        put_quad(m, part + NORMAL, normal);
        put_byte(m, part + SURFACE, record);
        let body = m.word(part + BODY);
        if f(m, body + B_POSITION + 4) < f(m, part + POINT + 4) + f(m, body + B_RADIUS) {
            m.set(part + GROUND, 1);
            push_out(m, s, part, part + POINT, part + NORMAL, curve);
        }
    }
    let body = m.word(part + BODY);
    let sphere = m.word(part + SPHERE);
    s.place(m, sphere, body + B_POSITION);
    search(m, s, part, SPHERE);
    let sphere = m.word(part + SPHERE);
    if byte(m, sphere + S_HIT_BYTE) & S_HIT_BIT == 0 {
        sweep(m, s, part, dt, curve);
        return;
    }
    m.set(part + SCENE, 1);
    let sphere = m.word(part + SPHERE);
    let hits = m.word(sphere + S_RECORD);
    let mut i = 0;
    while i < m.word(hits + R_COUNT) as i32 {
        let k = i as u32;
        m.set(part + SCENE, 1);
        let p = quad(m, hits + 0x10 * k + R_POINTS);
        put_quad(m, part + POINT, p);
        let n = quad(m, hits + 0x10 * k + R_NORMALS);
        put_quad(m, part + NORMAL, n);
        let other = m.word(hits + 4 * k + R_OTHERS);
        m.set(part + OTHER_HIT, other);
        let other = m.word(part + OTHER_HIT);
        m.set(part + OTHER, other);
        push_out(m, s, part, part + POINT, part + NORMAL, curve);
        i += 1;
    }
}

/// The part's shape at `shape` (its sphere's or its probe's offset) searched: in the part's pool,
/// with one, else in the whole scene.
fn search<R: Ram, S: Scene<R>>(m: &mut R, s: &mut S, part: u32, shape: u32) {
    match m.word(part + POOL) {
        0 => {
            let shape = m.word(part + shape);
            s.search_all(m, shape)
        }
        pool => {
            let shape = m.word(part + shape);
            s.search(m, shape, pool)
        }
    }
}

/// **The sweep**, with the sweep on and a probe: the probe's segment from where the body was
/// `dt · back` ago along its velocity to where it is, searched; with a hit, scene and swept set and a
/// [`push_out`] at the record's swept point and normal (against no collidable: the solve cleared it,
/// and swept sends the push-out straight onto the body).
pub fn sweep<R: Ram, S: Scene<R>>(m: &mut R, s: &mut S, part: u32, dt: F, curve: &CurveTable) {
    if m.word(part + SWEEP) == 0 || m.word(part + PROBE) == 0 {
        return;
    }
    let k = dt * f(m, part + SWEEP_BACK);
    let body = m.word(part + BODY);
    let back = scaled(quad(m, body + B_VELOCITY), k);
    let body = m.word(part + BODY);
    let from = minus(quad(m, body + B_POSITION), back);
    let body = m.word(part + BODY);
    let to = quad(m, body + B_POSITION);
    let probe = m.word(part + PROBE);
    s.segment(m, probe, from, to);
    search(m, s, part, PROBE);
    let probe = m.word(part + PROBE);
    if byte(m, probe + S_HIT_BYTE) & S_HIT_BIT == 0 {
        return;
    }
    m.set(part + SCENE, 1);
    m.set(part + SWEPT, 1);
    let probe = m.word(part + PROBE);
    let hits = m.word(probe + S_RECORD);
    let at = m.word(hits + R_SWEPT_POINT);
    let p = quad(m, at);
    put_quad(m, part + POINT, p);
    let at = m.word(hits + R_SWEPT_NORMAL);
    let n = quad(m, at);
    put_quad(m, part + NORMAL, n);
    push_out(m, s, part, part + POINT, part + NORMAL, curve);
}

/// **The velocity of the other side of the contact at `point`** (the part's collidable at `+0x70`),
/// which the push-out measures the body's against: `(0, 0, 0, 1)` with none, else its own velocity
/// plus its spin crossed with the arm from its centre of mass to the point, the arm taken into the
/// frame its spin is kept in and the spin's part turned back out. A collision object wins over a
/// mover:
///
/// - with a dynamics body: through the body's quaternion, the object's centre of mass in the world's
///   directions written on the way;
/// - else as a vehicle's chassis, through the object's node (the same write);
/// - a mover (its class 2) with no object: through its node, its spin and velocity its virtuals';
/// - neither: `(0, 0, 0, 1)`.
pub fn moving<R: Ram, S: Scene<R>>(m: &mut R, s: &mut S, part: u32, point: u32) -> Vf {
    let other = m.word(part + OTHER);
    if other == 0 {
        return quad(m, ORIGIN);
    }
    let mut mover = m.word(other + C_MOVER);
    let o = m.word(other + C_OBJECT);
    if mover != 0 && m.word(mover + MOVER_CLASS) != MOVER_IS {
        mover = 0;
    }
    if o != 0 {
        let b = m.word(o + O_BODY);
        if b != 0 {
            let (q, cofg) = (quad(m, b + B_QUAT), quad(m, o + O_COFG));
            put_quad(m, o + O_COFG_WORLD, turn(q, cofg));
            let centre = plus(quad(m, b + B_POSITION), quad(m, o + O_COFG_WORLD));
            let arm = minus(quad(m, point), centre);
            let arm = unturn(quad(m, b + B_QUAT), arm);
            let spun = cross_xyz(quad(m, b + B_RATES), arm);
            let spun = turn(quad(m, b + B_QUAT), spun);
            plus(spun, quad(m, b + B_VELOCITY))
        } else {
            let (n, ch) = (m.word(o + O_NODE), m.word(o + O_CHASSIS));
            let cofg = quad(m, o + O_COFG).to_bits();
            let w = node::rotate(m, n, cofg);
            put_quad(m, o + O_COFG_WORLD, Vf::from_bits(w));
            let at = quad(m, point).to_bits();
            let local = Vf::from_bits(node::unplace(m, n, at));
            let centre = plus(quad(m, ch + CH_COFG), quad(m, o + O_COFG_WORLD));
            let arm = minus(local, centre);
            let spun = cross_xyz(quad(m, ch + CH_RATES), arm);
            let spun = Vf::from_bits(node::rotate(m, n, spun.to_bits()));
            plus(spun, quad(m, ch + CH_VELOCITY))
        }
    } else if mover != 0 {
        let n = m.word(mover + MOVER_NODE);
        let spin = s.spin(m, mover);
        let velocity = s.velocity(m, mover);
        let at = quad(m, point).to_bits();
        let local = Vf::from_bits(node::unplace(m, n, at));
        let arm = minus(local, quad(m, mover + MOVER_ORIGIN));
        let spun = Vf::from_bits(node::rotate(m, n, cross_xyz(spin, arm).to_bits()));
        plus(spun, velocity)
    } else {
        quad(m, ORIGIN)
    }
}

/// **The push-out** of the part's body from the contact at `point` along `normal` (both quadwords in
/// RAM): touched set; the correction the normal times `unpenetration · (radius - depth)`; the velocity
/// against the other side's ([`moving`]) split along the normal and across it; the normal part
/// bounced (times `-restitution`, under 5 into the surface), the rest times the friction, both times
/// the unpenetration, less the relative velocity: the velocity change, kept at `+0x60`. Then
///
/// - with no body to turn, or swept: the correction and the change added to the body's position and
///   velocity;
/// - else as one [`impulse`] each (handed in negated), at the arm from the turned body's centre to
///   the point, a radius long (times the rotation scale), in its frame.
pub fn push_out<R: Ram, S: Scene<R>>(m: &mut R, s: &mut S, part: u32, point: u32, normal: u32, curve: &CurveTable) {
    m.set(part + TOUCHED, 1);
    let body = m.word(part + BODY);
    let d = minus(quad(m, body + B_POSITION), quad(m, point));
    let n = quad(m, normal);
    let depth = dot(d, n);
    let body = m.word(part + BODY);
    let c = f(m, part + UNPENETRATION) * (f(m, body + B_RADIUS) - depth);
    let corr = scaled(quad(m, normal), c);

    let against = moving(m, s, part, point);
    let body = m.word(part + BODY);
    let rel = minus(quad(m, body + B_VELOCITY), against);
    let n = quad(m, normal);
    let into = dot(rel, n);
    let vn = scaled(quad(m, normal), into);
    let vt = minus(rel, vn);
    let bounce = if into < BOUNCE_BELOW { scaled(vn, F(fpu::neg(f(m, part + RESTITUTION).0))) } else { vn };
    let slide = scaled(vt, f(m, part + FRICTION));
    let sum = plus(bounce, slide);
    let kept = scaled(sum, f(m, part + UNPENETRATION));
    put_quad(m, part + DV, minus(kept, rel));

    let turned = m.word(part + TURNED);
    if turned == 0 || m.word(part + SWEPT) != 0 {
        let body = m.word(part + BODY);
        let p = quad(m, body + B_POSITION);
        put_quad(m, body + B_POSITION, plus(p, corr));
        let body = m.word(part + BODY);
        let (v, dv) = (quad(m, body + B_VELOCITY), quad(m, part + DV));
        put_quad(m, body + B_VELOCITY, plus(v, dv));
        return;
    }
    let a = minus(quad(m, point), quad(m, turned + B_POSITION));
    let sq = a.mul(a);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    let a = a.merge(Dest::XYZ, a.mul_q(vu::rsqrt(1.0, len2)));
    let body = m.word(part + BODY);
    let a = scaled(a, f(m, part + ROTATION) * f(m, body + B_RADIUS));
    let turned = m.word(part + TURNED);
    let arm = unturn(quad(m, turned + B_QUAT), a);
    let flip = |v: Vf| Vf::new(fpu::neg(v.x()), fpu::neg(v.y()), fpu::neg(v.z()), v.w());
    let kick = flip(quad(m, part + DV));
    impulse(m, part, arm, flip(corr), kick, curve);
}

/// **The impulse at the arm** that moves the part's turned body by `-shift` and changes its velocity
/// by `-kick` (the push-out hands both in negated), MX Unleashed's ([`crate::ragdoll_contact`]'s)
/// with the body's words in MX vs. ATV's places: the inverse mass is first copied into the scratch
/// quadword's `x`, whose `w` starts each sum; the shift's turn goes onto the quaternion through the
/// curve table ([`sin_cos`]), the kick's into the rates.
pub fn impulse(m: &mut impl Ram, part: u32, arm: Vf, shift: Vf, kick: Vf, curve: &CurveTable) {
    let b = m.word(part + TURNED);
    let inv_mass = m.word(b + B_INV_MASS);
    m.set(part + SCRATCH, inv_mass);
    let b = m.word(part + TURNED);
    let (pos, vel, q, rates, ii) = (quad(m, b + B_POSITION), quad(m, b + B_VELOCITY), quad(m, b + B_QUAT), quad(m, b + B_RATES), quad(m, b + B_INV_INERTIA));
    let scratch = quad(m, part + SCRATCH);
    let (mass, base) = (scratch.x(), scratch.w());
    let minus_one = Vf::VF0.sub(Vf::VF0.bc(Field::W));
    // One impulse: its size along `v`'s direction, as `j`.
    let along = |v: Vf| -> Vf {
        let n = v.mul_q(vu::rsqrt(1.0, sum3(base, v.mul(v))));
        let nl = unturn(q, n);
        let c = cross(arm, nl).mul(ii);
        let e = cross(c, arm);
        let k = vu::add(sum3(base, nl.mul(e)), mass);
        let v = v.merge(Dest::XYZ, v.mul(minus_one));
        v.mul_q(vu::div(1.0, k))
    };
    let mv = Vf::splat(mass);

    let j = along(shift);
    let pos = pos.merge(Dest::XYZ, pos.add(j.mul(mv)));
    let turned = turn(q, cross(arm, unturn(q, j)).mul(ii));
    let ang = vu::add(base, vu::sqrt(sum3(base, turned.mul(turned))));
    let axis = turned.mul_q(vu::div(1.0, ang));

    let j = along(kick);
    let vel = vel.merge(Dest::XYZ, vel.add(j.mul(mv)));
    let r = cross(arm, unturn(q, j)).mul(ii);
    let rates = rates.merge(Dest::XYZ, rates.add(r));
    put_quad(m, b + B_POSITION, pos);
    put_quad(m, b + B_VELOCITY, vel);
    put_quad(m, b + B_RATES, rates);

    // The shift's turn: `ang` about `axis`, as the quaternion `(axis sin(ang/2), cos(ang/2))`
    // multiplied on the left.
    let (sin, cos) = sin_cos(F(0.5) * F(ang), curve);
    let r = axis.mul(Vf::splat(sin.0)).merge(Dest::W, Vf::VF0.mul(Vf::splat(cos.0)));
    let p = r.mul(q);
    let acc = Vf::opmula(r, q);
    let acc = Vf::madd(acc, r, q.bc(Field::W));
    let acc = Vf::madd(acc, q, r.bc(Field::W));
    let xyz = Vf::opmsub(acc, q, r);
    let w = vu::msub(vu::msub(vu::sub(p.w(), p.x()), 1.0, p.y()), 1.0, p.z());
    let b = m.word(part + TURNED);
    put_quad(m, b + B_QUAT, xyz.merge(Dest::W, Vf::splat(w)));
}

const PI: F = F(core::f32::consts::PI);
const TWO_PI: F = F(core::f32::consts::TAU);
const ONE_OVER_TWO_PI: F = F(0.15915494);
const CURVE_STEP: F = F(0.0010473721);
const CURVE_INV_STEP: F = F(954.77045);

/// One wrap into `-pi..=pi`, the impulse's way: the whole turns off (`mul`, `sub`), then one
/// correction either way.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn wrap(a: F) -> F {
    let turns = F(fpu::from_i32((ONE_OVER_TWO_PI * a).to_i32()));
    let w = a - TWO_PI * turns;
    if !(w <= PI) {
        w - TWO_PI
    } else if w < -PI {
        TWO_PI + w
    } else {
        w
    }
}

/// **An angle's sine and cosine off the curve table**, as the impulse takes them: wrapped twice
/// ([`wrap`]), then the entry and the interpolation of [`node::sin_cos`].
pub fn sin_cos(angle: F, curve: &CurveTable) -> (F, F) {
    let s = (PI + wrap(wrap(angle))) * CURVE_INV_STEP;
    let i = s.to_i32();
    let d = CURVE_STEP * (s - F(fpu::from_i32(i)));
    let [cos, sin] = curve.entries()[i as usize];
    (Acc::adda(F::ZERO, F(sin)).madd(d, F(cos)), Acc::adda(F::ZERO, F(cos)).msub(d, F(sin)))
}
