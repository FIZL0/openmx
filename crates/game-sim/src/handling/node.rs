//! **SLUS-21104's scene-node calls on RAM** (F43): the node functions the bike's vehicle update
//! ([`super::update`]) makes on the rider's skeleton every frame, as word-level ports over a node
//! graph in memory. [`crate::atv_moto::Scene`] holds the same graph as a struct for the quad; the
//! rider's nodes have parents, children and siblings the bike's frame does not own, so these read and
//! write them where they are. Where the original keeps them: research/oracle.txt `# F43`.
//!
//! A node's words: its own frame (`+0x30`, four rows, the position row `+0x60`), its world frame
//! (`+0x70`, the position row `+0xa0`), its parent (`+0xf0`), first child (`+0xf4`), next sibling
//! (`+0xf8`) and flag byte (`+0x100`: bit 0 its world frame current, bit 7 its bounds stale).
//!
//! - [`update`]: the world frame brought up to date, down from the first current parent.
//! - [`moved`]: a current node's frame changed — its bounds and its parents' marked stale, it and
//!   everything current under it marked out of date.
//! - [`place`], [`rotate`], [`local`], [`position`], [`axes`], [`frame`]: a point or a direction
//!   carried out of the node's world frame or into it, its position, two of its rows, the whole frame.
//! - [`set`], [`spin`], [`aim`], [`set_frame`], [`orient`]: the node's own frame changed, then
//!   [`moved`].
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::update::Quad;
use crate::atv_moto::{frame_inverse, frame_mul, frame_point, point_in_frame, quat_frame};
use crate::camera::CurveTable;
use ps2_float::vu::{Dest, Field};
use ps2_float::{F, Vf, fpu, fpu::Acc, vu};

/// The memory the nodes are in.
pub trait Ram {
    fn word(&mut self, at: u32) -> u32;
    fn set(&mut self, at: u32, w: u32);
}

const LOCAL: u32 = 0x30;
const POSITION: u32 = 0x60;
const WORLD: u32 = 0x70;
const PARENT: u32 = 0xf0;
const CHILD: u32 = 0xf4;
const SIBLING: u32 = 0xf8;
const FLAGS: u32 = 0x100;
const CURRENT: u32 = 0x01;
const STALE_BOUNDS: u32 = 0x80;

fn quad(m: &mut impl Ram, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at.wrapping_add(k)))
}

fn put_quad(m: &mut impl Ram, at: u32, q: Quad) {
    for (k, w) in q.into_iter().enumerate() {
        m.set(at.wrapping_add(4 * k as u32), w);
    }
}

fn v(m: &mut impl Ram, at: u32) -> Vf {
    Vf::from_bits(quad(m, at))
}

fn put_v(m: &mut impl Ram, at: u32, x: Vf) {
    put_quad(m, at, x.to_bits());
}

fn rows(m: &mut impl Ram, at: u32) -> [Vf; 4] {
    [0, 1, 2, 3].map(|r| v(m, at + 0x10 * r))
}

fn put_rows(m: &mut impl Ram, at: u32, f: &[Vf; 4]) {
    for (r, x) in f.iter().enumerate() {
        put_v(m, at + 0x10 * r as u32, *x);
    }
}

/// The `w` word the one handed in: the register holding it is only moved, so a denormal stays one.
fn keep_w(mut r: Quad, q: Quad) -> Quad {
    r[3] = q[3];
    r
}

/// A frame's sixteen words copied as they are.
fn copy(m: &mut impl Ram, from: u32, to: u32) {
    for k in (0..0x40).step_by(4) {
        let w = m.word(from + k);
        m.set(to + k, w);
    }
}

/// The flag byte is the word's low byte (the words are little-endian); a byte store leaves the rest.
fn flags(m: &mut impl Ram, n: u32) -> u32 {
    m.word(n + FLAGS) & 0xff
}

fn put_flags(m: &mut impl Ram, n: u32, f: u32) {
    let w = m.word(n + FLAGS);
    m.set(n + FLAGS, (w & !0xff) | (f & 0xff));
}

/// **The world frame brought up to date**: unless the node is current, the parents up to the first
/// current one (or the root) are gathered, the top one's world frame is its own frame (no parent)
/// or its own times its parent's world frame, and so on down to the node, each marked current.
pub fn update(m: &mut impl Ram, n: u32) {
    if flags(m, n) & CURRENT != 0 {
        return;
    }
    let mut chain = vec![n];
    let mut up = m.word(n + PARENT);
    while up != 0 && flags(m, up) & CURRENT == 0 {
        chain.push(up);
        up = m.word(up + PARENT);
    }
    for &i in chain.iter().rev() {
        match m.word(i + PARENT) {
            // A root's copy moves words (`lq`/`sq`): a denormal stays one.
            0 => copy(m, i + LOCAL, i + WORLD),
            p => {
                let world = frame_mul(&rows(m, i + LOCAL), &rows(m, p + WORLD));
                put_rows(m, i + WORLD, &world);
            }
        }
        let f = flags(m, i);
        put_flags(m, i, f | CURRENT);
    }
}

/// **A current node's frame changed**: its bounds and every parent's marked stale (bit 7), it marked
/// out of date, and its children ([`out_of_date`]). A node already out of date is left as it is.
pub fn moved(m: &mut impl Ram, n: u32) {
    if flags(m, n) & CURRENT == 0 {
        return;
    }
    let mut up = n;
    while up != 0 {
        let f = flags(m, up);
        put_flags(m, up, f | STALE_BOUNDS);
        up = m.word(up + PARENT);
    }
    let f = flags(m, n);
    put_flags(m, n, f & !CURRENT);
    let c = m.word(n + CHILD);
    if c != 0 {
        out_of_date(m, c);
    }
}

/// A sibling list marked out of date, as the original recurses: the rest of the list first, then
/// the node, and if it was current, its children.
fn out_of_date(m: &mut impl Ram, n: u32) {
    let next = m.word(n + SIBLING);
    if next != 0 {
        out_of_date(m, next);
    }
    let f = flags(m, n);
    if f & CURRENT != 0 {
        put_flags(m, n, f & !CURRENT);
        let c = m.word(n + CHILD);
        if c != 0 {
            out_of_date(m, c);
        }
    }
}

/// **A point of the node's frame in the world** (its world frame brought up to date first): the
/// position row plus the rows times the point's lanes; the `w` the point's.
pub fn place(m: &mut impl Ram, n: u32, p: Quad) -> Quad {
    update(m, n);
    keep_w(frame_point(&rows(m, n + WORLD), Vf::from_bits(p)).to_bits(), p)
}

/// **A direction of the node's frame in the world** (brought up to date first): the rows' `xyz`
/// times the direction's lanes (`vmulax`, `vmadday`, `vmaddz`); the `w` the direction's.
pub fn rotate(m: &mut impl Ram, n: u32, q: Quad) -> Quad {
    update(m, n);
    let w = rows(m, n + WORLD);
    let d = Vf::from_bits(q);
    keep_w(d.merge(Dest::XYZ, Vf::madd(Vf::madd(w[0].mul(d.bc(Field::X)), w[1], d.bc(Field::Y)), w[2], d.bc(Field::Z))).to_bits(), q)
}

/// **A world direction into the node's frame** (brought up to date first): each lane the dot of a
/// world row with it, over the 3×3 transposed (the original's `pextlw`/`pcpyld` shuffle, then
/// `vmulax`, `vmadday`, `vmaddz`); the `w` the direction's.
pub fn local(m: &mut impl Ram, n: u32, q: Quad) -> Quad {
    update(m, n);
    let w = rows(m, n + WORLD);
    let d = Vf::from_bits(q);
    let col = |k: usize| Vf::new(w[0].0[k], w[1].0[k], w[2].0[k], 0.0);
    keep_w(d.merge(Dest::XYZ, Vf::madd(Vf::madd(col(0).mul(d.bc(Field::X)), col(1), d.bc(Field::Y)), col(2), d.bc(Field::Z))).to_bits(), q)
}

/// **A world point into the node's frame** (brought up to date first, S278a): the world position row
/// off (`xyz`), then each lane the dot of a world row with it, over the 3×3 transposed; the `w` the
/// point's. [`place`]'s inverse.
pub fn unplace(m: &mut impl Ram, n: u32, p: Quad) -> Quad {
    update(m, n);
    point_in_frame(&rows(m, n + WORLD), Vf::from_bits(p)).to_bits()
}

/// **Where the node is in the world** (brought up to date first): the world position row's `xyz`,
/// `w` 1, into the 4 words at `out`.
pub fn position(m: &mut impl Ram, n: u32, out: u32) {
    update(m, n);
    for k in 0..3 {
        let x = m.word(n + WORLD + 0x30 + 4 * k);
        m.set(out + 4 * k, x);
    }
    m.set(out + 12, F::ONE.to_bits());
}

/// **The node's world rows `z` and `y`** (brought up to date first), each `xyz` with `w` 0, into the
/// quadwords at `z` and `y`.
pub fn axes(m: &mut impl Ram, n: u32, z: u32, y: u32) {
    update(m, n);
    for (row, out) in [(2, z), (1, y)] {
        for k in 0..3 {
            let x = m.word(n + WORLD + 0x10 * row + 4 * k);
            m.set(out + 4 * k, x);
        }
        m.set(out + 12, 0);
    }
}

/// **The node's world frame** (brought up to date first), its 16 words into `out`.
pub fn frame(m: &mut impl Ram, n: u32, out: u32) {
    update(m, n);
    copy(m, n + WORLD, out);
}

/// **The node's own frame set** from the 16 words at `from`, then [`moved`].
pub fn set_frame(m: &mut impl Ram, n: u32, from: u32) {
    copy(m, from, n + LOCAL);
    moved(m, n);
}

/// **The node's position set in the world**: the point taken into the parent's frame (its world
/// frame brought up to date first; the position row off, then the transposed 3×3) if there is a
/// parent, its `xyz` the own position row's (its `w` kept), then [`moved`].
pub fn set(m: &mut impl Ram, n: u32, p: Quad) {
    let p = match m.word(n + PARENT) {
        0 => p,
        parent => {
            update(m, parent);
            point_in_frame(&rows(m, parent + WORLD), Vf::from_bits(p)).to_bits()
        }
    };
    set_position(m, n, p);
}

/// The own position row's `xyz` the point's, then [`moved`].
pub(crate) fn set_position(m: &mut impl Ram, n: u32, p: Quad) {
    for (k, &x) in p.iter().take(3).enumerate() {
        m.set(n + POSITION + 4 * k as u32, x);
    }
    moved(m, n);
}

/// **The node aimed** along `dir` with `up` (the original's three flags all 1: the up made square to
/// the aim, both normalized): [`crate::math::basis`] at the own position (`w` 1) as the node's own
/// frame, its parent not taken into account, then [`moved`].
pub fn aim(m: &mut impl Ram, n: u32, dir: Quad, up: Quad) {
    let pos = quad(m, n + POSITION);
    let f = crate::math::basis(Vf::from_bits(pos), Vf::from_bits(dir), Vf::from_bits(up), true, true).0;
    put_rows(m, n + LOCAL, &f);
    // The position row's `xyz` go back as words: a denormal stays one.
    for (k, &x) in pos.iter().take(3).enumerate() {
        m.set(n + POSITION + 4 * k as u32, x);
    }
    moved(m, n);
}

/// **The node turned to a quaternion at a point** (F45; the original's call with no frame of
/// reference and a position): the quaternion's frame at the position
/// ([`crate::atv_moto::quat_frame`]), taken into the parent's (its world frame brought up to date
/// first, then inverted) if there is one, as the node's own frame; then [`moved`]. A root's position
/// row is the position's `xyz` words as they are (copied, not computed: a denormal stays one) and 1.
/// [`crate::atv_moto::Scene::place`] is its twin on the quad's struct.
pub fn orient(m: &mut impl Ram, n: u32, q: Quad, pos: Quad) {
    let f = quat_frame(Vf::from_bits(q), Vf::from_bits(pos));
    match m.word(n + PARENT) {
        0 => {
            put_rows(m, n + LOCAL, &f);
            for (k, &x) in pos.iter().take(3).enumerate() {
                m.set(n + POSITION + 4 * k as u32, x);
            }
        }
        parent => {
            update(m, parent);
            let f = frame_mul(&f, &frame_inverse(&rows(m, parent + WORLD)));
            put_rows(m, n + LOCAL, &f);
        }
    }
    moved(m, n);
}

/// **The node spun about an axis** by `angle` (radians): the axis (its three words in RAM)
/// normalized (`mula`/`madda`/`madd` of `y`, `x`, `z`, then `rsqrt`), the angle's sine and cosine off
/// the curve table ([`sin_cos`]), and the own frame's three rows the rotation about the axis by it
/// ([`turn`]), the position row put back as it was (`w` 1); then [`moved`].
pub fn spin(m: &mut impl Ram, n: u32, axis: u32, angle: F, curve: &CurveTable) {
    let pos = quad(m, n + POSITION);
    let [x, y, z] = [0, 4, 8].map(|k| F::from_bits(m.word(axis + k)));
    let s = F::ONE.rsqrt(Acc::mula(y, y).madda(x, x).madd(z, z));
    let (x, y, z) = (x * s, y * s, z * s);
    let (sin, cos) = sin_cos(angle, curve);
    let f = turn([x, y, z], sin, cos);
    put_rows(m, n + LOCAL, &f);
    set_position(m, n, pos);
}

/// The rotation about a unit axis by an angle's sine and cosine, VU0's way: with `a` the axis (its
/// `w` the sign word a `mfc1` leaves of `z`, all ones for a `z` with its sign set, else 0),
/// `t = a - a·cos` and `s = a·sin`, the rows `t·a.x`, `t·a.y`, `t·a.z` with `cos` on the diagonal and
/// `±s` off it; the position row `(0, 0, 0, 1)`.
pub(crate) fn turn([x, y, z]: [F; 3], sin: F, cos: F) -> [Vf; 4] {
    let sign = if z.to_bits() & 0x8000_0000 != 0 { u32::MAX } else { 0 };
    let a = Vf::from_bits([x.to_bits(), y.to_bits(), z.to_bits(), sign]);
    let t = Vf::msub(a.add(Vf::ZERO), a, Vf::splat(cos.0));
    let s = a.mul(Vf::splat(sin.0));
    let (mut r0, mut r1, mut r2) = (t.mul(a.bc(Field::X)), t.mul(a.bc(Field::Y)), t.mul(a.bc(Field::Z)));
    r0.0[0] = vu::add(r0.x(), cos.0);
    r1.0[0] = vu::sub(r1.x(), s.z());
    r2.0[0] = vu::add(r2.x(), s.y());
    r0.0[1] = vu::add(r0.y(), s.z());
    r1.0[1] = vu::add(r1.y(), cos.0);
    r2.0[1] = vu::sub(r2.y(), s.x());
    r0.0[2] = vu::sub(r0.z(), s.y());
    r1.0[2] = vu::add(r1.z(), s.x());
    r2.0[2] = vu::add(r2.z(), cos.0);
    [r0, r1, r2, Vf::new(0.0, 0.0, 0.0, 1.0)]
}

const PI: F = F(core::f32::consts::PI);
const TWO_PI: F = F(core::f32::consts::TAU);
const ONE_OVER_TWO_PI: F = F(0.15915494);
const CURVE_STEP: F = F(0.0010473721);
const CURVE_INV_STEP: F = F(954.77045);

/// One wrap into `-pi..=pi`: the whole turns off (`adda`, `msub`), then one correction either way.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn wrap(a: F) -> F {
    let turns = F(fpu::from_i32((ONE_OVER_TWO_PI * a).to_i32()));
    let w = Acc::adda(F::ZERO, a).msub(TWO_PI, turns);
    if !(w <= PI) {
        w - TWO_PI
    } else if w < -PI {
        TWO_PI + w
    } else {
        w
    }
}

/// **An angle's sine and cosine off the curve table**: wrapped twice, the entry below
/// `(pi + a) / step` and the distance past it `d = step · frac`; `sin + d·cos` and `cos - d·sin`.
pub fn sin_cos(angle: F, curve: &CurveTable) -> (F, F) {
    let s = (PI + wrap(wrap(angle))) * CURVE_INV_STEP;
    let i = s.to_i32();
    let d = CURVE_STEP * (s - F(fpu::from_i32(i)));
    let [cos, sin] = curve.entries()[i as usize];
    (Acc::adda(F::ZERO, F(sin)).madd(d, F(cos)), Acc::adda(F::ZERO, F(cos)).msub(d, F(sin)))
}

// --- F47: the node's box, for a node in the spatial database --------------------------------------

/// The node's bounds words: its own bounds' centre and half extent, the box's centre and half extent
/// in its frame, the second flag byte, its own radius, its reach, and the nodes its gather takes in
/// (how many, and where their list is); its spatial database entry.
const OWN_CENTRE: u32 = 0xb0;
const OWN_HALF: u32 = 0xc0;
const CENTRE: u32 = 0xd0;
const HALF: u32 = 0xe0;
const RADIUS: u32 = 0x104;
const REACH: u32 = 0x108;
const LISTED: u32 = 0x10c;
const LIST: u32 = 0x144;
const ENTRY: u32 = 0x8;
/// The flag byte's bits: own bounds (a box, or with [`SPHERE`] a sphere), bounds of the node's own
/// to gather, bounds gathered as a sphere, a reach above 0; the second byte's: the database box round
/// the node's origin.
const OWN_BOX: u32 = 0x04;
const OWN_BOUNDS: u32 = 0x08;
const SPHERE: u32 = 0x10;
const BOUNDED: u32 = 0x40;
const ORIGIN_REACH: u32 = 0x04;

fn f(m: &mut impl Ram, at: u32) -> F {
    F(f32::from_bits(m.word(at)))
}

fn xyz_of(a: Vf, b: Vf) -> Vf {
    a.merge(Dest::XYZ, b)
}

/// **The node's bounds are to be gathered again** (as [`crate::atv_moto::Scene::bounds_stale`]),
/// its world frame brought up to date first: stale (bit 7), and its own to gather (bit 3) or none
/// yet (bit 6 clear).
pub fn bounds_stale(m: &mut impl Ram, n: u32) -> bool {
    update(m, n);
    let fl = flags(m, n);
    fl & STALE_BOUNDS != 0 && (fl & OWN_BOUNDS != 0 || fl & BOUNDED == 0)
}

/// Where a listed node's box gather leaves its frames: the gathering node's inverse, then the listed
/// node's in it (the scratchpad's first 0x80 bytes).
const SCRATCHPAD: u32 = 0x7000_0000;

/// **The node's bounds gathered** (its vt `+0x94`, as [`crate::atv_moto::Scene::rebound`] gathers
/// them, here over the nodes in RAM): its world frame brought up to date, then its own bounds (bit 2)
/// and each listed node's that has its own (each brought up to date as it is taken in) as one sphere
/// (bit 4) or one box in the node's frame. The centre and half extent go to the box words, the reach
/// to `+0x108`, and a reach above 0 sets bit 6.
///
/// A sphere with neither its own bounds nor a listed node with them has no centre: the original's is
/// a stack word never written. The port takes `(0, 0, 0, 0)`, as the scene's does.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn rebound(m: &mut impl Ram, n: u32) {
    update(m, n);
    let fl = flags(m, n);
    let (own_centre, own_half, radius) = (v(m, n + OWN_CENTRE), v(m, n + OWN_HALF), f(m, n + RADIUS));
    let count = m.word(n + LISTED);
    let list = m.word(n + LIST);
    let listed: Vec<u32> = (0..count).map(|k| m.word(list.wrapping_add(4 * k))).collect();
    let reach = if fl & SPHERE != 0 {
        let (mut centre, mut r) = (Vf::ZERO, F::ZERO);
        if fl & OWN_BOX != 0 {
            let world = rows(m, n + WORLD);
            (centre, r) = (frame_point(&world, own_centre), radius);
        }
        for o in listed {
            if flags(m, o) & OWN_BOX == 0 {
                continue;
            }
            update(m, o);
            let p = frame_point(&rows(m, o + WORLD), v(m, o + OWN_CENTRE));
            let or = f(m, o + RADIUS);
            if r <= F::ZERO {
                (centre, r) = (p, or);
            } else {
                let d = xyz_of(p, p.sub(centre)).abs();
                let far = F(vu::max(vu::max(d.x(), d.y()), d.z())) + or;
                if !(far <= r) {
                    r = far;
                }
            }
        }
        update(m, n);
        let c = point_in_frame(&rows(m, n + WORLD), centre);
        put_v(m, n + CENTRE, c);
        let half = v(m, n + HALF);
        put_v(m, n + HALF, xyz_of(half, Vf::splat(r.0)));
        r
    } else {
        let (mut lo, mut hi, mut had) = (Vf::VF0, Vf::VF0, false);
        if fl & OWN_BOX != 0 {
            (lo, hi, had) = (xyz_of(own_centre, own_centre.sub(own_half)), xyz_of(own_centre, own_centre.add(own_half)), true);
        }
        for o in listed {
            if flags(m, o) & OWN_BOX == 0 {
                continue;
            }
            update(m, o);
            let inverse = frame_inverse(&rows(m, n + WORLD));
            put_rows(m, SCRATCHPAD, &inverse);
            let rel = frame_mul(&rows(m, o + WORLD), &inverse);
            put_rows(m, SCRATCHPAD + 0x40, &rel);
            let (c, e) = (frame_point(&rel, v(m, o + OWN_CENTRE)), v(m, o + OWN_HALF));
            let (hx, hy, hz) = (rel[0].mul(e.bc(Field::X)).abs(), rel[1].mul(e.bc(Field::Y)).abs(), rel[2].mul(e.bc(Field::Z)).abs());
            let e = Vf::madd(hy.add(hx), hz, Vf::splat(1.0));
            let (l, h) = (rel[0].merge(Dest::XYZ, c.sub(e)), rel[1].merge(Dest::XYZ, c.add(e)));
            (lo, hi) = if had { (l.mini(lo), h.max(hi)) } else { (l, h) };
            had = true;
        }
        let half_of = |x: Vf, keep: Vf| keep.merge(Dest::XYZ, x.mul(Vf::splat(0.5)));
        let (c, h) = (half_of(lo.add(hi), lo), half_of(hi.sub(lo), hi));
        put_v(m, n + CENTRE, c);
        put_v(m, n + HALF, h);
        Acc::mula(F(h.x()), F(h.x())).madda(F(h.y()), F(h.y())).madd(F(h.z()), F(h.z())).sqrt()
    };
    m.set(n + REACH, reach.to_bits());
    if !(reach <= F::ZERO) {
        let fl = flags(m, n);
        put_flags(m, n, fl | BOUNDED);
    }
}

/// **The node's box in the world** (as [`crate::atv_moto::Scene::world_box`]), its bounds gathered
/// first if they are stale ([`bounds_stale`], [`rebound`], bit 7 cleared): the centre carried
/// through the world frame, the half extent the rows times its lanes made positive; `xyz`, the `w`s
/// the box's own.
pub fn world_box(m: &mut impl Ram, n: u32) -> (Vf, Vf) {
    if bounds_stale(m, n) {
        rebound(m, n);
        let fl = flags(m, n);
        put_flags(m, n, fl & !STALE_BOUNDS);
    }
    let (w, c, e) = (rows(m, n + WORLD), v(m, n + CENTRE), v(m, n + HALF));
    let (hx, hy, hz) = (w[0].mul(e.bc(Field::X)).abs(), w[1].mul(e.bc(Field::Y)).abs(), w[2].mul(e.bc(Field::Z)).abs());
    let acc = Vf::madd(Vf::madd(w[3].add(Vf::ZERO), w[0], c.bc(Field::X)), w[1], c.bc(Field::Y));
    (xyz_of(c, Vf::madd(acc, w[2], c.bc(Field::Z))), xyz_of(e, Vf::madd(hy.add(hx), hz, Vf::splat(1.0))))
}

/// **The node's bounds into the spatial database** (`atv_node_bounds`, as
/// [`crate::atv_moto::Scene::database_box`] on [`world_box`]): nothing for a node with no entry
/// (`+8`); else the world box, round the node's origin with bit 2 of the second flag byte (the half
/// extent grown by `|centre - origin|`, the reach its length), or as it is with the node's reach.
/// The centre, half extent and reach the database move is handed.
pub fn bounds(m: &mut impl Ram, n: u32) -> Option<(Vf, Vf, F)> {
    if m.word(n + ENTRY) == 0 {
        return None;
    }
    update(m, n);
    let (centre, half) = world_box(m, n);
    if (m.word(n + FLAGS) >> 8) & ORIGIN_REACH == 0 {
        return Some((centre, half, f(m, n + REACH)));
    }
    update(m, n);
    let origin = v(m, n + WORLD + 0x30).merge(Dest::W, Vf::splat(1.0));
    let d = xyz_of(centre, centre.sub(origin));
    let d = Vf(d.0.map(|x| f32::from_bits(x.to_bits() & 0x7fff_ffff)));
    let half = xyz_of(half, half.add(d));
    let reach = Acc::mula(F(half.x()), F(half.x())).madda(F(half.y()), F(half.y())).madd(F(half.z()), F(half.z())).sqrt();
    Some((origin, half, reach))
}

/// **The spatial update's box** (the vehicle's vt `+0xe4`): nothing for an owner with no database
/// entry (`+8`); else its node's [`world_box`], which the database move is handed with reach 0.
pub fn owner_box(m: &mut impl Ram, owner: u32, n: u32) -> Option<(Vf, Vf)> {
    if m.word(owner + ENTRY) == 0 {
        return None;
    }
    Some(world_box(m, n))
}
