//! The transcendental functions a PS2 game computes **itself**, in software, on the EE FPU.
//!
//! `sinf`, `powf`, `atan2f` and friends are not instructions. They are C functions that the game's own
//! libm compiled into the executable, and on the PS2 that libm is almost always **fdlibm** (Sun's
//! freely-distributable libm) in its `float` flavour, as shipped with the Sony SDK's `libm.a` and with
//! every newlib/glibc derivative. So the game's `sinf(x)` is a specific sequence of range reductions and
//! polynomial evaluations, each step rounded by the EE FPU's chopping arithmetic — a value that differs
//! from a correctly-rounded `sin` by a few ulps, in a pattern that is perfectly reproducible and nothing
//! like what your host's libm produces.
//!
//! # Never call the host's `std` floating-point functions from a port
//!
//! This is the single easiest way to rot a bit-exact port, because it *works on your machine*:
//!
//! * `f32::sin`, `f32::powf`, `f32::exp`, `f32::atan2` and the rest are **not** specified to any
//!   accuracy by Rust. They call the platform's libm. glibc, musl, macOS's libm, Windows' CRT and the
//!   wasm32 shim all give different last bits, and glibc's results have changed between versions.
//! * So a replay test that passes on the machine it was recorded on fails on CI, on an arm64 laptop, in
//!   a browser build, or after a `glibc` update — and it fails *in the physics*, thousands of frames in,
//!   with no clue pointing back at `sin`.
//!
//! A port must use this module, or a pinned soft-float library (musl's `libm` crate) where this module
//! has no equivalent, and never `std`. Everything here is pure Rust, integer ops plus the four basic
//! IEEE operations, with no `#[cfg]` anywhere, so it gives identical bits on x86_64, aarch64 and
//! wasm32. (`sqrt` is the one exception that is safe either way: IEEE specifies it exactly. It is
//! implemented here anyway, by the bit-by-bit algorithm, so this crate depends on no host float library
//! at all.)
//!
//! # Verify against the game, do not trust this file
//!
//! fdlibm exists in several vintages and every toolchain rounds its decimal constants its own way. In the
//! project this kit was generalized from, *every* polynomial constant in the game's `acosf` and `powf`
//! sat exactly one ulp **below** the value here, because that compiler converted decimal literals toward
//! zero instead of to nearest; its `powf` also used an older `t_h` seed. Those are last-bit differences
//! that a bit-exact replay test will find three hours into a match and not before.
//!
//! So: run these against the game's own code before relying on them —
//!
//! ```text
//! ./kit oracle sinf --floats 20284.223      # one value, by hand
//! cargo test -p ps2-oracle libm             # every exponent, bit for bit
//! ```
//!
//! and when a function disagrees, fix *this* file to match the game (the constants are written as hex bit
//! patterns precisely so that a one-ulp change is a one-character change) and say in a comment which game
//! and which SDK it was checked against.
//!
//! # What is here
//!
//! `sinf cosf tanf asinf acosf atanf atan2f expf logf powf sqrtf fmodf`, all on [`crate::F`]
//! arithmetic.
//!
//! Six of them have been proved against this disc over every exponent an `f32` has, and they are the
//! six the disc links: `sinf`, `cosf`, `atanf`, `powf`, `fmodf` and `sqrtf`. The rest — `tanf`,
//! `asinf`, `acosf`, `atan2f`, `expf`, `logf` — are not in the image at all, so nothing here proves
//! them for *this* game; they are fdlibm as this disc's compiler would have rounded it (the constants
//! are the toward-zero ones, which is what every verified function in the image uses), and the first
//! game that does link them needs the oracle run again.
//!
//! One thing the disc's libm does differently from fdlibm's source throughout: **a denormal is zero**.
//! `powf` returns 1 for `|y| < 2^-126`, `fmodf` divides by zero for such a divisor, `powf`'s underflow
//! limit is `2^-126` rather than `2^-150`, and `scalbnf` has no gradual-underflow path. That is the SDK
//! adapting fdlibm to an FPU with no denormals, and it changes results well inside the ordinary range.
//! Argument reduction for `sin`/`cos`/`tan` is fdlibm's own: the three-step subtraction of pieces of
//! `π/2` up to `2^7·π/2 ≈ 201`, and above that `__kernel_rem_pio2f`, the 8-bit fixed-point product with
//! `2/π`. Both are ported from this disc's code rather than from fdlibm's source, which matters: a
//! reduction that is *more* accurate than `__kernel_rem_pio2f` still disagrees with the game in the last
//! bit, and does so for about one angle in a thousand above 201.
#![deny(clippy::float_arithmetic)]

use crate::fpu::{self, F};

/// A constant from its bit pattern: no decimal-literal rounding to argue about, and a one-ulp fix to
/// match a particular game's libm is a one-character edit.
const fn b(bits: u32) -> f32 {
    f32::from_bits(bits)
}

// --- sqrtf -----------------------------------------------------------------------------------------

/// `sqrtf(x)`, by fdlibm's bit-by-bit algorithm: integer arithmetic only, correctly rounded, and
/// therefore identical on every host and to the EE's own `sqrt.s`.
///
/// The sign is ignored, as `sqrt.s` ignores it — which is what a PS2 C compiler emits for `sqrtf()`, so
/// `sqrtf(-4.0)` is `2.0` here and not a NaN (the EE has no NaN to return).
pub fn sqrtf(x: f32) -> f32 {
    let bits = fpu::operand(x.to_bits()).to_bits();
    if bits & 0x7fff_ffff == 0 {
        // ±0, and the sign goes with it: `sqrt.s` takes the magnitude of its operand before it does
        // anything else, so it answers +0 for -0 where IEEE 754 answers -0. Proved against the disc's
        // own one-instruction `sqrtf` over every exponent (crates/ps2-oracle/tests/libm.rs).
        return 0.0;
    }
    // x = 1.m · 2^e, normalised (denormals were flushed above)
    let mut ix = (bits & 0x7fff_ffff) as i64;
    let mut m = (ix >> 23) - 127;
    ix = (ix & 0x007f_ffff) | 0x0080_0000;
    if m & 1 != 0 {
        ix += ix; // odd exponent: halve it by doubling the mantissa
    }
    m >>= 1;
    ix += ix;
    // one quotient bit per round, most significant first; s is twice the root so far
    let (mut q, mut s, mut r) = (0i64, 0i64, 0x0100_0000i64);
    while r != 0 {
        let t = s + r;
        if t <= ix {
            s = t + r;
            ix -= t;
            q += r;
        }
        ix += ix;
        r >>= 1;
    }
    if ix != 0 {
        q += q & 1; // inexact: round to nearest (q's low bit is the guard bit)
    }
    f32::from_bits((((q >> 1) + 0x3f00_0000) + (m << 23)) as u32)
}

// --- sin / cos / tan -------------------------------------------------------------------------------

/// `__kernel_sinf(x, y, iy)`: sin on `|x| <= π/4`, `y` the tail of a reduced argument.
fn k_sin(x: f32, y: f32, iy: bool) -> F {
    if x.to_bits() & 0x7fff_ffff <= 0x31ff_ffff && fpu::to_i32(x) == 0 {
        return F(x); // |x| < 2^-27: sin(x) = x
    }
    let (x, y) = (F(x), F(y));
    let z = x * x;
    let v = z * x;
    let mut r = z * b(0x2f2e_c9d2) + b(0xb2d7_2f34);
    r = z * r + b(0x3638_ef1a);
    r = z * r + b(0xb950_0d01);
    r = z * r + b(0x3c08_8889);
    if !iy {
        x + v * (z * r + b(0xbe2a_aaaa))
    } else {
        x - (z * (y * 0.5 - v * r) - y - v * b(0xbe2a_aaaa))
    }
}

/// `__kernel_cosf(x, y)`: cos on `|x| <= π/4`.
fn k_cos(x: f32, y: f32) -> F {
    let ix = x.to_bits() & 0x7fff_ffff;
    if ix <= 0x31ff_ffff && fpu::to_i32(x) == 0 {
        return F::ONE;
    }
    let (x, y) = (F(x), F(y));
    let z = x * x;
    let mut r = z * b(0xad47_d74e) + b(0x310f_74f5);
    r = z * r + b(0xb493_f27b);
    r = z * r + b(0x37d0_0d00);
    r = z * r + b(0xbab6_0b60);
    r = z * r + b(0x3d2a_aaaa);
    let zr = z * r;
    if ix <= 0x3e99_9999 {
        1.0 - (z * 0.5 - (z * zr - x * y))
    } else {
        // |x| > 0.3: split 1 - z/2 so the leading terms cancel exactly
        let qx = if ix > 0x3f48_0000 { b(0x3e90_0000) } else { b(ix - 0x0100_0000) };
        F::ONE - qx - (z * 0.5 - qx - (z * zr - x * y))
    }
}

const TAN_T: [u32; 13] = [
    0x3eaaaaab, 0x3e088889, 0x3d5d0dd1, 0x3cb327a3, 0x3c11371e, 0x3b6b6916, 0x3abede47, 0x3a1a26c7, 0x398137b8,
    0x38a3f445, 0x3895c07a, 0xb79bae5e, 0x37d95384,
];

/// `__kernel_tanf(x, y, iy)`: `tan(x + y)` for `iy == 1`, `-1/tan(x + y)` for `iy == -1`.
fn k_tan(x: f32, y: f32, iy: i32) -> F {
    let hx = x.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    if ix <= 0x317f_ffff && fpu::to_i32(x) == 0 {
        return if ix | (iy + 1) == 0 {
            1.0 / F(fpu::abs(x))
        } else if iy == 1 {
            F(x)
        } else {
            -1.0 / F(x)
        };
    }
    let big = ix > 0x3f2c_a13f; // |x| >= 0.6744
    let (x, y) = if big {
        let (x, y) = if hx < 0 { (-F(x), -F(y)) } else { (F(x), F(y)) };
        (b(0x3f49_0fda) - x + (b(0x3322_2167) - y), F::ZERO) // π/4 - x, in two pieces
    } else {
        (F(x), F(y))
    };
    let t = TAN_T.map(b);
    let z = x * x;
    let w = z * z;
    let s = z * x;
    let r = t[1] + w * (t[3] + w * (t[5] + w * (t[7] + w * (t[9] + w * t[11]))));
    let v = z * (t[2] + w * (t[4] + w * (t[6] + w * (t[8] + w * (t[10] + w * t[12])))));
    let r = y + z * (s * (r + v) + y) + t[0] * s;
    let w = x + r;
    if big {
        let v = F(fpu::from_i32(iy));
        let sign = F(fpu::from_i32(1 - ((hx >> 30) & 2)));
        return sign * (v - {
            let d = x - (w * w / (w + v) - r);
            d + d
        });
    }
    if iy == 1 {
        return w;
    }
    // -1/w, refined from the high half of w
    let hi = |a: F| F(b(a.0.to_bits() & 0xffff_f000));
    let z = hi(w);
    let v = r - (z - x);
    let a = -1.0 / w;
    let t = hi(a);
    t + a * (t * z + 1.0 + t * v)
}

/// The high words of `n·π/2` for `n` in 1..32, used to spot a cancelling reduction.
const NPIO2_HW: [u32; 32] = [
    0x3fc90f00, 0x40490f00, 0x4096cb00, 0x40c90f00, 0x40fb5300, 0x4116cb00, 0x412fed00, 0x41490f00, 0x41623100,
    0x417b5300, 0x418a3a00, 0x4196cb00, 0x41a35c00, 0x41afed00, 0x41bc7e00, 0x41c90f00, 0x41d5a000, 0x41e23100,
    0x41eec200, 0x41fb5300, 0x4203f200, 0x420a3a00, 0x42108300, 0x4216cb00, 0x421d1400, 0x42235c00, 0x4229a500,
    0x422fed00, 0x42363600, 0x423c7e00, 0x4242c700, 0x42490f00,
];

/// `__ieee754_rem_pio2f`: `x = n·π/2 + (y0 + y1)` with `|y0 + y1| <= π/4`. Returns `(n, y0, y1)`.
///
/// Three progressively longer pieces of π/2 (24 bits each) are subtracted, one more each time the
/// previous step cancelled too much, which is exact up to `|x| <= 2^7·π/2`. Larger arguments go to
/// [`rem_pio2_large`].
fn rem_pio2(x: f32) -> (i32, f32, f32) {
    let hx = x.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    let (p1, p1t, p2, p2t, p3, p3t) =
        (b(0x3fc9_0f80), b(0x3735_4442), b(0x3735_43ff), b(0x2e85_a308), b(0x2e85_a300), b(0x248d_3131));
    if ix < 0x4016_cbe4 {
        // |x| < 3π/4: n = ±1
        let s = if hx > 0 { F::ONE } else { F(-1.0) };
        let mut z = x + -s * p1;
        let (y0, y1) = if ix & !0xf != 0x3fc9_0fd0 {
            let y0 = z + -s * p1t;
            (y0, z - y0 + -s * p1t)
        } else {
            // within an ulp or two of π/2: 24+24+24 bits of it
            z += -s * p2;
            let y0 = z + -s * p2t;
            (y0, z - y0 + -s * p2t)
        };
        return (if hx > 0 { 1 } else { -1 }, y0.0, y1.0);
    }
    if ix > 0x4349_0f80 {
        return rem_pio2_large(x);
    }
    let t = F(fpu::abs(x));
    let n = fpu::to_i32((t * b(0x3f22_f984) + 0.5).0); // round(|x|·2/π)
    let fln = F(fpu::from_i32(n));
    let mut r = t - fln * p1;
    let mut w = fln * p1t;
    let mut y0 = r - w;
    if !(n < 32 && (ix as u32 & 0xffff_ff00) != NPIO2_HW[n as usize - 1]) {
        let j = ix >> 23;
        let i = j - ((y0.0.to_bits() >> 23) & 0xff) as i32;
        if i > 8 {
            // the first step cancelled: redo it with 48 bits of π/2
            let t0 = r;
            w = fln * p2;
            r = t0 - w;
            w = fln * p2t - (t0 - r - w);
            y0 = r - w;
            let i = j - ((y0.0.to_bits() >> 23) & 0xff) as i32;
            if i > 25 {
                // and again with 72 bits, which covers every remaining case
                let t0 = r;
                w = fln * p3;
                r = t0 - w;
                w = fln * p3t - (t0 - r - w);
                y0 = r - w;
            }
        }
    }
    let y1 = r - y0 - w;
    if hx < 0 { (-n, (-y0).0, (-y1).0) } else { (n, y0.0, y1.0) }
}

/// `2/π` as 198 eight-bit digits, most significant first: `2/π = Σ W[i]·2^(-8(i+1))`. 1584 bits, which
/// is what the worst cancellation an `f32` can produce needs.
const TWO_OVER_PI: [i32; 198] = [
    0xa2, 0xf9, 0x83, 0x6e, 0x4e, 0x44, 0x15, 0x29, 0xfc, 0x27, 0x57, 0xd1, 0xf5, 0x34, 0xdd, 0xc0, 0xdb, 0x62,
    0x95, 0x99, 0x3c, 0x43, 0x90, 0x41, 0xfe, 0x51, 0x63, 0xab, 0xde, 0xbb, 0xc5, 0x61, 0xb7, 0x24, 0x6e, 0x3a,
    0x42, 0x4d, 0xd2, 0xe0, 0x06, 0x49, 0x2e, 0xea, 0x09, 0xd1, 0x92, 0x1c, 0xfe, 0x1d, 0xeb, 0x1c, 0xb1, 0x29,
    0xa7, 0x3e, 0xe8, 0x82, 0x35, 0xf5, 0x2e, 0xbb, 0x44, 0x84, 0xe9, 0x9c, 0x70, 0x26, 0xb4, 0x5f, 0x7e, 0x41,
    0x39, 0x91, 0xd6, 0x39, 0x83, 0x53, 0x39, 0xf4, 0x9c, 0x84, 0x5f, 0x8b, 0xbd, 0xf9, 0x28, 0x3b, 0x1f, 0xf8,
    0x97, 0xff, 0xde, 0x05, 0x98, 0x0f, 0xef, 0x2f, 0x11, 0x8b, 0x5a, 0x0a, 0x6d, 0x1f, 0x6d, 0x36, 0x7e, 0xcf,
    0x27, 0xcb, 0x09, 0xb7, 0x4f, 0x46, 0x3f, 0x66, 0x9e, 0x5f, 0xea, 0x2d, 0x75, 0x27, 0xba, 0xc7, 0xeb, 0xe5,
    0xf1, 0x7b, 0x3d, 0x07, 0x39, 0xf7, 0x8a, 0x52, 0x92, 0xea, 0x6b, 0xfb, 0x5f, 0xb1, 0x1f, 0x8d, 0x5d, 0x08,
    0x56, 0x03, 0x30, 0x46, 0xfc, 0x7b, 0x6b, 0xab, 0xf0, 0xcf, 0xbc, 0x20, 0x9a, 0xf4, 0x36, 0x1d, 0xa9, 0xe3,
    0x91, 0x61, 0x5e, 0xe6, 0x1b, 0x08, 0x65, 0x99, 0x85, 0x5f, 0x14, 0xa0, 0x68, 0x40, 0x8d, 0xff, 0xd8, 0x80,
    0x4d, 0x73, 0x27, 0x31, 0x06, 0x06, 0x15, 0x56, 0xca, 0x73, 0xa8, 0xc9, 0x60, 0xe2, 0x7b, 0xc0, 0x8c, 0x6b,
];

/// The eleven pieces of `π/2` the kernel reassembles its answer from, 8 bits of exponent apart.
///
/// Six of the eleven are one ulp **below** the value fdlibm's source names, because the compiler that
/// built this disc converted decimal literals toward zero. These are the image's own words, read from
/// the table the kernel indexes.
const PIO2_PIECES: [u32; 11] = [
    0x3fc9_0000, 0x39f0_0000, 0x37d9_ffff, 0x33a2_0000, 0x2e83_ffff, 0x2b4f_ffff, 0x27c1_ffff,
    0x22cf_ffff, 0x1fc4_0000, 0x1bc5_ffff, 0x1744_0000,
];

/// `init_jk[prec]`: how many 8-bit digits of `2/π` the first pass multiplies by.
const INIT_JK: [i32; 3] = [4, 7, 9];

const TWO8: f32 = 256.0;
const TWON8: f32 = 0.003_906_25; // 2^-8, exact

/// `floorf`, fdlibm's: mask the fraction bits off, and add `1e30` to the argument first so the hardware
/// raises inexact. That add is kept because it is a real branch — `x + 1e30 > 0` is false for `x` around
/// `-1e30`, and then `floorf` returns its argument untouched.
fn floorf(x: f32) -> f32 {
    let bits = x.to_bits();
    let j = ((fpu::abs(x).to_bits() >> 23) as i32) - 0x7f; // the exponent of |x|
    if j >= 23 {
        return x; // already whole (or ±max)
    }
    if j < 0 {
        // |x| < 1: ±0, or -1 for a negative normal
        if fpu::add(x, HUGE) > 0.0 {
            if (bits as i32) >= 0 {
                return 0.0;
            } else if fpu::abs(x).to_bits() > 0x007f_ffff {
                return -1.0;
            }
        }
        return x;
    }
    let frac = 0x007f_ffff >> j;
    if bits & frac != 0 && fpu::add(x, HUGE) > 0.0 {
        // round toward -inf: a negative number carries into the integer part first
        let bits = if (bits as i32) < 0 { bits.wrapping_add(0x0080_0000 >> j) } else { bits };
        return f32::from_bits(bits & !frac);
    }
    x
}

/// `copysignf`.
fn copysignf(x: f32, y: f32) -> f32 {
    f32::from_bits((x.to_bits() & 0x7fff_ffff) | (y.to_bits() & 0x8000_0000))
}

/// `scalbnf(x, n)`: `x·2^n` by editing the exponent field.
///
/// The disc's version is shorter than the one in fdlibm's source: it has no gradual-underflow path, so a
/// result below the smallest normal saturates to `±0` through `copysign(1e-30, x)·1e-30` instead of
/// coming back as a denormal, and a denormal or zero argument is returned untouched.
fn scalbnf(x: f32, n: i32) -> f32 {
    let ax = fpu::abs(x).to_bits();
    if ax <= 0x007f_ffff {
        return x; // zero or denormal: nothing to scale
    }
    let k = ((ax >> 23) as i32) + n;
    let s = if k >= 0x100 {
        HUGE
    } else if k > 0 {
        return f32::from_bits((x.to_bits() & 0x807f_ffff) | ((k as u32) << 23));
    } else if n > 50000 {
        HUGE // the 50000 guards against n + k having overflowed
    } else {
        TINY
    };
    fpu::mul(copysignf(s, x), s)
}

/// `__kernel_rem_pio2f(x, y, e0, nx, prec, 2/π)`: the exact argument reduction, in fixed point.
///
/// `x` is `|arg|` split into `nx` eight-bit digits, each held as a whole float, with `arg = Σ x[i]·2^(e0
/// - 8i)`. The product with `2/π` is accumulated digit by digit — every partial product is an exact
/// float, which is the point of the 8-bit split — the integer part gives the quadrant, and the fraction
/// is multiplied back up by [`PIO2_PIECES`] to make `y0 + y1`. Returns `(n mod 8, y0, y1)`.
///
/// `prec` is 2 for `sinf`/`cosf`/`tanf`, which is the only value the disc ever passes.
fn kernel_rem_pio2(x: &[f32], e0: i32, nx: i32, prec: usize) -> (i32, f32, f32) {
    let (jk, jp) = (INIT_JK[prec], INIT_JK[prec]);
    let jx = nx - 1;
    let jv = ((e0 - 3) / 8).max(0);
    let mut q0 = e0 - 8 * (jv + 1);
    let digit = |j: i32| F(fpu::from_i32(TWO_OVER_PI[j as usize]));

    // f[] = the digits of 2/pi this argument's exponent lines up with, zero-padded on the left
    let mut f = [F::ZERO; 20];
    let mut j = jv - jx;
    for i in 0..=jx + jk {
        f[i as usize] = if j < 0 { F::ZERO } else { digit(j) };
        j += 1;
    }
    // q[] = the convolution x * f, one 8-bit column at a time
    let mut q = [F::ZERO; 20];
    let column = |f: &[F; 20], i: i32| {
        let mut fw = F::ZERO;
        for j in 0..=jx {
            fw = fw + F(x[j as usize]) * f[(jx + i - j) as usize];
        }
        fw
    };
    for i in 0..=jk {
        q[i as usize] = column(&f, i);
    }

    let mut jz = jk;
    let mut iq = [0i32; 20];
    let (mut z, mut n, mut ih);
    loop {
        // distill q[] into the 8-bit digits iq[], most significant last, carrying upward
        z = q[jz as usize];
        let mut j = jz;
        let mut i = 0;
        while j > 0 {
            let fw = F(fpu::from_i32(fpu::to_i32((z * TWON8).0)));
            iq[i] = fpu::to_i32((z - fw * TWO8).0);
            z = q[(j - 1) as usize] + fw;
            i += 1;
            j -= 1;
        }
        // the quadrant: the integer part of z, modulo 8
        z = F(scalbnf(z.0, q0));
        z = z - F(floorf((z * 0.125).0)) * 8.0;
        n = fpu::to_i32(z.0);
        z = z - F(fpu::from_i32(n));
        ih = 0;
        if q0 > 0 {
            // the top digit holds bits of the integer part too
            let i = iq[(jz - 1) as usize] >> (8 - q0);
            n += i;
            iq[(jz - 1) as usize] -= i << (8 - q0);
            ih = iq[(jz - 1) as usize] >> (7 - q0);
        } else if q0 == 0 {
            ih = iq[(jz - 1) as usize] >> 8;
        } else if z.0 >= 0.5 {
            ih = 2;
        }
        if ih > 0 {
            // the fraction is above 1/2: take 1 - q and the next quadrant, so |y| stays below pi/4
            n += 1;
            let mut carry = 0;
            for i in 0..jz as usize {
                let d = iq[i];
                if carry == 0 {
                    if d != 0 {
                        carry = 1;
                        iq[i] = 0x100 - d;
                    }
                } else {
                    iq[i] = 0xff - d;
                }
            }
            match q0 {
                1 => iq[(jz - 1) as usize] &= 0x7f,
                2 => iq[(jz - 1) as usize] &= 0x3f,
                _ => {}
            }
            if ih == 2 {
                z = F::ONE - z;
                if carry != 0 {
                    z = z - F(scalbnf(1.0, q0));
                }
            }
        }
        // everything cancelled: grind out more digits of 2/pi and start again
        if z.0 == 0.0 {
            let mut acc = 0;
            for i in jk..jz {
                acc |= iq[i as usize];
            }
            if acc == 0 {
                let mut k = 1;
                while iq[(jk - k) as usize] == 0 {
                    k += 1;
                }
                for i in jz + 1..=jz + k {
                    f[(jx + i) as usize] = digit(jv + i);
                    q[i as usize] = column(&f, i);
                }
                jz += k;
                continue;
            }
        }
        break;
    }

    // trim the digits that contribute nothing, or make one more out of what is left of z
    if z.0 == 0.0 {
        jz -= 1;
        q0 -= 8;
        while iq[jz as usize] == 0 {
            jz -= 1;
            q0 -= 8;
        }
    } else {
        z = F(scalbnf(z.0, -q0));
        if z.0 >= TWO8 {
            let fw = F(fpu::from_i32(fpu::to_i32((z * TWON8).0)));
            iq[jz as usize] = fpu::to_i32((z - fw * TWO8).0);
            jz += 1;
            q0 += 8;
            iq[jz as usize] = fpu::to_i32(fw.0);
        } else {
            iq[jz as usize] = fpu::to_i32(z.0);
        }
    }

    // the digits back to floats, each one 2^-8 of the one above it
    let mut fw = F(scalbnf(1.0, q0));
    for i in (0..=jz).rev() {
        q[i as usize] = fw * F(fpu::from_i32(iq[i as usize]));
        fw = fw * TWON8;
    }
    // and multiplied by pi/2, piece by piece
    let mut fq = [F::ZERO; 20];
    for i in (0..=jz).rev() {
        let mut fw = F::ZERO;
        let mut k = 0;
        while k <= jp && k <= jz - i {
            fw = fw + F(b(PIO2_PIECES[k as usize])) * q[(i + k) as usize];
            k += 1;
        }
        fq[(jz - i) as usize] = fw;
    }
    // prec 2: the sum, and then the sum of what the sum lost
    let mut fw = F::ZERO;
    for i in (0..=jz).rev() {
        fw = fw + fq[i as usize];
    }
    let y0 = if ih == 0 { fw } else { -fw };
    let mut tail = fq[0] - fw;
    for i in 1..=jz {
        tail = tail + fq[i as usize];
    }
    let y1 = if ih == 0 { tail } else { -tail };
    (n & 7, y0.0, y1.0)
}

/// Argument reduction for `|x| > 2^7·π/2 ≈ 201`, where subtracting pieces of `π/2` has stopped working.
///
/// `|x|` is split into three exact 8-bit digits and handed to [`kernel_rem_pio2`], which is what the
/// disc does. (An earlier version of this file reduced with its own 320-bit fixed-point product
/// instead — more bits than an `f32` can use, and still one ulp away from *this* answer often enough
/// that the oracle found it on the first sweep above 201. Being more accurate than the game is a bug.)
fn rem_pio2_large(x: f32) -> (i32, f32, f32) {
    let (hx, ix) = (x.to_bits() as i32, x.to_bits() & 0x7fff_ffff);
    let e0 = ((ix >> 23) as i32) - 134; // = ilogb(|x|) - 7, so each digit below is 8 bits
    let mut z = F(f32::from_bits(ix - ((e0 as u32) << 23)));
    let mut tx = [0.0f32; 3];
    for k in 0..2 {
        tx[k] = fpu::from_i32(fpu::to_i32(z.0));
        z = (z - F(tx[k])) * TWO8;
    }
    tx[2] = z.0;
    let mut nx = 3;
    while tx[nx - 1] == 0.0 {
        nx -= 1; // a digit of zero would only add a zero column
    }
    let (n, y0, y1) = kernel_rem_pio2(&tx[..nx], e0, nx as i32, 2);
    if hx < 0 { (-n, -y0, -y1) } else { (n, y0, y1) }
}

/// The disc's `sinf` has no Inf/NaN early-out — the SDK's libm was built for a machine with neither —
/// and it does not touch its argument with the FPU before the branch, so a denormal argument comes back
/// unchanged instead of flushed. Both of those are proved over every exponent against the disc.
pub fn sinf(x: f32) -> f32 {
    if x.to_bits() & 0x7fff_ffff <= 0x3f49_0fd8 {
        return k_sin(x, 0.0, false).0; // |x| <= π/4
    }
    let (n, y0, y1) = rem_pio2(x);
    let r = match n & 3 {
        0 => k_sin(y0, y1, true),
        1 => k_cos(y0, y1),
        2 => -k_sin(y0, y1, true),
        _ => -k_cos(y0, y1),
    };
    r.0
}

/// `cosf`.
pub fn cosf(x: f32) -> f32 {
    if x.to_bits() & 0x7fff_ffff <= 0x3f49_0fd8 {
        return k_cos(x, 0.0).0;
    }
    let (n, y0, y1) = rem_pio2(x);
    let r = match n & 3 {
        0 => k_cos(y0, y1),
        1 => -k_sin(y0, y1, true),
        2 => -k_cos(y0, y1),
        _ => k_sin(y0, y1, true),
    };
    r.0
}

/// `tanf`.
pub fn tanf(x: f32) -> f32 {
    let x = fpu::operand(x.to_bits());
    if x.to_bits() & 0x7fff_ffff <= 0x3f49_0fda {
        return k_tan(x, 0.0, 1).0;
    }
    let (n, y0, y1) = rem_pio2(x);
    k_tan(y0, y1, 1 - ((n & 1) << 1)).0
}

// --- atan / atan2 ----------------------------------------------------------------------------------

const ATAN_HI: [u32; 4] = [0x3eed6338, 0x3f490fda, 0x3f7b985d, 0x3fc90fda];
const ATAN_LO: [u32; 4] = [0x31ac3769, 0x33222167, 0x33140fb4, 0x33a22168];
const AT: [u32; 11] = [
    0x3eaaaaab, 0xbe4ccccc, 0x3e124924, 0xbde38e38, 0x3dba2e6e, 0xbd9d8795, 0x3d886b34, 0xbd6ef16a, 0x3d4bda59,
    0xbd15a221, 0x3c8569d7,
];

/// `atanf`: `atan(x) = atan(hi) + atan((x-hi)/(1+x·hi))` on four reduction intervals, then one odd
/// polynomial in `x²`.
///
/// Like `sinf`, it tests its argument with integer compares only, so a denormal below `2^-29` comes back
/// unchanged rather than flushed — proved over every exponent against the disc.
pub fn atanf(x0: f32) -> f32 {
    let hx = x0.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    if ix > 0x507f_ffff {
        // |x| > 2^66: atan(x) = ±π/2
        let (hi, lo) = (F(b(ATAN_HI[3])), b(ATAN_LO[3]));
        return if hx > 0 { (hi + lo).0 } else { (-hi - lo).0 };
    }
    let (x, id) = if ix <= 0x3edf_ffff {
        if ix <= 0x30ff_ffff && 1.0 < F(x0) + HUGE {
            return x0; // |x| < 2^-29: atan(x) = x
        }
        (F(x0), -1)
    } else {
        let x = F(fpu::abs(x0));
        if ix <= 0x3f97_ffff {
            if ix <= 0x3f2f_ffff { ((x + x - 1.0) / (x + 2.0), 0) } else { ((x - 1.0) / (x + 1.0), 1) }
        } else if ix <= 0x401b_ffff {
            ((x - 1.5) / (x * 1.5 + 1.0), 2)
        } else {
            (-1.0 / x, 3)
        }
    };
    let a = AT.map(b);
    let z = x * x;
    let w = z * z;
    let s1 = z * (a[0] + w * (a[2] + w * (a[4] + w * (a[6] + w * (a[8] + w * a[10])))));
    let s2 = w * (a[1] + w * (a[3] + w * (a[5] + w * (a[7] + w * a[9]))));
    if id < 0 {
        return (x - x * (s1 + s2)).0;
    }
    let id = id as usize;
    let z = b(ATAN_HI[id]) - (x * (s1 + s2) - b(ATAN_LO[id]) - x);
    if hx < 0 { (-z).0 } else { z.0 }
}

/// `atan2f(y, x)`. Denormal arguments count as zero, as everywhere on the EE, and the `±inf` cases
/// fdlibm handles cannot arise (an infinity pattern reads as `±max`, an ordinary number).
pub fn atan2f(y: f32, x: f32) -> f32 {
    const PI: f32 = b(0x4049_0fdb);
    const PI_O_2: f32 = b(0x3fc9_0fdb);
    const PI_LO: f32 = b(0xb3bb_bd2e); // negative: pi + pi_lo is π to ~31 bits
    let (y, x) = (fpu::operand(y.to_bits()), fpu::operand(x.to_bits()));
    let (hy, hx) = (y.to_bits() as i32, x.to_bits() as i32);
    if hx == 0x3f80_0000 {
        return atanf(y); // x = 1
    }
    let (iy, ix) = (hy & 0x7fff_ffff, hx & 0x7fff_ffff);
    let m = ((hy as u32) >> 31) | ((hx >> 30) & 2) as u32; // quadrant, from the two sign bits
    if hy == 0 {
        // y = +0 (−0 falls through and comes out of the quadrant arithmetic below)
        return match m {
            2 => (F(PI) + TINY).0,
            3 => (-F(PI) - TINY).0,
            _ => y,
        };
    }
    if hx == 0 {
        return if hy < 0 { (-F(PI_O_2) - TINY).0 } else { (F(PI_O_2) + TINY).0 };
    }
    let k = (iy - ix) >> 23;
    let z = if k > 60 {
        F(PI_O_2) + F(0.5) * PI_LO // |y/x| > 2^60
    } else if hx < 0 && k < -60 {
        F::ZERO // |y/x| < 2^-60 and x < 0
    } else {
        F(atanf(fpu::abs((F(y) / x).0)))
    };
    match m {
        0 => z.0,
        1 => f32::from_bits(z.to_bits() ^ 0x8000_0000),
        2 => (F(PI) - (z - PI_LO)).0,
        _ => ((z - PI_LO) - PI).0,
    }
}

// --- asin / acos -----------------------------------------------------------------------------------

const PIO2_HI: f32 = b(0x3fc9_0fda);
const PIO2_LO: f32 = b(0x33a2_2168);
const PIO4_HI: f32 = b(0x3f49_0fdb);
const PI: f32 = b(0x4049_0fdb);

/// fdlibm's rational approximation `p(z)/q(z)` shared by `asinf` and `acosf`, in its own term order.
fn pq(z: F) -> F {
    let p = z
        * (b(0x3e2a_aaab)
            + z * (b(0xbea6_b090)
                + z * (b(0x3e4e_0aa8) + z * (b(0xbd24_1146) + z * (b(0x3a4f_7f04) + z * b(0x3811_ef08))))));
    let q = 1.0 + z * (b(0xc019_d139) + z * (b(0x4001_572d) + z * (b(0xbf30_3361) + z * b(0x3d9d_c62e))));
    p / q
}

/// `asinf`.
pub fn asinf(x: f32) -> f32 {
    let x0 = fpu::operand(x.to_bits());
    let hx = x0.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    if ix == 0x3f80_0000 {
        return (F(x0) * PIO2_HI + F(x0) * PIO2_LO).0; // ±π/2
    }
    if ix > 0x3f80_0000 {
        return (F::ZERO / 0.0).0; // |x| > 1: NaN on IEEE, +max on the EE
    }
    if ix < 0x3f00_0000 {
        // |x| < 0.5
        if ix < 0x3200_0000 && 1.0 < F(b(0x7149_f2c9)) + x0 {
            return x0; // |x| < 2^-27
        }
        let t = F(x0) * x0;
        let w = pq(t);
        return (F(x0) + F(x0) * w).0;
    }
    // 0.5 <= |x| < 1
    let w = 1.0 - F(fpu::abs(x0));
    let t = w * 0.5;
    let s = F(sqrtf(t.0));
    let t = if ix >= 0x3f79_999a {
        // |x| > 0.975
        let w = pq(t);
        PIO2_HI - (F(2.0) * (s + s * w) - PIO2_LO)
    } else {
        let w = F(b(s.0.to_bits() & 0xffff_f000));
        let c = (t - w * w) / (s + w);
        let r = pq(t);
        let p = F(2.0) * s * r - (PIO2_LO - F(2.0) * c);
        let q = PIO4_HI - F(2.0) * w;
        PIO4_HI - (p - q)
    };
    if hx > 0 { t.0 } else { (-t).0 }
}

/// `acosf`.
pub fn acosf(x: f32) -> f32 {
    let x0 = fpu::operand(x.to_bits());
    let hx = x0.to_bits() as i32;
    let ix = hx & 0x7fff_ffff;
    if ix == 0x3f80_0000 {
        return if hx > 0 { 0.0 } else { (PI + F(2.0) * PIO2_LO).0 };
    }
    if ix > 0x3f80_0000 {
        return (F::ZERO / 0.0).0; // |x| > 1: NaN on IEEE, +max on the EE
    }
    let x = F(x0);
    if ix < 0x3f00_0000 {
        // |x| < 0.5
        if ix <= 0x2300_0000 {
            return (F(PIO2_HI) + PIO2_LO).0; // |x| < 2^-57
        }
        let z = x * x;
        let r = pq(z);
        return (PIO2_HI - (x - (PIO2_LO - x * r))).0;
    }
    if hx < 0 {
        // x < -0.5
        let z = (1.0 + x) * 0.5;
        let s = F(sqrtf(z.0));
        let r = pq(z);
        let w = r * s - PIO2_LO;
        return (PI - F(2.0) * (s + w)).0;
    }
    // x > 0.5
    let z = (1.0 - x) * 0.5;
    let s = F(sqrtf(z.0));
    let df = F(b(s.0.to_bits() & 0xffff_f000));
    let c = (z - df * df) / (s + df);
    let r = pq(z);
    let w = r * s + c;
    (F(2.0) * (df + w)).0
}

// --- exp / log / pow -------------------------------------------------------------------------------

const LN2_HI: f32 = b(0x3f31_7180);
const LN2_LO: f32 = b(0x3717_f7d1);
// `1e30` and `1e-30`, as *this* disc holds them: both are one ulp below the nearest float to the
// decimal literal, because the compiler that built it converted toward zero. They are the same words in
// `powf`, `floorf`, `scalbnf` and `atanf`, so there is one pair here.
const HUGE: f32 = b(0x7149_f2c9); // 9.9999994e29
const TINY: f32 = b(0x0da2_425f); // 9.999999e-31
const TWOM100: f32 = b(0x0d80_0000); // 2^-100
const P1: f32 = b(0x3e2a_aaaa);
const P2: f32 = b(0xbb36_0b60);
const P3: f32 = b(0x388a_b354);
const P4: f32 = b(0xb5dd_ea0e);
const P5: f32 = b(0x3331_bb4b);

/// Scale by `2^k` by adding to the exponent field, saturating at `±max` and flushing to `±0` the way the
/// EE does (where fdlibm would produce an infinity or a denormal).
fn ldexp_bits(y: f32, k: i32) -> f32 {
    let e = ((y.to_bits() >> 23) & 0xff) as i32 + k;
    if e >= 0xff {
        return if y.is_sign_negative() { fpu::NEG_MAX } else { fpu::MAX };
    }
    if e <= 0 {
        return if y.is_sign_negative() { -0.0 } else { 0.0 };
    }
    f32::from_bits((y.to_bits() & 0x807f_ffff) | ((e as u32) << 23))
}

/// `expf`.
pub fn expf(x: f32) -> f32 {
    let x0 = fpu::operand(x.to_bits());
    let xsb = x0.to_bits() >> 31;
    let hx = x0.to_bits() & 0x7fff_ffff;
    if hx >= 0x42b1_7218 {
        // |x| >= 88.72: over/underflow (the EE saturates instead of going infinite)
        if x0 > b(0x42b1_7180) {
            return fpu::mul(HUGE, HUGE);
        }
        if x0 < b(0xc2cf_f1b5) {
            return fpu::mul(TWOM100, TWOM100);
        }
    }
    let mut x = F(x0);
    let (mut hi, mut lo) = (F::ZERO, F::ZERO);
    let mut k = 0i32;
    if hx > 0x3eb1_7218 {
        // |x| > 0.5·ln2: x = k·ln2 + r
        if hx < 0x3f85_1592 {
            // |x| < 1.5·ln2: k = ±1
            let (lh, ll) = if xsb == 0 { (F(LN2_HI), F(LN2_LO)) } else { (-F(LN2_HI), -F(LN2_LO)) };
            hi = x - lh;
            lo = ll;
            k = 1 - 2 * xsb as i32;
        } else {
            k = fpu::to_i32((F(b(0x3fb8_aa3a)) * x + if xsb == 0 { 0.5 } else { -0.5 }).0);
            let t = F(fpu::from_i32(k));
            hi = x - t * LN2_HI; // exact: t·ln2_hi has no low bits
            lo = t * LN2_LO;
        }
        x = hi - lo;
    } else if hx < 0x3180_0000 {
        // |x| < 2^-28: exp(x) = 1 + x
        if F(HUGE) + x > 1.0 {
            return (F::ONE + x).0;
        }
    }
    let t = x * x;
    let c = x - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    if k == 0 {
        return (1.0 - ((x * c) / (c - 2.0) - x)).0;
    }
    let y = 1.0 - ((lo - (x * c) / (F(2.0) - c)) - hi);
    if k >= -125 {
        ldexp_bits(y.0, k)
    } else {
        fpu::mul(ldexp_bits(y.0, k + 100), TWOM100)
    }
}

const LG: [u32; 7] =
    [0x3f2a_aaab, 0x3ecc_cccd, 0x3e92_4925, 0x3e63_8e29, 0x3e3a_3325, 0x3e1c_d04f, 0x3e17_8897];

/// `logf` (natural log). `log(0)` is `-max` and `log(x < 0)` is `+max`: the EE has neither an infinity
/// nor a NaN to return, and `0/0` on its divider is `+max`.
pub fn logf(x: f32) -> f32 {
    let x0 = fpu::operand(x.to_bits());
    let mut ix = x0.to_bits() as i32;
    if ix < 0x0080_0000 {
        if ix & 0x7fff_ffff == 0 {
            return fpu::div(b(0xcc00_0000), 0.0); // log(±0)
        }
        if ix < 0 {
            return fpu::div(fpu::sub(x0, x0), 0.0); // log(negative)
        }
    }
    let mut k = (ix >> 23) - 127;
    ix &= 0x007f_ffff;
    let i = (ix + (0x95f64 << 3)) & 0x0080_0000; // pick x or x/2 so that |f| is smallest
    let x = F(b((ix | (i ^ 0x3f80_0000)) as u32));
    k += i >> 23;
    let fx = x - 1.0;
    let dk = F(fpu::from_i32(k));
    let lg = LG.map(b);
    if (0x007f_ffff & (15 + ix)) < 16 {
        // |f| < 2^-20
        if fx == 0.0 {
            return if k == 0 { 0.0 } else { (dk * LN2_HI + dk * LN2_LO).0 };
        }
        let r = fx * fx * (F(0.5) - F(b(0x3eaa_aaab)) * fx);
        return if k == 0 { (fx - r).0 } else { (dk * LN2_HI - ((r - dk * LN2_LO) - fx)).0 };
    }
    let s = fx / (F(2.0) + fx);
    let z = s * s;
    let w = z * z;
    let i = (ix - (0x6147a << 3)) | ((0x6b851 << 3) - ix); // is |f| in the middle of the interval?
    let t1 = w * (lg[1] + w * (lg[3] + w * lg[5]));
    let t2 = z * (lg[0] + w * (lg[2] + w * (lg[4] + w * lg[6])));
    let r = t2 + t1;
    if i > 0 {
        let hfsq = F(0.5) * fx * fx;
        if k == 0 {
            (fx - (hfsq - s * (hfsq + r))).0
        } else {
            (dk * LN2_HI - ((hfsq - (s * (hfsq + r) + dk * LN2_LO)) - fx)).0
        }
    } else if k == 0 {
        (fx - s * (fx - r)).0
    } else {
        (dk * LN2_HI - ((s * (fx - r) - dk * LN2_LO) - fx)).0
    }
}

/// The 12-bit-mantissa head of a float: fdlibm's way of splitting one value into two exactly
/// multipliable halves.
fn hi12(x: F) -> F {
    F(b(x.0.to_bits() & 0xffff_f000))
}

/// `powf(x, y)`, fdlibm's: `2^(y·log2(x))` with `log2(x)` and the product each carried in two pieces so
/// the result keeps ~30 bits where a single `f32` would keep 24.
///
/// Two known variants of the `t_h` seed exist; if the oracle says your game disagrees here, try the older
/// one noted at that line.
pub fn powf(x: f32, y: f32) -> f32 {
    let (hx, hy) = (x.to_bits() as i32, y.to_bits() as i32);
    let (ix, iy) = (hx & 0x7fff_ffff, hy & 0x7fff_ffff);
    if iy < 0x0080_0000 {
        return 1.0; // x^0 = 1, for every x — and a denormal y *is* zero to this FPU
    }
    // is y an integer, and is it odd? (only matters for x < 0)
    let mut yisint = 0;
    if hx < 0 {
        if iy >= 0x4b80_0000 {
            yisint = 2; // |y| >= 2^24: certainly an even integer
        } else if iy >= 0x3f80_0000 {
            let k = (iy >> 23) - 0x7f;
            let j = iy >> (23 - k);
            if (j << (23 - k)) == iy {
                yisint = 2 - (j & 1);
            }
        }
    }
    if iy == 0x3f80_0000 {
        return if hy < 0 { fpu::div(1.0, x) } else { x }; // y = ±1
    }
    if hy == 0x4000_0000 {
        return fpu::mul(x, x); // y = 2
    }
    if hy == 0x3f00_0000 && hx >= 0 {
        return sqrtf(x); // y = 0.5
    }
    let ax = fpu::abs(x);
    if ix < 0x0080_0000 || ix == 0x3f80_0000 {
        // x = ±0 or ±1
        let mut z = ax;
        if hy < 0 {
            z = fpu::div(1.0, z);
        }
        if hx < 0 {
            if (ix - 0x3f80_0000) | yisint == 0 {
                z = fpu::div(fpu::sub(z, z), fpu::sub(z, z)); // (-1)^non-integer: 0/0, +max on the EE
            } else if yisint == 1 {
                z = fpu::neg(z);
            }
        }
        return z;
    }
    if hx < 0 && yisint == 0 {
        return fpu::div(fpu::sub(x, x), fpu::sub(x, x)); // negative base, fractional exponent
    }
    // the sign of the result: negative only for a negative base and an odd integer exponent
    let sign = if hx < 0 && yisint == 1 { -1.0f32 } else { 1.0f32 };
    let (t1, t2);
    if iy > 0x4d00_0000 {
        // |y| > 2^27: either over/underflow, or x is so close to 1 that log(x) needs only four terms
        // |y| that large makes y an even integer, so `sign` is +1 and these need no sign at all —
        // which is how the disc has them, `huge*huge` and `tiny*tiny` already folded to max and zero
        if ix < 0x3f7f_fff8 {
            return if hy >= 0 { 0.0 } else { fpu::MAX };
        }
        if ix > 0x3f80_0007 {
            return if hy <= 0 { 0.0 } else { fpu::MAX };
        }
        let t = F(x) - 1.0;
        let w = (t * t) * (F(0.5) - t * (F(b(0x3eaa_aaaa)) - t * 0.25));
        let u = F(b(0x3fb8_aa00)) * t; // ivln2_h, 16 significant bits
        let v = t * b(0x36ec_a56f) - w * b(0x3fb8_aa3a); // ivln2_l, ivln2
        let a = u + v;
        t1 = hi12(a);
        t2 = v - (t1 - u);
    } else {
        // no renormalising step: a denormal base left through the `ix < 0x0080_0000` branch above,
        // which is why the disc's `powf` has no `2^24` scaling where fdlibm's source does
        let mut ixn = ix;
        let mut n = (ixn >> 23) - 0x7f;
        let j = ixn & 0x007f_ffff;
        ixn = j | 0x3f80_0000; // ax normalised into [1, 2)
        let k = if j <= 0x1c_c471 {
            0 // |x| < sqrt(3/2)
        } else if j < 0x5d_b3d7 {
            1 // |x| < sqrt(3)
        } else {
            n += 1;
            ixn -= 0x0080_0000;
            0
        };
        let ax = F(b(ixn as u32));
        let bp = if k == 1 { 1.5f32 } else { 1.0f32 };
        // s = (ax - bp)/(ax + bp), in two pieces
        let u = ax - bp;
        let v = 1.0 / (ax + bp);
        let s = u * v;
        let s_h = hi12(s);
        // t_h ≈ ax + bp, built straight out of the exponent field: halve the exponent, set the bias
        // back, then nudge. fdlibm's float `pow` nudges by 0x0004_0000 where the scaling of the double
        // one says 0x0040_0000, and leaves the result unmasked where the double one masks to 12 bits —
        // both are in the disc's instructions, and either one changed costs an ulp in `powf`.
        let t_h = F(b((((ixn >> 1) as u32) | 0x2000_0000) + 0x0004_0000 + ((k as u32) << 21)));
        let t_l = ax - (t_h - bp);
        let s_l = v * ((u - s_h * t_h) - s_h * t_l);
        // log(ax) = 2s + 2s^3/3 + …, as (s_h + s_l)·(3 + s^2 + r)
        let s2 = s * s;
        let r = s2
            * s2
            * (b(0x3f19_9999)
                + s2 * (b(0x3edb_6db7)
                    + s2 * (b(0x3eaa_aaab) + s2 * (b(0x3e8b_a304) + s2 * (b(0x3e6c_3254) + s2 * b(0x3e53_f142))))));
        let r = r + s_l * (s_h + s);
        let s2 = s_h * s_h;
        let t_h = hi12(F(3.0) + s2 + r);
        let t_l = r - ((t_h - 3.0) - s2);
        let u = s_h * t_h;
        let v = s_l * t_h + t_l * s;
        // × 2/(3·log2) to get log2(ax)
        let p_h = hi12(u + v);
        let p_l = v - (p_h - u);
        let z_h = F(b(0x3f76_3800)) * p_h; // cp_h
        let z_l = F(b(0x369d_c39f)) * p_h + p_l * b(0x3f76_384e) + if k == 1 { b(0x35d1_cfdc) } else { 0.0 };
        let t = F(fpu::from_i32(n));
        t1 = hi12(((z_h + z_l) + if k == 1 { b(0x3f15_c000) } else { 0.0 }) + t);
        t2 = z_l - (((t1 - t) - if k == 1 { b(0x3f15_c000) } else { 0.0 }) - z_h);
    }
    // y·log2(x) = p_h + p_l, then 2^that
    let y1 = hi12(F(y));
    let p_l = (F(y) - y1) * t1 + F(y) * t2;
    let mut p_h = y1 * t1;
    let z = p_l + p_h;
    let j = z.0.to_bits() as i32;
    let i = j & 0x7fff_ffff;
    // The limits are the EE's, not IEEE's: no denormals, so anything below 2^-126 is zero and the
    // underflow test is `z <= -126` where fdlibm's source says -150. Overflow is tested at 129 and the
    // last exponent is left to saturate on its own.
    if j < 1 {
        // z is zero or negative: the answer is below 1
        if i > 0x42fc_0000 || (i == 0x42fc_0000 && (z - p_h).0 >= p_l.0) {
            return fpu::mul(fpu::mul(sign, TINY), TINY); // ±0
        }
    } else if i > 0x4301_0000 || (i == 0x4301_0000 && p_l.0 + b(0x3338_aa3b) > (z - p_h).0) {
        return fpu::mul(fpu::mul(sign, HUGE), HUGE); // ±max
    }
    let mut k = (i >> 23) - 0x7f;
    let mut n = 0i32;
    if i > 0x3f00_0000 {
        // |z| > 0.5: split off the integer part n = round(z)
        let m = j + (0x0080_0000 >> (k + 1));
        k = ((m & 0x7fff_ffff) >> 23) - 0x7f;
        let t = F(b((m & !(0x007f_ffff >> k)) as u32));
        n = ((m & 0x007f_ffff) | 0x0080_0000) >> (23 - k);
        if j < 0 {
            n = -n;
        }
        p_h -= t;
    }
    let t = hi12(p_l + p_h);
    let u = t * b(0x3f31_7200); // lg2_h
    let v = (p_l - (t - p_h)) * b(0x3f31_7217) + t * b(0x35bf_be8c); // lg2, lg2_l
    let z = u + v;
    let w = v - (z - u);
    let t = z * z;
    let t1 = z - t * (P1 + t * (P2 + t * (P3 + t * (P4 + t * P5))));
    let r = (z * t1) / (t1 - 2.0) - (w + z * w);
    let z = 1.0 - (r - z);
    // 2^n by adding to the exponent field; if that carried out of it (either way), `scalbnf` saturates
    let j = (z.0.to_bits() as i32).wrapping_add(n << 23);
    let z = if (j >> 23) < 1 { scalbnf(z.0, n) } else { b(j as u32) };
    fpu::mul(sign, z)
}

// --- fmodf -----------------------------------------------------------------------------------------

/// `fmodf(x, y)`: `x - n·y` for the whole `n` that leaves a remainder with `x`'s sign and a magnitude
/// below `|y|`.
///
/// fdlibm computes this in fixed point — shift and subtract on the mantissas, no floating-point
/// operation anywhere — so the answer is *exact*, and it is the same on any machine. The only arithmetic
/// is the `(x·y)/(x·y)` an IEEE library returns for a zero divisor; on the EE that divide is `0/0`, which
/// is [`fpu::MAX`], not a NaN.
///
/// A denormal divisor takes the same path as zero, which is right for the EE: the FPU reads denormals as
/// zero, so no code upstream can produce one, and the game's own `fmodf` tests the whole exponent field.
pub fn fmodf(x: f32, y: f32) -> f32 {
    let sx = x.to_bits() & 0x8000_0000;
    let (hx, hy) = ((x.to_bits() ^ sx) as i32, (y.to_bits() & 0x7fff_ffff) as i32);
    // y zero or denormal: 0/0 on the EE's divider
    if (hy as u32) < 0x0080_0000 {
        let p = fpu::mul(x, y);
        return fpu::div(p, p);
    }
    let zero = || f32::from_bits(sx); // ±0, keeping x's sign
    if hx < hy {
        return x; // |x| < |y|: x is already the remainder
    }
    if hx == hy {
        return zero();
    }
    // ix, iy = the unbiased exponents; the mantissas with their implicit 1, aligned to bit 23
    let (mut ix, mut iy) = ((hx >> 23) - 127, (hy >> 23) - 127);
    let mut mx = if ix < -126 { hx << (-ix - 126) } else { (hx & 0x007f_ffff) | 0x0080_0000 };
    let my = if iy < -126 { hy << (-iy - 126) } else { (hy & 0x007f_ffff) | 0x0080_0000 };
    let mut sub = iy < -126;
    // long division: ix - iy steps of shift-and-subtract, stopping early on an exact multiple
    let mut n = ix - iy;
    let mut mz;
    loop {
        n -= 1;
        mz = mx - my;
        if n == -1 {
            break;
        }
        mx <<= 1;
        if mz >= 0 {
            if mz == 0 {
                return zero();
            }
            mx = mz << 1;
        }
    }
    if mz >= 0 {
        mx = mz;
    }
    if mx == 0 {
        return zero();
    }
    // back to a float: renormalise, then reassemble
    if mx < 0x0080_0000 {
        while mx < 0x0080_0000 {
            mx <<= 1;
            iy -= 1;
        }
        sub = iy < -126;
    }
    ix = iy; // the result's exponent is the divisor's, after the shifts above
    if !sub {
        f32::from_bits(((mx - 0x0080_0000) | ((ix + 127) << 23)) as u32 | sx)
    } else {
        f32::from_bits((mx >> (-ix - 126)) as u32 | sx)
    }
}
