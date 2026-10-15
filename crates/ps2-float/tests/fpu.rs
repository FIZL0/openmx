//! The FPU model's own properties: each test is one deviation from IEEE 754, written so that it fails
//! loudly if someone "fixes" the model into ordinary floating point.
//!
//! These are *properties*, not captures. Proving the model against a particular console means running
//! the game's own code: `cargo test -p ps2-oracle`.

use ps2_float::fpu::{self, F, f};

#[test]
fn every_rounding_is_toward_zero() {
    // 1/3 rounds down in binary; the IEEE nearest result is one ulp above the chopped one.
    assert_eq!(fpu::div(1.0, 3.0), 1.0f32 / 3.0, "divide is the exception: it rounds to nearest");
    // a product whose exact value sits between two floats loses the remainder entirely
    let (a, b) = (1.0000001f32, 1.0000002f32);
    assert!(fpu::mul(a, b) <= (a as f64 * b as f64) as f32);
    let mut worst = 0;
    for i in 0..100_000u32 {
        let x = f32::from_bits(0x3f80_0000 + i * 7);
        let y = f32::from_bits(0x3fc0_0000 + i * 13);
        let exact = x as f64 * y as f64;
        let got = fpu::mul(x, y) as f64;
        assert!(got.abs() <= exact.abs(), "mul({x}, {y}) rounded away from zero");
        worst = worst.max((exact - got).abs().to_bits());
    }
    assert!(worst > 0, "no input was inexact — the test is not exercising the chop");

    // negatives chop toward zero too, i.e. upward
    let n = fpu::mul(-1.0000001, 1.0000002);
    assert!(n >= (-1.0000001f64 * 1.0000002f64) as f32);
}

#[test]
fn chop_only_moves_one_bit_and_only_inward() {
    for i in 0..200_000u64 {
        let x = f64::from_bits(0x3ff0_0000_0000_0000 + i * 0x4_0000_0003);
        let got = fpu::chop(x);
        let near = x as f32;
        assert!(got == near || got.to_bits() == near.to_bits() - 1, "chop({x}) vs nearest {near}");
        assert!((got as f64).abs() <= x.abs(), "chop({x}) grew");
    }
}

#[test]
fn the_adder_keeps_one_guard_bit_and_gives_up_at_25() {
    // The headline symptom: a small addend simply disappears.
    assert_eq!(fpu::add(1.0, 1.0e-8), 1.0);
    assert_eq!(f(1.0) + 1.0e-8, 1.0);

    // Where the host and the EE part company. 1.5·2^-25 is 25 exponents below 1.0, so the adder keeps
    // only its sign: the subtraction is the identity, where the host moves one ulp.
    let d25 = f32::from_bits(0x3340_0000);
    assert_eq!(fpu::sub(1.0, d25), 1.0);
    assert_eq!(1.0f32 - d25, 0.99999994, "the host keeps it");

    // One exponent higher the operand survives, but only its top mantissa bit (the guard bit) does —
    // so the EE's answer is one ulp *above* the host's correctly-rounded one.
    let d24 = f32::from_bits(0x33c0_0000);
    assert_eq!(fpu::sub(1.0, d24), 0.99999994);
    assert_eq!(1.0f32 - d24, 0.9999999, "the host keeps the bits below the guard bit");

    // the mantissa below the guard bit is gone before the add, not rounded into it
    let all_ones = f32::from_bits(0x3380_0000 | 0x007f_ffff); // just under 2·2^-24
    assert_eq!(fpu::sub(1.0, all_ones), fpu::sub(1.0, f32::from_bits(0x3380_0000)));

    // a worked case from a real port: lerp(0.95, 0.9, 0.25) is exactly 0.9375 on the PS2, where plain
    // chopped arithmetic without the alignment step gives 0.93749994.
    assert_eq!(fpu::lerp(0.95, 0.9, 0.25), 0.9375);
}

#[test]
fn madd_is_a_chopped_product_then_an_aligned_add() {
    let (acc, a, b) = (1.0f32, 1.0 / 3.0, 3.000_001);
    assert_eq!(fpu::madd(acc, a, b), fpu::add(acc, fpu::mul(a, b)), "madd must not fuse");
    assert_ne!(fpu::madd(acc, a, b).to_bits(), acc.mul_add(a, b).to_bits(),
               "and it is not the host's FMA either");
    // the accumulate form keeps the same two steps
    assert_eq!(fpu::msub(acc, a, b), fpu::sub(acc, fpu::mul(a, b)));
    assert_eq!(f(acc).madd(a, b), fpu::madd(acc, a, b));
    // a product too small to reach the accumulator vanishes, exactly as a plain add would
    assert_eq!(fpu::madd(1.0, 1.0e-4, 1.0e-5), 1.0);
}

#[test]
fn denormals_are_zero_coming_in_and_going_out() {
    let d = f32::from_bits(0x0040_0000); // 5.9e-39, a denormal
    assert!(d != 0.0, "the host can hold it");
    assert_eq!(fpu::operand(d.to_bits()), 0.0, "the FPU reads it as zero");
    assert_eq!(fpu::add(d, d), 0.0);
    assert_eq!(fpu::mul(1.0, d), 0.0);
    assert_eq!(fpu::add(1.0, d), 1.0);
    // a product that would underflow into the denormal range comes out as exactly zero
    assert_eq!(fpu::mul(1.0e-30, 1.0e-20), 0.0);
    assert_eq!(fpu::mul(-1.0e-30, 1.0e-20).to_bits(), (-0.0f32).to_bits(), "and keeps its sign");
    // ±0 is preserved
    assert_eq!(fpu::operand(0x8000_0000).to_bits(), (-0.0f32).to_bits());
}

#[test]
fn there_are_no_infinities_and_no_nans() {
    assert_eq!(fpu::mul(f32::MAX, 2.0), fpu::MAX, "overflow saturates");
    assert_eq!(fpu::mul(-f32::MAX, 2.0), -fpu::MAX);
    assert_eq!(fpu::add(f32::MAX, f32::MAX), fpu::MAX);
    assert_eq!(fpu::div(1.0, 0.0), fpu::MAX, "the divider only looks at the signs");
    assert_eq!(fpu::div(-1.0, 0.0), -fpu::MAX);
    assert_eq!(fpu::div(0.0, 0.0), fpu::MAX, "0/0 is +max, not a NaN");
    assert_eq!(fpu::div(0.0, -0.0), -fpu::MAX);
    // an Inf or NaN bit pattern (from a save state, a file, a host computation) reads as ±max
    assert_eq!(fpu::operand(0x7f80_0000), fpu::MAX);
    assert_eq!(fpu::operand(0xff80_0000), -fpu::MAX);
    assert_eq!(fpu::operand(0x7fc0_0000), fpu::MAX);
    assert!(fpu::MAX.is_finite() && !fpu::MAX.is_nan(), "nothing this model returns is non-finite");
}

#[test]
fn divide_and_root_round_to_nearest() {
    // a different unit, a different rounding mode: these agree with the host bit for bit
    for i in 1..50_000u32 {
        let a = f32::from_bits(0x3f80_0000 + i * 11);
        let b = f32::from_bits(0x4000_0000 + i * 7);
        assert_eq!(fpu::div(a, b).to_bits(), (a / b).to_bits(), "div({a}, {b})");
        assert_eq!(fpu::sqrt(b).to_bits(), b.sqrt().to_bits());
    }
    assert_eq!(fpu::sqrt(-4.0), 2.0, "sqrt.s ignores the sign");
    assert_eq!(fpu::rsqrt(1.0, 4.0), 0.5);
}

#[test]
fn the_wrapper_reads_like_the_decompilation() {
    // one mul.s, one add.s, in that order — and nothing Rust does may change that
    let (y, vy, dt) = (f(10.0), f(-1.0 / 3.0), f(1.0 / 60.0));
    assert_eq!((y + vy * dt).to_bits(), fpu::add(10.0, fpu::mul(-1.0 / 3.0, 1.0 / 60.0)).to_bits());
    // f32 on either side, in any position
    assert_eq!((1.0 + f(2.0) * 3.0).to_f32(), 7.0);
    assert_eq!((f(1.0) - 0.5) / 2.0, F(0.25));
    let mut a = f(1.0);
    a += 1.0e-8;
    assert_eq!(a, 1.0, "even through +=");
    assert_eq!(-f(2.0), F(-2.0));
    assert!(f(1.0) < 2.0 && 0.5 < f(1.0) && f(1.0) == 1.0);
    assert_eq!(f(-3.0).abs().max(2.0).min(f(10.0)), F(3.0));
    assert_eq!(f(2.5).to_i32(), 2, "cvt.w.s truncates");
    assert_eq!(f(-2.5).to_i32(), -2);
    assert_eq!(F::from(3), F(3.0));
    assert_eq!(f32::from(f(3.0)), 3.0);
}

#[test]
fn conversions_match_the_instructions() {
    assert_eq!(fpu::to_i32(2.9), 2);
    assert_eq!(fpu::to_i32(-2.9), -2);
    assert_eq!(fpu::from_i32(1 << 24), 16777216.0);
    assert_eq!(fpu::from_i32(-1), -1.0);
    // (float)(unsigned) is a compiler sequence, not an instruction; it chops like everything else
    assert_eq!(fpu::from_u32(0xffff_ffff), 4294967040.0);
    assert_eq!(fpu::from_u32(1 << 31), 2147483648.0);
    // cvt.w.s saturates rather than wrapping, like the hardware
    assert_eq!(fpu::to_i32(1.0e30), i32::MAX);
    assert_eq!(fpu::to_i32(-1.0e30), i32::MIN);
}

// --- F1: the four basic operations against models that share no code with them ----------------------

/// An independent model of `add.s`, `sub.s` and `mul.s` on bit patterns, in integer arithmetic only:
/// no host float operation anywhere in it, so it shares neither code nor rounding with the model under
/// test. Written straight from the reference emulator's rules — the operand fix-up, the one-guard-bit
/// alignment mask, an exact integer sum or product, truncation toward zero, flush-to-zero on the result
/// and saturation at `±max` where IEEE would produce an infinity.
mod int_model {
    /// Exponent 255 reads as `±max`, a denormal reads as `±0`.
    fn operand(bits: u32) -> u32 {
        match bits & 0x7f80_0000 {
            0x7f80_0000 => (bits & 0x8000_0000) | 0x7f7f_ffff,
            0 => bits & 0x8000_0000,
            _ => bits,
        }
    }

    /// `(-1)^sign · m · 2^e` as a float: the significand truncated to 24 bits (whatever falls off the
    /// bottom is dropped, not rounded), anything below the smallest normal flushed to `±0`, anything
    /// above the largest saturated to `±max`.
    fn pack(sign: u32, mut m: u64, e: i32) -> u32 {
        if m == 0 {
            return sign;
        }
        let width = 64 - m.leading_zeros() as i32;
        if width > 24 {
            m >>= width - 24;
        } else {
            m <<= 24 - width;
        }
        let biased = e + (width - 24) + 150; // m is now 1.xxx · 2^23
        if biased > 254 {
            return sign | 0x7f7f_ffff;
        }
        if biased < 1 {
            return sign;
        }
        sign | ((biased as u32) << 23) | (m as u32 & 0x007f_ffff)
    }

    /// The signed significand of an operand and the exponent of its lowest bit.
    fn part(x: u32) -> (i64, i32) {
        let e = ((x >> 23) & 0xff) as i32;
        let m = if e == 0 { 0 } else { ((x & 0x007f_ffff) | 0x0080_0000) as i64 };
        (if x & 0x8000_0000 != 0 { -m } else { m }, e - 150)
    }

    pub fn add(a: u32, b: u32) -> u32 {
        addsub(a, b, 0)
    }

    pub fn sub(a: u32, b: u32) -> u32 {
        addsub(a, b, 0x8000_0000)
    }

    fn addsub(a: u32, b: u32, flip: u32) -> u32 {
        // the mask is decided by the exponents, which a sign flip does not touch, so `sub` is `add`
        // with the second operand negated first
        let (mut a, mut b) = (operand(a), operand(b) ^ flip);
        let d = (((a >> 23) & 0xff) as i32) - (((b >> 23) & 0xff) as i32);
        match d {
            25..=255 => b &= 0x8000_0000,
            1..=24 => b &= 0xffff_ffff_u32 << (d - 1),
            -24..=-1 => a &= 0xffff_ffff_u32 << (-d - 1),
            -255..=-25 => a &= 0x8000_0000,
            _ => {}
        }
        let ((ma, ea), (mb, eb)) = (part(a), part(b));
        match (ma, mb) {
            (0, 0) => a & b & 0x8000_0000, // `-0` only when both of them are
            (0, _) => b,                   // `x ± 0` is `x`, with nothing left to round
            (_, 0) => a,
            // after the mask the exponents are at most 24 apart, so the sum is exact in 49 bits
            _ => {
                let (total, e) = if ea >= eb {
                    ((ma << (ea - eb)) + mb, eb)
                } else {
                    (ma + (mb << (eb - ea)), ea)
                };
                if total == 0 {
                    return 0; // an exact cancellation is `+0` in this rounding mode
                }
                pack(if total < 0 { 0x8000_0000 } else { 0 }, total.unsigned_abs(), e)
            }
        }
    }

    pub fn mul(a: u32, b: u32) -> u32 {
        let (a, b) = (operand(a), operand(b));
        let sign = (a ^ b) & 0x8000_0000;
        let (ma, ea) = part(a);
        let (mb, eb) = part(b);
        if ma == 0 || mb == 0 {
            return sign;
        }
        pack(sign, ma.unsigned_abs() * mb.unsigned_abs(), ea + eb)
    }
}

/// Patterns worth hitting on purpose: both zeroes, both ends of the normal range, denormals, the
/// infinity and NaN encodings, and the powers of two the alignment mask turns over.
fn edge_patterns() -> Vec<u32> {
    let mut v = vec![
        0x0000_0000, 0x8000_0000, // ±0
        0x0000_0001, 0x007f_ffff, // the smallest and largest denormals
        0x0080_0000, 0x7f7f_ffff, // the smallest and largest normals
        0x7f80_0000, 0xff80_0000, // ±inf
        0x7fc0_0000, 0x7fff_ffff, // NaNs
        0x3f80_0000, 0xbf80_0000, // ±1
        0x4000_0000, 0x3f00_0000, // 2, 0.5
        0x0080_0001, 0x7f7f_fffe,
    ];
    // ±(1 + every single mantissa bit) and ±(1 + all of them), at the exponents around 1.0 that the
    // guard-bit mask cares about
    for e in [0x3380_0000_u32, 0x3f80_0000, 0x4080_0000, 0x7f00_0000, 0x0100_0000] {
        for m in [0x0000_0001_u32, 0x0040_0000, 0x007f_ffff, 0x0000_0003] {
            v.push(e | m);
            v.push(e | m | 0x8000_0000);
        }
    }
    v
}

/// xorshift32: a generator, not a model — it only has to cover the input space the same way on every
/// host, which rules out anything that reads a host clock.
struct Rng(u32);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    /// A pattern with a given exponent and a random mantissa and sign: the awkward inputs for the
    /// adder are two operands a few exponents apart, which uniform random patterns almost never are.
    fn near(&mut self, exp: i32) -> u32 {
        let e = exp.clamp(0, 255) as u32;
        (self.next() & 0x8000_0000) | (e << 23) | (self.next() & 0x007f_ffff)
    }
}

#[test]
fn add_sub_and_mul_match_an_integer_model() {
    let mut rng = Rng(0x1234_5678);
    let mut pairs: Vec<(u32, u32)> = Vec::new();
    for &a in &edge_patterns() {
        for &b in &edge_patterns() {
            pairs.push((a, b));
        }
    }
    for _ in 0..40_000 {
        pairs.push((rng.next(), rng.next())); // anywhere at all, infinities and NaNs included
        let e = (rng.next() % 256) as i32;
        let d = (rng.next() % 55) as i32 - 27; // exponent differences either side of the 25 cliff
        pairs.push((rng.near(e), rng.near(e + d)));
    }
    let mut inexact = 0;
    for (a, b) in pairs {
        let (x, y) = (f32::from_bits(a), f32::from_bits(b));
        for (name, got, want) in [
            ("add", fpu::add(x, y).to_bits(), int_model::add(a, b)),
            ("sub", fpu::sub(x, y).to_bits(), int_model::sub(a, b)),
            ("mul", fpu::mul(x, y).to_bits(), int_model::mul(a, b)),
        ] {
            assert_eq!(got, want, "{name}({a:#010x}, {b:#010x}): {got:#010x} vs {want:#010x}");
        }
        // the chop and the alignment mask must actually be exercised, or this proves nothing
        if fpu::mul(x, y).to_bits() != (x as f64 * y as f64) as f32 as u32 {
            inexact += 1;
        }
    }
    assert!(inexact > 1000, "only {inexact} inputs rounded at all — the model is not being exercised");
}

#[test]
fn the_smaller_addend_keeps_exactly_one_guard_bit() {
    let a = f32::from_bits(0x4000_0000); // 2.0, exponent 128
    for d in 1..=26u32 {
        // the addend's whole mantissa is ones, so every bit the adder keeps shows up in the result
        let b = f32::from_bits((128 - d) << 23 | 0x007f_ffff);
        let kept = if d >= 25 {
            f32::from_bits(b.to_bits() & 0x8000_0000) // nothing but the sign survives
        } else {
            f32::from_bits(b.to_bits() & !((1u32 << (d - 1)) - 1))
        };
        // the bits below the guard bit are gone before the add, not rounded into it
        assert_eq!(fpu::add(a, b), fpu::add(a, kept), "add, d={d}");
        assert_eq!(fpu::add(b, a), fpu::add(kept, a), "add the other way round, d={d}");
        assert_eq!(fpu::sub(a, b), fpu::sub(a, kept), "sub, d={d}");
        assert_eq!(fpu::sub(b, a), fpu::sub(kept, a), "sub the other way round, d={d}");

        // and the guard bit itself is not: an addend of nothing but its leading bit still moves the
        // result, right up to d = 24, because a subtraction shifts it back up into the mantissa
        let lead = f32::from_bits((128 - d) << 23);
        if d <= 24 {
            assert_ne!(fpu::sub(a, lead), a, "d={d}: the guard bit does reach the result");
        } else {
            assert_eq!(fpu::sub(a, lead), a, "d={d}: past the cliff the operand is gone");
            assert_eq!(fpu::sub(a, b), a, "d={d}: whatever its mantissa");
            assert_eq!(fpu::add(a, b), a, "d={d}");
        }
    }
}

/// The square root of a non-negative `x` truncated toward zero. The host's root is correctly rounded,
/// so the answer is it or one ulp inward, and `r·r` is a 24×24-bit product — exact in `f64`. So the two
/// inequalities that define a truncated root are decided here with nothing rounded at all.
fn truncated_root(x: f32) -> f32 {
    let (sq, up) = (|r: f32| (r as f64) * (r as f64), |r: f32| f32::from_bits(r.to_bits() + 1));
    let mut r = x.sqrt();
    while sq(r) > x as f64 {
        r = f32::from_bits(r.to_bits() - 1);
    }
    while sq(up(r)) <= x as f64 {
        r = up(r);
    }
    r
}

/// `|a| / b` truncated toward zero, for a positive `b` and a quotient inside the normal range (where
/// neither the saturation nor the flush applies). `q·b` is exact in `f64` for the same reason.
fn truncated_quotient(a: f32, b: f32) -> f32 {
    let (a, prod) = (a.abs() as f64, |q: f32| (q as f64) * (b as f64));
    let up = |q: f32| f32::from_bits(q.to_bits() + 1);
    let mut q = (a as f32) / b;
    while prod(q) > a {
        q = f32::from_bits(q.to_bits() - 1);
    }
    while prod(up(q)) <= a {
        q = up(q);
    }
    q
}

#[test]
fn rsqrt_chops_both_of_its_steps() {
    // Where it shows. Both steps truncate, so the result can be one ulp either side of the
    // nearest-rounding answer: the chopped divide takes it down, the chopped root (a smaller divisor)
    // takes it up.
    let b = f32::from_bits(0x3f80_3dde);
    assert_eq!(fpu::rsqrt(1.0, b).to_bits(), 0x3f7f_c238, "the divide chops");
    assert_eq!(fpu::div(1.0, fpu::sqrt(b)).to_bits(), 0x3f7f_c239, "where nearest rounding rounds up");
    let b = f32::from_bits(0x3f80_1eef);
    assert_eq!(fpu::rsqrt(1.0, b).to_bits(), 0x3f7f_e117, "the root chops too, which lifts the quotient");
    assert_eq!(fpu::div(1.0, fpu::sqrt(b)).to_bits(), 0x3f7f_e116);
    assert_eq!(fpu::rsqrt(1.0, 4.0), 0.5, "an exact case is exact either way");

    let mut rng = Rng(0x9e37_79b9);
    let (mut tested, mut differ) = (0, 0);
    for _ in 0..40_000 {
        let e = (rng.next() % 254 + 1) as i32;
        let (a, b) = (f32::from_bits(rng.near(e)), f32::from_bits(rng.near(e) & 0x7fff_ffff));
        let want = truncated_quotient(a, truncated_root(b));
        if !want.is_normal() {
            continue; // the saturation and the flush have their own tests
        }
        let want = if a.is_sign_negative() { -want } else { want };
        assert_eq!(fpu::rsqrt(a, b).to_bits(), want.to_bits(), "rsqrt({a:e}, {b:e})");
        tested += 1;
        differ += (fpu::rsqrt(a, b).to_bits() != fpu::div(a, fpu::sqrt(b)).to_bits()) as u32;
    }
    assert!(tested > 10_000, "only {tested} inputs landed in the normal range");
    assert!(differ * 4 > tested, "only {differ} of {tested} differed from rounding to nearest");

    // the divisor's sign cannot reach the result: it is taken absolutely before the zero check, so a
    // zero divisor gives ±max with the sign of the dividend alone (where div.s xors the two signs)
    assert_eq!(fpu::rsqrt(1.0, -0.0), fpu::MAX);
    assert_eq!(fpu::rsqrt(-1.0, 0.0), -fpu::MAX);
    assert_eq!(fpu::rsqrt(-1.0, -0.0), -fpu::MAX);
    assert_eq!(fpu::div(1.0, -0.0), -fpu::MAX, "div.s is the other way");
    assert_eq!(fpu::rsqrt(3.0, -4.0), fpu::rsqrt(3.0, 4.0), "a negative divisor is its magnitude");
    // a denormal divisor is a zero divisor
    assert_eq!(fpu::rsqrt(-2.0, f32::from_bits(0x0040_0000)), -fpu::MAX);
}

#[test]
fn min_and_max_compare_the_bit_patterns() {
    // +0 wins over -0 in max.s whichever way round the operands come, and loses in min.s: the
    // reference compares the patterns as signed integers rather than as floats
    assert_eq!(fpu::max(-0.0, 0.0).to_bits(), 0);
    assert_eq!(fpu::max(0.0, -0.0).to_bits(), 0);
    assert_eq!(fpu::min(0.0, -0.0).to_bits(), 0x8000_0000);
    assert_eq!(fpu::min(-0.0, 0.0).to_bits(), 0x8000_0000);

    // and the operands are read as the FPU reads any operand
    assert_eq!(fpu::max(f32::from_bits(0x0040_0000), -1.0).to_bits(), 0, "a denormal is a zero");
    assert_eq!(fpu::max(f32::INFINITY, 1.0), fpu::MAX);
    assert_eq!(fpu::min(f32::NEG_INFINITY, 1.0), -fpu::MAX);
    assert_eq!(fpu::min(f32::NAN, -2.0), -2.0);

    let mut rng = Rng(0x0bad_c0de);
    for _ in 0..50_000 {
        let (a, b) = (fpu::operand(rng.next()), fpu::operand(rng.next()));
        if a == 0.0 && b == 0.0 {
            continue; // the signed-zero cases are the assertions above
        }
        assert_eq!(fpu::max(a, b), if a > b { a } else { b }, "max({a:e}, {b:e})");
        assert_eq!(fpu::min(a, b), if a < b { a } else { b }, "min({a:e}, {b:e})");
    }
}

#[test]
fn cvt_w_s_turns_over_at_two_to_the_31() {
    // the largest float below 2^31 converts; 2^31 itself and everything above saturates by sign
    let near = f32::from_bits(0x4eff_ffff);
    assert_eq!(fpu::to_i32(near), 2_147_483_520);
    assert_eq!(fpu::to_i32(-near), -2_147_483_520);
    assert_eq!(fpu::to_i32(f32::from_bits(0x4f00_0000)), i32::MAX);
    assert_eq!(fpu::to_i32(f32::from_bits(0xcf00_0000)), i32::MIN);
    assert_eq!(fpu::to_i32(f32::INFINITY), i32::MAX, "there is nothing else it could give");
    assert_eq!(fpu::to_i32(f32::from_bits(0x0040_0000)), 0, "a denormal is a zero");
    // the round trip is a truncation in both directions, never a rounding
    for i in 0..20_000i32 {
        // below 2^24 every integer is a float, so the round trip is exact in both directions
        let v = i * 838;
        assert_eq!(fpu::to_i32(fpu::from_i32(v)), v, "{v} does not survive the round trip");
        assert_eq!(fpu::to_i32(fpu::from_i32(-v)), -v);
        // above it, cvt.s.w drops the low bits toward zero rather than to nearest
        let big = v | 0x4000_0001;
        assert!(fpu::from_i32(big) <= big as f32, "cvt.s.w rounded away from zero at {big}");
        assert!(fpu::from_i32(-big) >= -(big as f32));
    }
}

#[test]
fn div_and_sqrt_round_to_nearest_over_the_whole_range() {
    // IEEE 754 requires `/` and `sqrt` to be correctly rounded, so the host is an independent oracle
    // for the two operations the EE rounds to nearest — on every platform, by specification.
    let mut rng = Rng(0xfeed_face);
    let (mut saturated, mut flushed) = (0, 0);
    for _ in 0..60_000 {
        let (a, b) = (fpu::operand(rng.next()), fpu::operand(rng.next()));
        let want = if b == 0.0 {
            // x/0 is ±max with the two signs xored, and 0/0 is no different
            if a.is_sign_negative() != b.is_sign_negative() { -fpu::MAX } else { fpu::MAX }
        } else {
            let q = a / b;
            if !q.is_finite() {
                saturated += 1;
                if q.is_sign_negative() { -fpu::MAX } else { fpu::MAX }
            } else if q != 0.0 && !q.is_normal() {
                flushed += 1;
                if q.is_sign_negative() { -0.0 } else { 0.0 }
            } else {
                q
            }
        };
        assert_eq!(fpu::div(a, b).to_bits(), want.to_bits(), "div({a:e}, {b:e})");
        // sqrt.s takes the magnitude, and a normal operand's root is always normal
        assert_eq!(fpu::sqrt(a).to_bits(), a.abs().sqrt().to_bits(), "sqrt({a:e})");
    }
    assert!(saturated > 100 && flushed > 100, "{saturated} saturated, {flushed} flushed — too few");
    assert_eq!(fpu::sqrt(-0.0).to_bits(), 0, "the sign goes before the root, so -0 roots to +0");
    assert_eq!(fpu::div(f32::MAX, f32::from_bits(0x0040_0000)), fpu::MAX, "a denormal divisor is zero");
}
