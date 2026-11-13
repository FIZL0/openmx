//! **The part walk's second particle emitter kind** (F59): the listener state 4's part walk
//! ([`super::tail::walk`]) steps with the second emitter thunk (at its object's `+0xc`), as a port on
//! RAM. Where the original keeps it: research/oracle.txt `# F59`.
//!
//! The object is the first kind's ([`super::emitter`]: the same spot, node, flags, clocks, speeds and
//! definition) with a ring after it: 120 directions ([`RING`], a quad each), an index into them
//! ([`RING_AT`]), a stride ([`RING_STRIDE`]) and a scale ([`RING_SCALE`]). Each frame, [`step`]:
//!
//! 1. runs the first kind's [`emitter::gate`]; a non-zero answer ends the frame;
//! 2. from the spot's move over `dt` and two draws of `rand`, takes the number to spawn
//!    ([`emitter::count`]) and a walk through the spread table, as the first kind does;
//! 3. puts the spot back on its node (where the first kind leaves it where the gate put it);
//! 4. aims the ring: the base and its add summed ([`AIM`]), its angle about `y` from a lookup of the
//!    arc cosine, turned into a ring index, plus a third draw of `rand` times the stride;
//! 5. spawns into the definition's pool, each particle at the spot pushed out along the ring's next
//!    direction (times the scale) plus an offset off the spread table, its velocity the ring's
//!    current direction plus the base and a push; its size, life, fade, look and flags off the
//!    table's entries after.
//!
//! The calls it makes that are not the emitter's own work are handed out: the first kind's
//! ([`emitter::Call`]: `rand`, the gate's, settle and bind), and the clock read and `srand` that seed
//! `rand` the first time any emitter of this kind aims ([`Rings`]).
#![deny(clippy::float_arithmetic)] // the lint that keeps a bit-exact port bit-exact

use super::emitter::{self, Call, Emitters};
use super::node;
use super::update::Quad;
use ps2_float::vu;
use ps2_float::{F, Vf, fpu, fpu::Acc};

/// The ring's words: the index of its current direction (`+0x100`), the stride it steps by
/// (`+0x108`), the aim (`+0x110`: the base plus its add, written each spawning frame), the scale the
/// spawn point is pushed out by (`+0x134`), and its 120 directions (`+0x140`, a quad each).
pub const RING_AT: u32 = 0x100;
pub const RING_STRIDE: u32 = 0x108;
pub const AIM: u32 = 0x110;
pub const RING_SCALE: u32 = 0x134;
pub const RING: u32 = 0x140;
pub const RING_LEN: i32 = 120;

/// The quad this kind's spread entries are centred on (not the first kind's).
pub const SPREAD_CENTRE: u32 = 0x0052_53a0;
/// The direction the aim's angle is taken from (its arc cosine against the aim, the sign from the
/// `y` of their cross product).
pub const AIM_FROM: u32 = 0x0039_7010;
/// The arc cosine's lookup: a pointer to its table, and the float that scales `1 + cos` to an index.
pub const ACOS_TABLE: u32 = 0x003e_a9c0;
pub const ACOS_SCALE: u32 = 0x003e_a9d8;
/// The seed `rand` was given (0 until the first emitter of this kind aims: then the clock's).
pub const SEEDED: u32 = 0x0052_cd80;

/// `rand`'s scale, 2⁻³¹.
const RAND_SCALE: u32 = 0x3000_0000;
/// The second draw's walks, as the first kind's: at or under each bound, (direction, stride).
const WALKS: [(u32, i32, i32); 5] = [(0x3e19_999a, 1, 1), (0x3e99_999a, 2, 2), (0x3ee6_6666, 3, 3), (0x3f33_3333, -1, 2), (0x3f59_999a, -2, 2)];
const TWO_PI: u32 = 0x40c9_0fdb;
/// Three eighths of a turn, added to the aim's angle.
const ANGLE_OFF: u32 = 0x4016_cbe4;

/// What this kind hands out beside the first kind's calls: the clock and `srand`, run once.
pub trait Rings: Emitters {
    /// The real-time clock read (the original's eight bytes on its stack): answers its seconds,
    /// minutes, hours and day (the bytes at 1, 2, 3 and 5), a byte each from the low one up.
    fn clock(&mut self) -> u32;
    /// `srand` of the seed.
    fn seed(&mut self, seed: u32);
}

fn byte(m: &mut impl Emitters, at: u32) -> u8 {
    (m.word(at & !3) >> (8 * (at & 3))) as u8
}

fn half(m: &mut impl Emitters, at: u32) -> u16 {
    (m.word(at & !3) >> (8 * (at & 2))) as u16
}

fn f(m: &mut impl Emitters, at: u32) -> F {
    F::from_bits(m.word(at))
}

fn set_f(m: &mut impl Emitters, at: u32, x: F) {
    m.set(at, x.to_bits());
}

fn quad(m: &mut impl Emitters, at: u32) -> Quad {
    [0, 4, 8, 12].map(|k| m.word(at + k))
}

fn put_quad(m: &mut impl Emitters, at: u32, q: Quad) {
    for (k, w) in q.into_iter().enumerate() {
        m.set(at + 4 * k as u32, w);
    }
}

fn v(q: Quad) -> Vf {
    Vf::from_bits(q)
}

/// A `.xyz` result: `r`'s three lanes, `w` the raw word `of` holds.
fn xyz(r: Vf, of: Quad) -> Quad {
    let b = r.to_bits();
    [b[0], b[1], b[2], of[3]]
}

fn def(m: &mut impl Emitters, o: u32) -> u32 {
    m.word(o + emitter::DEF)
}

fn add(a: F, b: F) -> F {
    F(fpu::add(a.0, b.0))
}
fn sub(a: F, b: F) -> F {
    F(fpu::sub(a.0, b.0))
}
fn mul(a: F, b: F) -> F {
    F(fpu::mul(a.0, b.0))
}
fn div(a: F, b: F) -> F {
    F(fpu::div(a.0, b.0))
}

/// `rand` times 2⁻³¹.
fn draw(m: &mut impl Emitters) -> F {
    let r = m.out(Call::Rand) as i32;
    mul(F(f32::from_bits(RAND_SCALE)), F(fpu::from_i32(r)))
}

fn entry(m: &mut impl Emitters, i: i32) -> F {
    f(m, emitter::SPREAD.wrapping_add((i as u32).wrapping_mul(4)))
}

/// The ring's direction `k` (`lqc2` of the object's `+0x140 + 16k`).
fn ring(m: &mut impl Emitters, o: u32, k: u32) -> Quad {
    let at = o.wrapping_add(k.wrapping_mul(16)).wrapping_add(RING);
    quad(m, at)
}

/// `v`'s `xyz` made a unit vector (`vrsqrt` of the lanes' squares summed `x + y` then `+ z`), `w`
/// its own.
fn unit(q: Quad) -> Quad {
    let x = v(q);
    let sq = x.mul(x);
    let s = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    xyz(x.mul_q(vu::rsqrt(1.0, s)), q)
}

/// `x` held to `-1..=1` (a NaN to 1), as the original's two compares do.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
fn clamp_unit(x: F) -> F {
    if !(x <= 1.0) {
        F(1.0)
    } else if x < -1.0 {
        F(-1.0)
    } else {
        x
    }
}

/// **The ring aimed** (into [`RING_AT`]): the angle about `y` of the aim against [`AIM_FROM`] — the
/// arc cosine of their dot by the lookup, its sign by their cross product's `y` — turned three
/// eighths on and into an index of the ring, plus a draw of `rand` (seeding it from the clock the
/// first time) times the stride, round the ring.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn aim(m: &mut impl Rings, o: u32) {
    let aim = quad(m, o + AIM);
    let d = unit(aim);
    let c = v(quad(m, AIM_FROM));
    let dv = v(d);
    let cross = Vf::opmsub(Vf::opmula(c, dv), dv, c);
    let up = F(cross.y());
    let dot = Acc::mula(F(c.x()), F(dv.x())).madda(F(c.y()), F(dv.y())).madd(F(c.z()), F(dv.z()));
    let cos = clamp_unit(clamp_unit(dot));
    let t = mul(add(F(1.0), cos), f(m, ACOS_SCALE));
    let i = fpu::to_i32(t.0);
    let table = m.word(ACOS_TABLE).wrapping_add((i as u32).wrapping_mul(4));
    let frac = sub(t, F(fpu::from_i32(i)));
    let (t0, t1) = (f(m, table), f(m, table.wrapping_add(4)));
    let mut a = Acc::mula(frac, t1).madd(sub(F(1.0), frac), t0);
    let two_pi = F(f32::from_bits(TWO_PI));
    if up < 0.0 {
        a = sub(two_pi, a);
    }
    let mut a = add(F(f32::from_bits(ANGLE_OFF)), sub(two_pi, a));
    if !(a <= two_pi) {
        a = sub(a, two_pi);
    }
    let at = fpu::to_i32(fpu::mul(120.0, div(a, two_pi).0));
    let stride = m.word(o + RING_STRIDE).wrapping_sub(1);
    if m.word(SEEDED) == 0 {
        let [s, mi, h, day] = m.clock().to_le_bytes().map(u32::from);
        let seed = s.wrapping_add(60 * mi).wrapping_add(360 * h).wrapping_add(86_400 * day);
        m.set(SEEDED, seed);
        m.seed(seed);
    }
    let r = m.out(Call::Rand) as i32;
    let u = mul(F(f32::from_bits(RAND_SCALE)), F(fpu::from_i32(r)));
    let k = fpu::to_i32(fpu::mul(fpu::from_i32(stride.wrapping_add(1) as i32), u.0));
    m.set(o + RING_AT, (at.wrapping_add(k) % RING_LEN) as u32);
}

/// **The second emitter kind's step** for a frame of `dt` (the thunk's target, on the object `o`):
/// the first kind's [`emitter::gate`], then the spawns round the ring. Answers 1, as the original.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // the original's ordered compares
pub fn step(m: &mut impl Rings, o: u32, dt: F) -> u32 {
    if emitter::gate(m, o, dt) != 0 {
        return 1;
    }
    let n = m.word(o + emitter::NODE);
    if n != 0 {
        let owner = m.word(n + 8);
        if owner != 0 {
            let at = m.word(owner + 4).wrapping_add(0xa4);
            if half(m, emitter::FRAME_ID) != half(m, at) {
                return 1;
            }
        }
    }
    let spot = quad(m, o + emitter::SPOT);
    let moved = xyz(v(spot).sub(v(quad(m, o + emitter::LAST))), spot);
    let (x, y, z) = (F::from_bits(moved[0]), F::from_bits(moved[1]), F::from_bits(moved[2]));
    let dist = F(fpu::sqrt(Acc::mula(x, x).madda(y, y).madd(z, z).0));
    let speed = div(dist, dt);
    let d = def(m, o);
    let mut vel = if byte(m, d + emitter::DEF_OPTIONS) & emitter::DEF_VELOCITY != 0 {
        let q = vu::div(1.0, dt.0);
        xyz(v(moved).mul_q(q), moved)
    } else {
        quad(m, emitter::ZERO_QUAD)
    };
    let fast = !(speed <= f(m, o + emitter::FAST_ABOVE));
    if speed < f(m, o + emitter::SLOWEST) {
        return 1;
    }
    let u = draw(m);
    let count = emitter::count(m, o, fast, dt, dist, speed, u);
    let u2 = draw(m);
    let (mut dir, mut stride) = WALKS.iter().find(|w| u2 <= f32::from_bits(w.0)).map_or((-3, 3), |w| (w.1, w.2));
    if byte(m, o + emitter::FLAGS) & emitter::IN_FRAME != 0 {
        let n = m.word(o + emitter::NODE);
        if n != 0 {
            node::update(m, n);
        }
    }
    // the spot back on the node, its offset dropped; with no node, the words at the node's own
    // position row from address 0 (the original's call takes the node's own row when the node is
    // the frame asked for, and both are 0)
    let n = m.word(o + emitter::NODE);
    if n != 0 {
        node::position(m, n, o + emitter::SPOT);
    } else {
        for k in 0..3 {
            let w = m.word(0x60 + 4 * k);
            m.set(o + emitter::SPOT + 4 * k, w);
        }
        m.set(o + emitter::SPOT + 12, F::ONE.to_bits());
    }
    if count <= 0 {
        return 1;
    }
    let d = def(m, o);
    if m.word(d + emitter::DEF_MODE) == 1 {
        let b = byte(m, o + emitter::FLAGS);
        m.set_byte(o + emitter::FLAGS, (b & !emitter::LIT) | emitter::LIT);
    }
    let d = def(m, o);
    let mut base = if byte(m, d + emitter::DEF_OPTIONS) & emitter::DEF_VELOCITY != 0 {
        emitter::base(m, o, &mut vel)
    } else {
        // the base in the emitter's frame, only its `x` added to the velocity
        let rows = m.word(o + emitter::FRAME);
        let b = quad(m, o + emitter::BASE);
        let t = emitter::in_frame(m, rows, b);
        let mut q = vel;
        q[0] = add(F::from_bits(q[0]), F::from_bits(t[0])).to_bits();
        q
    };
    let d = def(m, o);
    if byte(m, d + emitter::DEF_OPTIONS) & emitter::DEF_POOL_PUSH != 0 {
        let pool = m.word(d + emitter::POOL);
        base = xyz(v(base).add(v(quad(m, pool + 0x140))), base);
    }
    let mut idx = fpu::to_i32(fpu::mul(u.0, fpu::from_i32(599 - 13 * stride)));
    let d = def(m, o);
    let stepped = m.word(d + emitter::DEF_FLAGS) & emitter::LOOK_STEPPED != 0;
    let b = quad(m, o + emitter::BASE);
    let sum = xyz(v(b).add(v(quad(m, o + emitter::BASE_ADD))), b);
    put_quad(m, o + AIM, sum);
    aim(m, o);
    let centre = quad(m, SPREAD_CENTRE);
    for _ in 0..count {
        let d = def(m, o);
        let pool = m.word(d + emitter::POOL);
        let k = m.word(pool + emitter::POOL_COUNT);
        if !((k as i32) < m.word(pool + emitter::POOL_ROOM) as i32) {
            return 1;
        }
        if (dir > 0 && idx + 13 * stride >= emitter::SPREAD_LEN) || (dir < 0 && idx < 13 * stride) {
            dir = -dir;
            if dir >= 2 {
                dir -= 1;
                stride -= 1;
            } else if dir < -1 {
                dir += 1;
                stride += 1;
            }
        }
        let array = m.word(pool + emitter::POOL_ARRAY);
        let p = m.word(array.wrapping_add(k.wrapping_mul(4)));
        let at = |i: i32| emitter::SPREAD.wrapping_add((i as u32).wrapping_mul(4));
        let raw = [m.word(at(idx + dir)), m.word(at(idx + 2 * dir)), m.word(at(idx + 3 * dir)), 1.0f32.to_bits()];
        let off = xyz(Vf::from_bits(raw).sub(v(centre)), raw);
        let o3 = v(off);
        let most = fpu::max(fpu::max(fpu::abs(o3.x()), fpu::abs(o3.y())), fpu::abs(o3.z()));
        let spread = unit(off);
        let spread = xyz(v(spread).mul(Vf::splat(most)), spread);
        // the offset: the definition's `+0xd0`..`+0xe0` box's middle plus its `+0xf0` by the spread
        let d = def(m, o);
        let r = quad(m, d + 0xf0);
        let along = xyz(v(r).mul(v(spread)), r);
        let hi = quad(m, d + 0xe0);
        let mid = xyz(v(hi).add(v(quad(m, d + 0xd0))), hi);
        let mid = xyz(v(mid).mul(Vf::splat(0.5)), mid);
        let mut offset = xyz(v(mid).add(v(along)), mid);
        // the push: `+0x130`..`+0x140`'s middle plus `+0x150` by the spread
        let r = quad(m, d + 0x150);
        let along = xyz(v(r).mul(v(spread)), r);
        let hi = quad(m, d + 0x140);
        let mid = xyz(v(hi).add(v(quad(m, d + 0x130))), hi);
        let mid = xyz(v(mid).mul(Vf::splat(0.5)), mid);
        let mut push = xyz(v(mid).add(v(along)), mid);
        let b = byte(m, o + emitter::FLAGS);
        if b & emitter::IN_FRAME != 0 {
            let rows = m.word(o + emitter::FRAME);
            push = emitter::in_frame(m, rows, push);
            let rows = m.word(o + emitter::FRAME);
            offset = emitter::in_frame(m, rows, offset);
        } else if b & emitter::TURNED != 0 {
            push = emitter::turned(m, o + emitter::TURN, push);
            offset = emitter::turned(m, o + emitter::TURN, offset);
        }
        // the ring's current direction (the velocity's) and the next (the spawn point's)
        let now = m.word(o + RING_AT);
        let r = ring(m, o, now);
        let heading = xyz(v(r).add(v(base)), r);
        let next = m.word(o + RING_AT).wrapping_add(m.word(o + RING_STRIDE));
        m.set(o + RING_AT, next);
        let next = m.word(o + RING_AT) as i32 % RING_LEN;
        m.set(o + RING_AT, next as u32);
        let r = ring(m, o, next as u32);
        let s = f(m, o + RING_SCALE);
        let out = xyz(v(r).mul(Vf::splat(s.0)), r);
        let spot = quad(m, o + emitter::SPOT);
        let at_ring = xyz(v(spot).add(v(out)), spot);
        put_quad(m, p, xyz(v(at_ring).add(v(offset)), at_ring));
        // size: the scaled one off the size before this one's
        let d = def(m, o);
        let s = mul(f(m, p + 0x34), f(m, d + 0x10c));
        set_f(m, p + 0x30, s);
        let t = entry(m, idx + 4 * dir);
        let d = def(m, o);
        let size = Acc::adda(0.0, f(m, d + 0x100)).madd(f(m, d + 0x108), t);
        set_f(m, p + 0x34, size);
        let t = entry(m, idx + 5 * dir);
        let d = def(m, o);
        let life = Acc::adda(0.0, f(m, d + 0xbc)).madd(f(m, d + 0xc4), t);
        set_f(m, p + 0x3c, life);
        let r = div(F(1.0), f(m, p + 0x34));
        set_f(m, p + 0x38, r);
        put_quad(m, p + 0x10, xyz(v(heading).add(v(push)), heading));
        let t = entry(m, idx + 6 * dir);
        let d = def(m, o);
        let fade = Acc::adda(0.0, f(m, d + 0x118)).madd(f(m, d + 0x120), t);
        set_f(m, p + 0x44, fade);
        m.set(p + 0x54, o);
        let d = def(m, o);
        let flags = m.word(d + emitter::DEF_FLAGS);
        m.set(p + 0x48, flags);
        idx += 7 * dir;
        let t = entry(m, idx);
        let lk = emitter::look(m, o, t, stepped);
        m.set_half(p + 0x26, lk as u16);
        if f(m, p + 0x44) == 0.0 {
            let w = m.word(p + 0x48);
            m.set(p + 0x48, w & !8);
        }
        let d = def(m, o);
        if byte(m, d + emitter::DEF_OPTIONS) & emitter::DEF_SETTLE != 0 {
            m.out(Call::Settle(p));
            let y = add(f(m, p + 4), F::from_bits(offset[1]));
            set_f(m, p + 4, y);
        }
        let d = def(m, o);
        if m.word(d + emitter::DEF_FLAGS) & emitter::SPINS != 0 {
            let spin = m.word(d + 0xa0);
            m.set(p + 0x4c, spin);
        } else {
            m.set(p + 0x4c, 0);
            m.set(p + 0x50, 0);
        }
        if m.word(o + emitter::BIND) == 1 {
            m.out(Call::Bind { emitter: o, particle: p });
        }
        idx += dir;
        let fade_a = !(entry(m, idx) <= 0.5);
        idx += dir;
        let fade_b = !(entry(m, idx) <= 0.5);
        let d = def(m, o);
        let w = m.word(p + 0x48);
        if fade_a && byte(m, d + emitter::DEF_OPTIONS) & emitter::DEF_FADE_A != 0 {
            m.set(p + 0x48, w | 0x4000);
        } else {
            m.set(p + 0x48, w & !0x4000);
        }
        let d = def(m, o);
        let w = m.word(p + 0x48);
        if fade_b && byte(m, d + emitter::DEF_OPTIONS) & emitter::DEF_FADE_B != 0 {
            m.set(p + 0x48, w | 0x8000);
        } else {
            m.set(p + 0x48, w & !0x8000);
        }
        let d = def(m, o);
        let pool = m.word(d + emitter::POOL);
        let k = m.word(pool + emitter::POOL_COUNT);
        m.set(pool + emitter::POOL_COUNT, k.wrapping_add(1));
        if !stepped {
            idx += dir;
            let t = entry(m, idx);
            let d = def(m, o);
            let (lo, hi) = (m.word(d + 0x38), m.word(d + 0x3c));
            let span = fpu::from_i32(hi.wrapping_sub(lo) as i32);
            let k = fpu::to_i32(fpu::mul(span, t.0));
            m.set_byte(o + emitter::LOOK, lo.wrapping_add(k as u32) as u8);
        }
    }
    1
}
