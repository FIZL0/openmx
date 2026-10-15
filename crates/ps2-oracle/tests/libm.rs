//! F4 — the disc's own transcendentals against `ps2_float::libm`, over the whole input range.
//!
//! `tests/game.rs` is the worked example: a few names, a few thousand inputs each. This file is the
//! proof. The game's `libm` is pure arithmetic on its arguments, so the oracle can answer for *every*
//! exponent an `f32` has, which is what [`sweep1`] does: it walks exponents 0..=254 and tries random
//! mantissas in each, both signs, so the tiny arguments that take the early-out, the ordinary ones, the
//! ones that need the big argument reduction and the ones the EE reads as `±max` are all covered in one
//! run. A range-limited test would have missed every one of the branches that only a real angle hits.
//!
//! Everything here skips with a message when there is no disc, so a fresh checkout stays green.

use ps2_oracle::{Ee, Kit, skip};

/// A deterministic xorshift: a failure is reproducible from the seed, which "it failed once in CI" is
/// not.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }
    /// A float with exponent field `e` and a random mantissa and sign.
    fn in_exponent(&mut self, e: u32) -> f32 {
        let (m, s) = (self.next() & 0x007f_ffff, (self.next() >> 31) << 31);
        f32::from_bits(s | (e << 23) | m)
    }
}

/// The inputs every float function has to be tried on whatever its domain is: the signed zeros, the
/// signed ones, the smallest normal and denormal, and the bit patterns an IEEE machine calls infinity
/// and NaN — which on the EE are just `±max` and large numbers, so the game's code runs straight into
/// them rather than branching away.
const EDGES: [u32; 12] = [
    0x0000_0000, 0x8000_0000, 0x3f80_0000, 0xbf80_0000, 0x0080_0000, 0x0000_0001, 0x807f_ffff,
    0x7f7f_ffff, 0xff7f_ffff, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000,
];

/// Run `port` and the game's `at` on the same inputs and require identical bits.
///
/// Exponents outside `exps` are skipped, not because they cannot be tried, but because for a function
/// like `powf` they are the cases where every answer is `±max` and a failure says nothing.
fn sweep1(ee: &mut Ee, at: u32, name: &str, port: impl Fn(f32) -> f32, exps: core::ops::RangeInclusive<u32>,
          per_exp: u32) {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut n = 0u32;
    let check = |ee: &mut Ee, x: f32, n: &mut u32| {
        ee.call(at, &[], &[x], 2_000_000);
        let (got, want) = (ee.f0(), port(x));
        assert_eq!(got.to_bits(), want.to_bits(),
                   "{name}({x:e} = {:#010x}): the game gives {got:e} ({:#010x}), the port {want:e} \
                    ({:#010x})",
                   x.to_bits(), got.to_bits(), want.to_bits());
        *n += 1;
    };
    for bits in EDGES {
        check(ee, f32::from_bits(bits), &mut n);
    }
    for e in exps {
        for _ in 0..per_exp {
            let x = rng.in_exponent(e);
            check(ee, x, &mut n);
        }
    }
    println!("{name}: {n} inputs, bit-exact");
}

/// The same for a two-argument function. `pick` turns two random floats into the argument pair, so a
/// caller can keep `powf`'s base positive or give `fmodf` a divisor worth dividing by.
fn sweep2(ee: &mut Ee, at: u32, name: &str, port: impl Fn(f32, f32) -> f32,
          exps: core::ops::RangeInclusive<u32>, per_exp: u32, pick: impl Fn(f32, f32) -> (f32, f32)) {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut n = 0u32;
    let check = |ee: &mut Ee, x: f32, y: f32, n: &mut u32| {
        ee.call(at, &[], &[x, y], 2_000_000);
        let (got, want) = (ee.f0(), port(x, y));
        assert_eq!(got.to_bits(), want.to_bits(),
                   "{name}({x:e}, {y:e} = {:#010x}, {:#010x}): the game gives {got:e} ({:#010x}), the \
                    port {want:e} ({:#010x})",
                   x.to_bits(), y.to_bits(), got.to_bits(), want.to_bits());
        *n += 1;
    };
    for a in EDGES {
        for b in EDGES {
            let (x, y) = (f32::from_bits(a), f32::from_bits(b));
            check(ee, x, y, &mut n);
        }
    }
    let all: Vec<u32> = exps.collect();
    for &e in &all {
        for _ in 0..per_exp {
            let f = all[(rng.next() as usize) % all.len()];
            let (x, y) = pick(rng.in_exponent(e), rng.in_exponent(f));
            check(ee, x, y, &mut n);
        }
    }
    println!("{name}: {n} inputs, bit-exact");
}

/// Everything this file can prove, in one test: one `Ee` is ~32 MB, and loading it once per function
/// would be the slowest thing in `./kit check`.
#[test]
fn the_discs_libm_is_bit_exact() {
    let kit = match Kit::load() {
        Ok(k) => k,
        Err(why) => return skip(&why),
    };
    let mut ee = match Ee::game() {
        Ok(ee) => ee,
        Err(why) => return skip(&why),
    };
    let at = |n: &str| kit.addr(n).unwrap_or_else(|e| panic!("{e}"));

    // sqrt.s, and so the whole of `fpu::sqrt`: one instruction, every exponent, 1 in 4 of them denormal
    // or ±max on the way in or out.
    sweep1(&mut ee, at("sqrtf"), "sqrtf", ps2_float::libm::sqrtf, 0..=254, 20);

    // The trigonometry. Below 2^-27 (exponent 100) sinf returns its argument and cosf returns 1; above
    // 2^7·π/2 (exponent 133) the three-step reduction gives up and the fixed-point one runs, which is
    // the part of this that no range-limited test would have reached.
    sweep1(&mut ee, at("sinf"), "sinf", ps2_float::libm::sinf, 0..=254, 40);
    sweep1(&mut ee, at("cosf"), "cosf", ps2_float::libm::cosf, 0..=254, 40);

    // atanf: the disc links it, and nothing links atan2f (F4's journal entry says what the game uses
    // instead), so this is the only angle function here with a real call site.
    sweep1(&mut ee, at("atanf"), "atanf", ps2_float::libm::atanf, 0..=254, 40);

    // fmodf is exact — no rounding anywhere in it — so every exponent pair must agree, including the
    // ones where the quotient needs all 255 shifts.
    sweep2(&mut ee, at("fmodf"), "fmodf", ps2_float::libm::fmodf, 1..=254, 24, |a, b| (a, b));

    // powf, with a positive base: the negative-base cases are the next block, because they only reach
    // the polynomial when y is a whole number and random pairs never are. The whole exponent range on
    // both sides, so the |y| > 2^27 shortcut, the over/underflow limits (which are the EE's own, 2^-126
    // and 2^129, not fdlibm's) and the ordinary path all get their turn.
    sweep2(&mut ee, at("powf"), "powf", ps2_float::libm::powf, 1..=254, 24, |a, b| (a.abs(), b));
    // and the integer exponents, where x may be negative and the sign of the result is y's parity.
    let mut rng = Rng(0xdead_beef_0bad_f00d);
    for k in 0..4_000u32 {
        let e = 100 + rng.next() % 60;
        let x = rng.in_exponent(e);
        let y = (k % 2 == 0).then(|| (rng.next() % 64) as f32).unwrap_or(-((rng.next() % 64) as f32));
        ee.call(at("powf"), &[], &[x, y], 2_000_000);
        let (got, want) = (ee.f0(), ps2_float::libm::powf(x, y));
        assert_eq!(got.to_bits(), want.to_bits(),
                   "powf({x:e}, {y}): the game gives {got:e} ({:#010x}), the port {want:e} ({:#010x})",
                   got.to_bits(), want.to_bits());
    }
    println!("powf: 4000 whole-number exponents, bit-exact");
}
