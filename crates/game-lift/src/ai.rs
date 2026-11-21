//! The AI rider, lifted (S496): the brain's ground controller, which turns the direction and speed
//! the route follower asked for into the bar, the lean, the throttle and the brake.
//!
//! The brain is the original's object at `ai`. What the controller reads and writes there:
//!
//! - `+0x30` flags: 0x20 on the ground, 0x10000 oversteer correction, 0x10000000 cruise,
//!   0x20000000 graded brake
//! - `+0x70`, `+0x74` the body's spin about x and y; `+0x80` speed; `+0xa0` up; `+0xb0` forward;
//!   `+0xc4` height; `+0x204` the ground's height; `+0x1d0` the direction asked for
//! - `+0x1e0` the speed asked for, raised to at least `+0x254`
//! - written: `+0xe0` throttle, `+0xe4` brake, `+0xf0` bar, `+0xf4` lean, `+0x1f0` aim, `+0x1f4`
//!   heading, `+0x1f8` roll, `+0x270` the forward flattened onto the ground plane and made unit
//! - tuning: `+0x100`/`+0x104` bar and lean scale, `+0x208` steering gain, `+0x21c` oversteer gain,
//!   `+0x260`/`+0x264` slow and fast tolerance, `+0x2b4` the height the lean starts aiming for the
//!   landing at, `+0x2c0` the ground's normal ahead, `+0x2d0` aim gain, `+0x2d4` lean gain, `+0x2d8`
//!   lean filter, `+0x2ec` landing pitch, `+0x32c`/`+0x330` cruise base and gain, `+0x334` brake gain
#![deny(clippy::float_arithmetic)]

use crate::Mem;
use ps2_float::{Acc, Dest, F, Vf, fpu, vu};

/// Where the controller's two lookup tables are, in the memory it runs over. Both are the engine's,
/// built once at startup; the lift takes their places from its caller rather than naming them.
#[derive(Clone, Copy, Debug)]
pub struct Tables {
    /// The word holding the curve table's address: 6000 `{cos, sin}` pairs over a turn.
    pub curve: u32,
    /// The float the angle is scaled by to index the curve table.
    pub curve_scale: u32,
    /// The float the leftover between two entries is scaled by.
    pub curve_step: u32,
    /// The arc cosine's 25 `{base, scale, table}` pieces.
    pub acos: u32,
}

const ONE: F = F(1.0);
const PI: F = F(f32::from_bits(0x4049_0fdb));
const TWO_PI: F = F(f32::from_bits(0x40c9_0fdb));
const INV_TWO_PI: F = F(f32::from_bits(0x3e22_f983));
const HALF_PI: F = F(f32::from_bits(0x3fc9_0fdb));

/// The `mula.s` / `madda.s` / `madd.s` dot of two quadwords' x, y and z.
fn dot3(a: Vf, b: Vf) -> F {
    Acc::mula(a.x(), b.x()).madda(a.y(), b.y()).madd(a.z(), b.z())
}

/// The arc cosine the brain measures its angles with: the argument clamped to ±1 (twice, as the
/// original does), then `acos(1 − |x|)` read off the piece its exponent picks and blended between
/// two entries, reflected about π/2 for a negative argument.
pub fn acos(m: &Mem, t: &Tables, x: F) -> F {
    let mut x = x;
    for _ in 0..2 {
        if !(x <= ONE) {
            x = ONE;
        } else if x < F(-1.0) {
            x = F(-1.0);
        }
    }
    let (base, sign, y) = if x < F(0.0) { (PI, F(-1.0), x + ONE) } else { (F(0.0), ONE, ONE - x) };
    let piece = ((y.to_bits() & 0x7f80_0000) >> 23) as i32 - 102;
    let piece = if piece >= 25 { 24 } else if piece < 0 { 0 } else { piece };
    let at = t.acos + 12 * piece as u32;
    let scaled = (y - m.f(at)) * m.f(at + 4);
    let k = scaled.to_i32();
    let frac = scaled - F(fpu::from_i32(k));
    let entry = m.u32(at + 8).wrapping_add((k as u32).wrapping_mul(4));
    let blend = Acc::mula(ONE - frac, m.f(entry)).madd(frac, m.f(entry + 4));
    base + sign * blend
}

/// The angle whose cosine is `along / |(across, along)|`, signed by `across`.
fn signed_angle(m: &Mem, t: &Tables, across: F, along: F) -> F {
    let inv = ONE.rsqrt(Acc::mula(across, across).madd(along, along));
    let a = acos(m, t, along * inv);
    if across < F(0.0) { -a } else { a }
}

/// One pass of the original's inlined wrap into ±π.
fn wrap(x: F) -> F {
    let turns = F(fpu::from_i32((INV_TWO_PI * x).to_i32()));
    let r = x - TWO_PI * turns;
    if !(r <= PI) {
        r - TWO_PI
    } else if r < -PI {
        TWO_PI + r
    } else {
        r
    }
}

/// An angle as the curve table indexes it: wrapped twice, then `(a + π) × scale` split into the
/// entry's address and the leftover.
fn curve_entry(m: &Mem, t: &Tables, a: F) -> (u32, F) {
    let at = (PI + wrap(wrap(a))) * m.f(t.curve_scale);
    let k = at.to_i32();
    let frac = at - F(fpu::from_i32(k));
    (m.u32(t.curve).wrapping_add((k as u32).wrapping_mul(8)), frac)
}

/// The curve table's sine of `a`.
fn sine(m: &Mem, t: &Tables, a: F) -> F {
    let (e, frac) = curve_entry(m, t, a);
    m.f(e + 4) + m.f(t.curve_step) * (m.f(e) * frac)
}

/// The curve table's cosine of `a`.
fn cosine(m: &Mem, t: &Tables, a: F) -> F {
    let (e, frac) = curve_entry(m, t, a);
    m.f(e) - m.f(t.curve_step) * (m.f(e + 4) * frac)
}

/// `a` held to ±π/2.
fn quarter(a: F) -> F {
    if a < -HALF_PI {
        -HALF_PI
    } else if !(a <= HALF_PI) {
        HALF_PI
    } else {
        a
    }
}

/// The word at `a` held to `[lo, 1]`, written back only when it was outside.
fn clamp_word(m: &mut Mem, a: u32, lo: F) {
    let v = m.f(a);
    if !(v <= ONE) {
        m.set_f(a, ONE);
    } else if v < lo {
        m.set_f(a, lo);
    }
}

/// The brain's ground controller, one tick: always answers 1.
pub fn controls(m: &mut Mem, t: &Tables, ai: u32) -> u32 {
    if m.f(ai + 0x1e0) < m.f(ai + 0x254) {
        m.set_f(ai + 0x1e0, m.f(ai + 0x254));
    }
    // The forward, flattened and made unit by VU0.
    m.set_vf(ai + 0x270, m.vf(ai + 0xb0));
    m.set_u32(ai + 0x274, 0);
    let flat = m.vf(ai + 0x270);
    let sq = flat.mul(flat);
    let len2 = vu::madd(vu::add(sq.x(), sq.y()), 1.0, sq.z());
    let flat = flat.merge(Dest::XYZ, flat.mul_q(vu::rsqrt(1.0, len2)));
    m.set_vf(ai + 0x270, flat);
    let side = Vf([m.f(ai + 0x278).0, 0.0, (-m.f(ai + 0x270)).0, 0.0]);

    let target = m.vf(ai + 0x1d0);
    let heading = signed_angle(m, t, dot3(target, side), dot3(target, flat));
    m.set_f(ai + 0x1f4, m.f(ai + 0x208) * heading);

    let forward = m.vf(ai + 0xb0);
    let up = m.vf(ai + 0xa0);
    let off = target.merge(Dest::XYZ, target.sub(forward));
    let aim = signed_angle(m, t, dot3(off, up), dot3(off, forward));
    m.set_f(ai + 0x1f0, -m.f(ai + 0x2d0) * aim);

    let roll = signed_angle(m, t, dot3(up, side), m.f(ai + 0xa4));
    m.set_f(ai + 0x1f8, m.f(ai + 0x208) * roll);

    let flags = m.u32(ai + 0x30);
    let bar = if flags & 0x10000 != 0 {
        m.f(ai + 0x1f4) - m.f(ai + 0x21c) * m.f(ai + 0x74)
    } else {
        m.f(ai + 0x1f4)
    };
    m.set_f(ai + 0xf0, sine(m, t, quarter(bar)));

    if flags & 0x20 != 0 {
        m.set_u32(ai + 0xf4, 0);
    } else {
        let above = m.f(ai + 0xc4) - m.f(ai + 0x204);
        if !(above <= m.f(ai + 0x2b4)) {
            let pitch = m.f(ai + 0x21c) * m.f(ai + 0x70) - m.f(ai + 0x1f0);
            m.set_f(ai + 0xf4, sine(m, t, quarter(pitch)));
        } else if above <= m.f(ai + 0x2b4) {
            // Near the ground: lean toward the landing pitch against the ground ahead, filtered.
            let landing = m.f(ai + 0x2ec);
            let s = sine(m, t, landing);
            let up = m.vf(ai + 0xa0);
            let lifted = up.merge(Dest::XYZ, up.mul(Vf::splat(s.0)));
            let c = cosine(m, t, landing);
            let forward = m.vf(ai + 0xb0);
            let level = forward.merge(Dest::XYZ, forward.mul(Vf::splat(c.0)));
            let want = level.merge(Dest::XYZ, level.add(lifted));
            let push = m.f(ai + 0x2d4) * -dot3(m.vf(ai + 0x2c0), want);
            let k = m.f(ai + 0x2d8);
            let lean = Acc::mula(m.f(ai + 0xf4), ONE - k).madd(k, push);
            m.set_f(ai + 0xf4, lean);
            if !(lean <= ONE) {
                m.set_f(ai + 0xf4, ONE);
            } else if lean < F(-1.0) {
                m.set_f(ai + 0xf4, F(-1.0));
            }
        }
    }

    m.set_f(ai + 0xf0, m.f(ai + 0xf0) * m.f(ai + 0x100));
    m.set_f(ai + 0xf4, m.f(ai + 0xf4) * m.f(ai + 0x104));
    clamp_word(m, ai + 0xf0, F(-1.0));
    clamp_word(m, ai + 0xf4, F(-1.0));

    let want = m.f(ai + 0x1e0);
    let speed = m.f(ai + 0x80);
    if speed < want - m.f(ai + 0x260) {
        m.set_f(ai + 0xe0, ONE);
        m.set_u32(ai + 0xe4, 0);
    } else if !(speed <= want + m.f(ai + 0x264)) {
        m.set_u32(ai + 0xe0, 0);
        if flags & 0x2000_0000 != 0 {
            let over = m.f(ai + 0x80) - m.f(ai + 0x1e0);
            m.set_f(ai + 0xe4, ONE - m.f(ai + 0x334) * over);
        } else {
            m.set_f(ai + 0xe4, ONE);
        }
    } else {
        if flags & 0x1000_0000 != 0 {
            let gas = Acc::mula(want, m.f(ai + 0x32c)).madd(m.f(ai + 0x330), want - speed);
            m.set_f(ai + 0xe0, gas);
        } else {
            m.set_u32(ai + 0xe0, 0);
        }
        m.set_u32(ai + 0xe4, 0);
    }
    clamp_word(m, ai + 0xe0, F(0.0));
    clamp_word(m, ai + 0xe4, F(0.0));
    1
}
