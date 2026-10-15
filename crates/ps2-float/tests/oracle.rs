//! The arithmetic of this crate, against the game's own code, over the inputs nobody plays.
//!
//! `tests/fpu.rs`, `tests/vu.rs` and `tests/libm.rs` check these models against what the hardware is
//! *documented* to do. This file checks them against what the disc actually contains: `ps2-oracle` loads
//! the game's code and runs it, so every case here is a real `add.s` in a real function, called with
//! arguments no recording would ever have produced.
//!
//! # What a carrier is, and why the awkward inputs are the point
//!
//! No shipped game has a function that is one `div.s` and a `jr`. What it has are *carriers*: small leaf
//! functions whose arithmetic is one or two operations wide, so that a disagreement can only be the
//! operation. They are named in `research/oracle.txt` with the fields they read and write, and they were
//! found by disassembling every call-free function in the image and looking for the ops with no coverage.
//!
//! Each carrier is then fed from a generator ([`Rng::edge`]) built out of the cases that break ports
//! rather than the cases play produces: `±0`, both ends of the denormal range, the smallest normal,
//! `±MAX`, the Inf and NaN bit patterns the EE does not have, powers of two, uniformly random 32-bit
//! patterns — and, for the adder, *pairs* with a chosen exponent difference, because the EE's one guard
//! bit makes a difference of 0, 24, 25 and 26 four different instructions (see [`ps2_float::fpu::align`]).
//! Ordinary gameplay values exercise none of that, which is exactly why a port can drift for a day of
//! play before anyone notices.
//!
//! # What this cannot prove
//!
//! The oracle's own FPU *is* [`ps2_float`] (`ps2-oracle`'s interpreter calls `fpu::add` for `add.s`), so
//! these tests cannot prove the rounding model itself: if `fpu::add` chopped in the wrong direction, both
//! sides of every assertion would be wrong together. What they do prove is everything built on top of it
//! — which operation the game used, in which order, on which operands, with which constants, and what it
//! does at the edges where the real code's own branches live. The rounding model's own proof is the
//! hardware comparison in `tests/fpu.rs`'s references and PCSX2's recompiler; this file is about the
//! port agreeing with the game, operation for operation.
//!
//! Everything skips with a message when the disc is not configured, so a fresh checkout stays green.

use ps2_float::{Acc, fpu, vu};
use ps2_oracle::{Ee, addr, game_or_skip, skip};

/// Cases per property. `AGENT.md` asks for ten thousand; several of the tests below compare three or six
/// values per call, so most of them are well past it.
const N: u32 = 10_000;

/// None of these carriers runs more than a few dozen instructions.
const BUDGET: u64 = 100_000;

/// Load the game and look up every carrier the test needs, skipping with one message if any is missing.
fn ready(names: &[&str]) -> Option<(Ee, Vec<u32>)> {
    let mut found = Vec::new();
    for name in names {
        match addr(name) {
            Ok(a) => found.push(a),
            Err(why) => {
                skip(&why);
                return None;
            }
        }
    }
    let ee = game_or_skip()?;
    Some((ee, found))
}

// --- the generator ----------------------------------------------------------------------------------

/// A tiny xorshift: every test seeds its own, so a failure is reproducible from the seed in the source.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    /// One value from the classes that break ports, chosen uniformly between them rather than between
    /// values — so a sixteenth of the cases are exactly zero, a sixteenth are denormal, and so on.
    ///
    /// Bit patterns the EE cannot represent (exponent 255, and the denormals) are generated on purpose:
    /// the hardware substitutes `±MAX` and `±0` for them when it *reads* them, and a port that does the
    /// substitution anywhere else — or not at all — disagrees here first.
    fn edge(&mut self) -> f32 {
        let sign = (self.next() & 1) << 31;
        match self.next() % 16 {
            0 => f32::from_bits(sign),                                   // ±0
            1 => f32::from_bits(sign | 1),                               // the smallest denormal
            2 => f32::from_bits(sign | 0x007f_ffff),                     // the largest denormal
            3 => f32::from_bits(sign | (self.next() & 0x007f_ffff)),     // anywhere in the denormals
            4 => f32::from_bits(sign | 0x7f7f_ffff),                     // ±MAX
            5 => f32::from_bits(sign | 0x7f7f_fffe),                     // one ulp inside it
            6 => f32::from_bits(sign | 0x0080_0000),                     // the smallest normal
            7 => f32::from_bits(sign | 0x7f80_0000),                     // ±Inf, which the EE has not got
            8 => f32::from_bits(sign | 0x7f80_0000 | (self.next() & 0x007f_ffff)), // ±NaN, likewise
            9 => f32::from_bits(sign | 0x3f80_0000),                     // ±1
            10 => f32::from_bits(sign | (((self.next() % 254) + 1) << 23)), // an exact power of two
            11 | 12 => f32::from_bits(self.next()),                      // any pattern at all
            _ => self.playable(),                                        // and the ordinary ones
        }
    }

    /// A value of the magnitude a game actually computes with: `2^e` times a full random mantissa, with
    /// `e` in the range positions, speeds and dot products live in.
    fn playable(&mut self) -> f32 {
        let sign = (self.next() & 1) << 31;
        let e = (127i32 - 24 + (self.next() % 49) as i32) as u32;
        f32::from_bits(sign | (e << 23) | (self.next() & 0x007f_ffff))
    }

    /// A value whose exponent is `d` below `a`'s, with its own sign and mantissa — the adder's whole
    /// story is in `d`, so these are generated rather than hoped for.
    ///
    /// `d` of 0 is the equal-exponent case (no alignment at all), 24 is the last one where the smaller
    /// operand still reaches the guard bit, and 25 and above is where it contributes nothing but its
    /// sign. When `a` is not a normal number there is no exponent to step down from, so the value falls
    /// back to the edge classes.
    fn exponent_away(&mut self, a: f32, d: u32) -> f32 {
        let e = (a.to_bits() >> 23) & 0xff;
        if e == 0 || e == 0xff || e <= d {
            return self.edge();
        }
        let sign = (self.next() & 1) << 31;
        f32::from_bits(sign | ((e - d) << 23) | (self.next() & 0x007f_ffff))
    }

    /// A pair for the adder: a quarter of them independent, the rest with a chosen exponent difference,
    /// and either way the larger one is as likely to be the second argument as the first.
    fn pair(&mut self) -> (f32, f32) {
        let a = self.edge();
        let b = match self.next() % 4 {
            0 => self.edge(),
            _ => {
                let d = [0, 0, 1, 2, 23, 24, 25, 26, 31, 60][(self.next() % 10) as usize];
                self.exponent_away(a, d)
            }
        };
        if self.next() & 1 == 0 { (a, b) } else { (b, a) }
    }

    fn vec3(&mut self) -> [f32; 3] {
        [self.edge(), self.edge(), self.edge()]
    }
}

// --- helpers ----------------------------------------------------------------------------------------

/// Write a quadword's worth of vector where a carrier can read it: these helpers are compiled for `lqc2`
/// as often as for `lwc1`, so the `w` lane has to hold something defined.
fn put_vec(ee: &mut Ee, at: u32, v: [f32; 3], w: f32) {
    for (k, x) in v.iter().enumerate() {
        ee.set_f32(at + 4 * k as u32, *x);
    }
    ee.set_f32(at + 12, w);
}

/// Compare the stored bits, and say the case in hex when they differ: `0.1 differs` is not actionable,
/// `0x3dcccccd` is.
#[track_caller]
fn same(got: u32, want: f32, what: std::fmt::Arguments) {
    assert_eq!(got, want.to_bits(),
               "{what}: the game stored {} ({got:#010x}), the model {want} ({:#010x})",
               f32::from_bits(got), want.to_bits());
}

/// The same for a value that came back in `$f0`.
#[track_caller]
fn same_f0(ee: &Ee, want: f32, what: std::fmt::Arguments) {
    same(ee.f0().to_bits(), want, what);
}

// --- the scalar FPU: F1 and F2 ----------------------------------------------------------------------

/// `add.s` and `sub.s`, three lanes at a time, over every exponent difference.
///
/// The two carriers are the game's own `vec3` add and subtract: three `add.s` (or `sub.s`) and three
/// `swc1`, nothing else. What is being proved is the adder's alignment — that the port masks the smaller
/// operand to the exponent difference minus one guard bit, and drops it entirely at 25 — and the
/// saturation at both ends, since a quarter of the pairs here are `±MAX` against something.
#[test]
fn the_adder_agrees_at_every_exponent_difference() {
    let Some((mut ee, f)) = ready(&["vec3_add", "vec3_sub"]) else { return };
    let (add, sub) = (f[0], f[1]);
    let (out, pa, pb) = (ee.scratch(0), ee.scratch(1), ee.scratch(2));
    let mut rng = Rng(0x0192_a4c3_55ee_1101);
    for _ in 0..N {
        let mut a = [0.0f32; 3];
        let mut b = [0.0f32; 3];
        for k in 0..3 {
            let (x, y) = rng.pair();
            a[k] = x;
            b[k] = y;
        }
        put_vec(&mut ee, pa, a, rng.edge());
        put_vec(&mut ee, pb, b, rng.edge());

        for (name, carrier, model) in [("add", add, fpu::add as fn(f32, f32) -> f32), ("sub", sub, fpu::sub)] {
            put_vec(&mut ee, out, [f32::NAN; 3], f32::NAN);
            ee.call(carrier, &[out as u64, pa as u64, pb as u64], &[], BUDGET);
            for k in 0..3 {
                // The operands are read back through the FPU's own rules, because that is what `lwc1`
                // hands the adder: a denormal in memory arrives as a zero.
                let (x, y) = (ee.f32_at(pa + 4 * k), ee.f32_at(pb + 4 * k));
                same(ee.u32_at(out + 4 * k), model(x, y),
                     format_args!("{name}({x:e} = {:#010x}, {y:e} = {:#010x})", x.to_bits(), y.to_bits()));
            }
        }
    }
    println!("add.s/sub.s: {} pairs, bit-exact", 3 * 2 * N);
}

/// `mul.s` followed by `add.s`, with all three operands free.
///
/// The carrier accumulates a product into a field of its object — `*(obj + MUL_ADD_FIELD) += b * a`, as a
/// `mul.s` and a separate `add.s` rather than a `madd.s`. That makes it the full-domain test for `mul.s`
/// (both factors are arguments) and a second one for the adder, with a sum whose exponent is wherever the
/// product landed, including the saturated `±MAX` a product reaches long before a sum does.
#[test]
fn a_product_accumulated_into_a_field_matches_mul_then_add() {
    /// The float field the carrier adds into, and reads back out. From `research/oracle.txt`.
    const MUL_ADD_FIELD: u32 = 0x6c;
    let Some((mut ee, f)) = ready(&["float_mul_add_into"]) else { return };
    let mul_add = f[0];
    let obj = ee.scratch(0);
    let mut rng = Rng(0x5f1d_0bb3_7721_ac05);
    for _ in 0..N {
        let (a, b) = rng.pair();
        let prev = rng.edge();
        ee.set_f32(obj + MUL_ADD_FIELD, prev);
        ee.call(mul_add, &[obj as u64], &[a, b], BUDGET);

        // The field is read before the multiply and written after the add, so the model is the field as
        // the FPU would have loaded it, plus the chopped product.
        let want = fpu::add(fpu::operand(prev.to_bits()), fpu::mul(b, a));
        same(ee.u32_at(obj + MUL_ADD_FIELD), want,
             format_args!("{prev:e} + {b:e} * {a:e} ({:#010x}, {:#010x}, {:#010x})",
                          prev.to_bits(), b.to_bits(), a.to_bits()));
        same_f0(&ee, want, format_args!("the same value in $f0"));
    }
    println!("mul.s then add.s: {N} cases, bit-exact");
}

/// The accumulator chain: `mula.s`, `madda.s`, `madd.s`, and `sqrt.s` on the end of it.
///
/// Three carriers run the identical chain — the dot product, the squared length and the length — so one
/// set of vectors proves the order of the chain, the truncation of each product before it is added, and
/// the square root of whatever that left. With the edge generator the chain's input is routinely a
/// saturated product or a flushed denormal, which is where an accumulator modelled as `a + b * c` parts
/// company with the hardware.
#[test]
fn the_accumulator_chain_holds_up_at_the_edges() {
    let Some((mut ee, f)) = ready(&["vec3_dot", "vec3_length_sq", "vec3_length"]) else { return };
    let (dot, length_sq, length) = (f[0], f[1], f[2]);
    let (pa, pb) = (ee.scratch(0), ee.scratch(1));
    let mut rng = Rng(0x77c1_0a55_3312_dd91);
    for _ in 0..N {
        let (a, b) = (rng.vec3(), rng.vec3());
        put_vec(&mut ee, pa, a, rng.edge());
        put_vec(&mut ee, pb, b, rng.edge());
        let a: [f32; 3] = std::array::from_fn(|k| ee.f32_at(pa + 4 * k as u32));
        let b: [f32; 3] = std::array::from_fn(|k| ee.f32_at(pb + 4 * k as u32));

        ee.call(dot, &[pa as u64, pb as u64], &[], BUDGET);
        same_f0(&ee, fpu::dot(a, b), format_args!("dot({a:?}, {b:?})"));

        let square = Acc::mula(a[0], a[0]).madda(a[1], a[1]).madd(a[2], a[2]);
        ee.call(length_sq, &[pa as u64], &[], BUDGET);
        same_f0(&ee, square.to_f32(), format_args!("length_sq({a:?})"));

        ee.call(length, &[pa as u64], &[], BUDGET);
        same_f0(&ee, square.sqrt().to_f32(), format_args!("length({a:?})"));
    }
    println!("mula.s/madda.s/madd.s/sqrt.s: {} chains, bit-exact", 3 * N);
}

/// `mula.s` with `msub.s` and `madd.s`, from the quaternion-to-matrix code.
///
/// This is the only call-free carrier in the image that uses `msub.s` and lets you see the result: it
/// turns a quaternion into two rows of a rotation matrix, as eleven `mul.s`, two `add.s`, three `sub.s`
/// and the two accumulator pairs `mula.s`/`msub.s` and `mula.s`/`madd.s`, writing six floats. The model
/// below is that instruction sequence transcribed, so it proves the operand *order* of the accumulator
/// ops as much as their arithmetic — `msub.s` is `acc - a*b`, and getting the sign or the operands the
/// wrong way round is the classic way to be subtly wrong about a rotation.
#[test]
fn the_quaternion_rows_pin_down_msub_and_madd() {
    let Some((mut ee, f)) = ready(&["quat_two_matrix_rows"]) else { return };
    let rows = f[0];
    let (q, row_a, row_b) = (ee.scratch(0), ee.scratch(1), ee.scratch(2));
    let mut rng = Rng(0x2d9f_c411_6a07_b3e2);
    for _ in 0..N {
        // Half the cases are a quaternion of ordinary magnitudes (what the game has) and half are the
        // edge classes (what it does not, and what a port has to survive anyway).
        let unit = rng.next() & 1 == 0;
        let qv: [f32; 4] = std::array::from_fn(|_| if unit { rng.playable() } else { rng.edge() });
        for (k, x) in qv.iter().enumerate() {
            ee.set_f32(q + 4 * k as u32, *x);
        }
        let [q0, q1, q2, q3]: [f32; 4] = std::array::from_fn(|k| ee.f32_at(q + 4 * k as u32));

        ee.call(rows, &[q as u64, row_a as u64, row_b as u64], &[], BUDGET);

        // The instruction order, transcribed. Two and one are the only constants in the function.
        let (two, one) = (2.0f32, 1.0f32);
        let (t0, t2, t1) = (fpu::mul(q0, two), fpu::mul(q2, two), fpu::mul(q1, two));
        let zz = fpu::mul(q2, t2);
        let xx = fpu::mul(q0, t0);
        let wx = fpu::mul(q3, t0);
        let xx_zz = fpu::add(xx, zz);
        let yz = fpu::mul(q1, t2);
        let yy = fpu::mul(q1, t1);
        let xx_yy = fpu::add(xx, yy);
        let want_b = [
            Acc::mula(q0, t1).msub(q3, t2).to_f32(), // 2·q0·q1 − 2·q3·q2
            fpu::sub(one, xx_zz),
            fpu::add(yz, wx),
        ];
        let want_a = [
            Acc::mula(q0, t2).madd(q3, t1).to_f32(), // 2·q0·q2 + 2·q3·q1
            fpu::sub(yz, wx),
            fpu::sub(one, xx_yy),
        ];
        for k in 0..3 {
            let at = 4 * k as u32;
            same(ee.u32_at(row_b + at), want_b[k], format_args!("row_b[{k}] of {qv:?}"));
            same(ee.u32_at(row_a + at), want_a[k], format_args!("row_a[{k}] of {qv:?}"));
        }
    }
    println!("mula.s/msub.s/madd.s: {} rows, bit-exact", 2 * N);
}

/// `div.s`, through both carriers: one with the numerator fixed and one with it free.
///
/// The divider is the operation that rounds *to nearest* rather than toward zero, and the one with a
/// defined answer for `x / 0` (`±MAX`, signed by the operands) — so the generator's zeros, denormals
/// (which the divisor flushes to zero before dividing) and `±MAX` are all live cases rather than
/// curiosities. The second carrier also runs `max.s`, a `mul.s` by a half and a `sub.s` on the way, in
/// the order the function has them, which is what makes the numerator free.
#[test]
fn the_divider_agrees_including_by_zero() {
    /// The fields the two carriers use, from `research/oracle.txt`.
    const RECIP_IN: u32 = 0x94;
    const RECIP_OUT: u32 = 0x90;
    const SPAN_BASE: u32 = 0x5c;
    const SPAN_LIMIT: u32 = 0x64;
    const SPAN_RATE: u32 = 0x68;
    const SPAN_HALF: u32 = 0x6c;
    let Some((mut ee, f)) = ready(&["float_recip_32767", "float_span_rate"]) else { return };
    let (recip, span) = (f[0], f[1]);
    let obj = ee.scratch(0);
    let mut rng = Rng(0x41b3_77ca_0e92_5de1);
    for _ in 0..N {
        // 32767.0 / x, for every x the generator can make.
        let x = rng.edge();
        ee.call(recip, &[obj as u64], &[x], BUDGET);
        same(ee.u32_at(obj + RECIP_IN), x, format_args!("the argument is stored unchanged"));
        same(ee.u32_at(obj + RECIP_OUT), fpu::div(32767.0, x),
             format_args!("32767 / {x:e} ({:#010x})", x.to_bits()));

        // (b − base) / (0.5 · max(a, limit)), which puts the generator on both sides of the divide.
        let (a, b) = rng.pair();
        let (base, limit) = (rng.edge(), rng.edge());
        ee.set_f32(obj + SPAN_BASE, base);
        ee.set_f32(obj + SPAN_LIMIT, limit);
        ee.call(span, &[obj as u64], &[a, b], BUDGET);

        let kept = fpu::max(a, fpu::operand(limit.to_bits()));
        let half = fpu::mul(0.5, kept);
        let rate = fpu::div(fpu::sub(b, fpu::operand(base.to_bits())), half);
        same(ee.u32_at(obj + SPAN_LIMIT), kept, format_args!("max({a:e}, {limit:e})"));
        same(ee.u32_at(obj + SPAN_HALF), half, format_args!("0.5 * max({a:e}, {limit:e})"));
        same(ee.u32_at(obj + SPAN_RATE), rate,
             format_args!("({b:e} - {base:e}) / {half:e} ({:#010x}, {:#010x})", b.to_bits(), base.to_bits()));
    }
    println!("div.s: {} divisions, bit-exact", 2 * N);
}

/// `rsqrt.s`, over the whole input range including the negatives.
///
/// The carrier is `1.0 / sqrt(x)` as a single `rsqrt.s` against a `lui`-built one — a one-argument
/// function, so unlike `sqrt.s` (which the image only ever reaches through a sum of squares) every
/// pattern the generator produces is a legal case: negatives, which the instruction roots by magnitude,
/// zero, which gives `+MAX`, and denormals, which flush first and then do the same.
#[test]
fn the_reciprocal_square_root_covers_the_whole_range() {
    let Some((mut ee, f)) = ready(&["float_inv_sqrt"]) else { return };
    let inv_sqrt = f[0];
    let mut rng = Rng(0x6b20_1fe4_c08a_7741);
    for _ in 0..N {
        let x = rng.edge();
        ee.call(inv_sqrt, &[], &[x], BUDGET);
        same_f0(&ee, fpu::rsqrt(1.0, x), format_args!("1 / sqrt({x:e} = {:#010x})", x.to_bits()));
    }
    println!("rsqrt.s: {N} inputs, bit-exact");
}

/// `max.s`, `min.s` and `abs.s` on the patterns the FPU has opinions about.
///
/// `tests/basics.rs` in `ps2-oracle` already runs these over ordinary values; what this adds is the cases
/// where a sign-magnitude compare and a numeric one differ — `max(0.0, -0.0)`, anything against a
/// denormal (flushed on the way in, so `max(-0.0, 1e-40)` is a comparison of two zeros), and the
/// Inf/NaN patterns, which arrive as `±MAX` and compare like any other number.
#[test]
fn min_max_and_abs_at_the_edges() {
    let Some((mut ee, f)) = ready(&["float_max", "float_min", "float_abs"]) else { return };
    let (max, min, abs) = (f[0], f[1], f[2]);
    let mut rng = Rng(0x1c74_5ab0_9f31_2206);
    for _ in 0..N {
        let (a, b) = rng.pair();
        ee.call(max, &[], &[a, b], BUDGET);
        same_f0(&ee, fpu::max(a, b), format_args!("max({a:e}, {b:e})"));
        ee.call(min, &[], &[a, b], BUDGET);
        same_f0(&ee, fpu::min(a, b), format_args!("min({a:e}, {b:e})"));
        ee.call(abs, &[], &[a], BUDGET);
        // `abs.s` is a bit operation, not an arithmetic one: it clears the sign and leaves even a
        // denormal alone, where `max.s` and `min.s` flush their operands on the way in. Modelling it
        // as "flush, then take the magnitude" is wrong, and this generator is what says so.
        same_f0(&ee, fpu::abs(a), format_args!("abs({a:e})"));
    }
    println!("max.s/min.s/abs.s: {} cases, bit-exact", 3 * N);
}

/// `cvt.w.s` and `cvt.s.w`, the two conversions, over the range the carriers can show.
///
/// Both carriers store their answer as a 16-bit field, so this proves the conversion modulo 65536 going
/// out and over `i16` coming back — enough to catch a rounding mode (the EE truncates, it does not round)
/// or a saturation bug in the low bits, not enough to see the `0x7fffffff` an out-of-range float
/// saturates to. The saturation itself is in `tests/fpu.rs`.
#[test]
fn the_conversions_match_over_the_range_the_carriers_show() {
    let Some((mut ee, f)) = ready(&["float_to_i16_pair", "i16_pair_to_float"]) else { return };
    let (to_int, to_float) = (f[0], f[1]);
    let (pair, pa, pb) = (ee.scratch(0), ee.scratch(1), ee.scratch(2));
    let mut rng = Rng(0x30fe_6612_bb41_90a7);
    for _ in 0..N {
        // Out: two floats in memory, two i16 fields out.
        let (x, y) = (rng.edge(), rng.edge());
        ee.set_f32(pa, x);
        ee.set_f32(pb, y);
        ee.call(to_int, &[pair as u64, pa as u64, pb as u64], &[], BUDGET);
        for (k, v) in [(0, x), (2, y)] {
            let want = fpu::to_i32(fpu::operand(v.to_bits())) as u16;
            assert_eq!(ee.u16_at(pair + k), want,
                       "(int){v:e} ({:#010x}): the game stored {:#06x}, the model {want:#06x}",
                       v.to_bits(), ee.u16_at(pair + k));
        }

        // Back: two i16 fields in, two floats out.
        let (i, j) = (rng.next() as u16, rng.next() as u16);
        ee.set_u32(pair, i as u32 | ((j as u32) << 16));
        ee.set_f32(pa, f32::NAN);
        ee.set_f32(pb, f32::NAN);
        ee.call(to_float, &[pair as u64, pa as u64, pb as u64], &[], BUDGET);
        for (at, v) in [(pa, i as i16), (pb, j as i16)] {
            same(ee.u32_at(at), fpu::from_i32(v as i32), format_args!("(float){v}"));
        }
    }
    println!("cvt.w.s/cvt.s.w: {} conversions, bit-exact", 4 * N);
}

// --- VU0 macro mode: F3 -----------------------------------------------------------------------------

/// `vsub`, `vmul` and `vadd`, lane by lane, on the same edge cases.
///
/// VU0 is a different adder from the FPU's — full width, no guard-bit masking — and a different
/// saturation, so running the identical generator through both is the cheapest way to prove the port has
/// not mixed the two models up. All three carriers write a whole quadword, so each one also proves the
/// masked `w` lane survives: `vsub.xyz` must leave it exactly as it was loaded, which is the detail that
/// turns into a drifting fourth component in a matrix.
#[test]
fn the_vu_lane_arithmetic_matches_lane_by_lane() {
    let Some((mut ee, f)) = ready(&["vec3_sub_vu", "vec3_mul_vu", "mat4_row3_add"]) else { return };
    let (sub, mul, row_add) = (f[0], f[1], f[2]);
    let (out, pa, pb) = (ee.scratch(0), ee.scratch(1), ee.scratch(2));
    let mut rng = Rng(0x4e8a_02d7_6fb1_c359);
    for _ in 0..N {
        let (mut a, mut b) = ([0.0f32; 3], [0.0f32; 3]);
        for k in 0..3 {
            let (x, y) = rng.pair();
            a[k] = x;
            b[k] = y;
        }
        let (aw, bw) = (rng.edge(), rng.edge());
        put_vec(&mut ee, pa, a, aw);
        put_vec(&mut ee, pb, b, bw);
        let a: [f32; 3] = std::array::from_fn(|k| vu::operand(ee.u32_at(pa + 4 * k as u32)));
        let b: [f32; 3] = std::array::from_fn(|k| vu::operand(ee.u32_at(pb + 4 * k as u32)));

        for (name, carrier, model) in
            [("vsub", sub, vu::sub as fn(f32, f32) -> f32), ("vmul", mul, vu::mul)]
        {
            put_vec(&mut ee, out, [f32::NAN; 3], f32::NAN);
            ee.call(carrier, &[out as u64, pa as u64, pb as u64], &[], BUDGET);
            for k in 0..3 {
                same(ee.u32_at(out + 4 * k), model(a[k as usize], b[k as usize]),
                     format_args!("{name}({a:?}, {b:?})[{k}]"));
            }
            assert_eq!(ee.u32_at(out + 12), aw.to_bits(),
                       "{name}: the masked w lane must come back as it was loaded");
        }

        // The matrix carrier adds a vector into row three and copies the rest: `vadd.xyz` on the lanes,
        // and three quadwords that must arrive untouched.
        for k in 0..12u32 {
            ee.set_u32(pa + 4 * k, rng.next());
        }
        put_vec(&mut ee, pa + 0x30, a, aw);
        put_vec(&mut ee, pb, b, bw);
        ee.call(row_add, &[out as u64, pa as u64, pb as u64], &[], BUDGET);
        for k in 0..12u32 {
            assert_eq!(ee.u32_at(out + 4 * k), ee.u32_at(pa + 4 * k),
                       "the first three rows are copied, not computed");
        }
        for k in 0..3u32 {
            same(ee.u32_at(out + 0x30 + 4 * k), vu::add(a[k as usize], b[k as usize]),
                 format_args!("vadd({a:?}, {b:?})[{k}]"));
        }
        assert_eq!(ee.u32_at(out + 0x3c), aw.to_bits(), "vadd.xyz must leave row three's w alone");
    }
    println!("vsub/vmul/vadd: {} lanes, bit-exact", 12 * N);
}

/// `vdiv` and the broadcast `vmulq` behind it, and `vrsqrt` through the normaliser.
///
/// The divide carrier is a vector divided by a scalar: one `vdiv` of the constant one by the argument,
/// `vwaitq`, then a broadcast multiply of `Q` into the three lanes. The normaliser is the same shape with
/// `vrsqrt` — the VU's *approximation*, not a correctly rounded root — on a length built from `vmul`,
/// `vadda` and `vmadd`. Between them they are every piece of VU0 the port's `normalize` is made of, and
/// the edge generator puts a zero, a denormal and a `±MAX` through each.
#[test]
fn the_vu_divider_and_the_normaliser_agree() {
    let Some((mut ee, f)) = ready(&["vec3_div_scalar", "vec3_normalize"]) else { return };
    let (div_scalar, normalize) = (f[0], f[1]);
    let (out, pv) = (ee.scratch(0), ee.scratch(1));
    let mut rng = Rng(0x58c3_11af_70d2_6e44);
    for _ in 0..N {
        let (v, w, s) = (rng.vec3(), rng.edge(), rng.edge());
        put_vec(&mut ee, pv, v, w);
        let v: [f32; 3] = std::array::from_fn(|k| vu::operand(ee.u32_at(pv + 4 * k as u32)));

        // In place, and it returns its own argument.
        ee.call(div_scalar, &[pv as u64], &[s], BUDGET);
        assert_eq!(ee.v0() as u32, pv, "the carrier returns the vector it divided");
        let q = vu::div(1.0, s);
        for k in 0..3 {
            same(ee.u32_at(pv + 4 * k), vu::mul(v[k as usize], q),
                 format_args!("({v:?} / {s:e})[{k}]"));
        }
        assert_eq!(ee.u32_at(pv + 12), w.to_bits(), "vmulq.xyz must leave w alone");

        // The normaliser, whose chain is `vmul` for the squares, `vadda`/`vmadd` for the sum and
        // `vrsqrt` for the scale.
        put_vec(&mut ee, pv, v, w);
        put_vec(&mut ee, out, [f32::NAN; 3], f32::NAN);
        ee.call(normalize, &[out as u64, pv as u64], &[], BUDGET);
        let sq: [f32; 3] = std::array::from_fn(|k| vu::mul(v[k], v[k]));
        let len2 = vu::madd(vu::add(sq[0], sq[1]), 1.0, sq[2]);
        let inv = vu::rsqrt(1.0, len2);
        for k in 0..3 {
            same(ee.u32_at(out + 4 * k), vu::mul(v[k as usize], inv),
                 format_args!("normalize({v:?})[{k}]"));
        }
        assert_eq!(ee.u32_at(out + 12), w.to_bits(), "the normaliser must leave w alone");
    }
    println!("vdiv/vmulq/vrsqrt: {} lanes, bit-exact", 6 * N);
}
