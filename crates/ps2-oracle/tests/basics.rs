//! The oracle, aimed at functions whose answer a human already knows.
//!
//! Everything else proved with the oracle rests on the interpreter being right, so the interpreter has to
//! stop being a suspect first. The cheapest way to do that is not to study it — it is to call code whose
//! answer needs no study: `max(3, 7)` is 7, the length of `(3, 4, 0)` is 5, and a binary search over a
//! sorted table returns the same index a linear scan does.
//!
//! So this file picks the smallest real functions the game has — one instruction each, in places — and
//! runs them two ways: a handful of cases worked out by hand, then ten thousand generated ones against a
//! [`ps2_float`] model. The hand cases say the interpreter is calling the right code at all; the
//! generated ones say it agrees bit for bit where nobody would have thought to look.
//!
//! The functions are the game's own inlined maths helpers, named in `research/oracle.txt`. They were
//! chosen for being *boring*: no globals, no allocation, no calls, nothing that depends on when they ran.
//! Between them they still exercise most of what a real proof will need — the FPU's accumulator chain,
//! `sqrt.s`, `max.s`/`min.s`, VU0 macro mode, quadword loads and stores, pointer arguments, out-parameters
//! written back to memory, and a loop with a data-dependent exit.
//!
//! Everything here skips with a message when the disc is not set up, so a fresh checkout stays green.

use ps2_float::{Acc, F, fpu, vu};
use ps2_oracle::{Ee, addr, game_or_skip, skip};

/// How many generated cases each property gets. Ten thousand is the number in `AGENT.md`, and it costs
/// well under a second for functions this small.
const N: u32 = 10_000;

/// A generous ceiling: the largest of these functions runs a few dozen instructions.
const BUDGET: u64 = 100_000;

/// Load the game, then look up every name the test needs, skipping with one message if any is missing.
///
/// A name goes missing when the addresses file has not been filled in for this disc yet, which is a
/// "not set up" rather than a failure — the same treatment the disc itself gets.
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

/// A tiny xorshift, so a failure is reproducible from the seed alone rather than "it happened once".
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    /// A float spread over the magnitudes a game actually uses, both signs, and occasionally exactly
    /// zero — which is where `max.s`/`min.s` and the accumulator have their own opinions.
    fn float(&mut self) -> f32 {
        let (a, b) = (self.next(), self.next());
        let m = 1.0 - 2.0 * (a as f32 / u32::MAX as f32);
        match b % 8 {
            0 => 0.0,
            1 => -0.0,
            2 => m,
            3 => m / 1024.0,
            4 => m * 1024.0,
            5 => m * 1.0e6,
            6 => m / 16_777_216.0, // down where chopping shows
            _ => m * 100.0,
        }
    }

    fn vec3(&mut self) -> [f32; 3] {
        [self.float(), self.float(), self.float()]
    }
}

/// Write a vector where a function can read it, as a quadword: these helpers are compiled for `lqc2` as
/// often as for `lwc1`, so the fourth lane has to hold something defined.
fn put_vec(ee: &mut Ee, at: u32, v: [f32; 3], w: f32) {
    for (k, x) in v.iter().enumerate() {
        ee.set_f32(at + 4 * k as u32, *x);
    }
    ee.set_f32(at + 12, w);
}

fn get_vec(ee: &Ee, at: u32) -> [f32; 3] {
    std::array::from_fn(|k| ee.f32_at(at + 4 * k as u32))
}

/// Compare bit patterns, and say the inputs in both decimal and hex when they differ — `0.1 differs` is
/// not something anyone can act on, `0x3dcccccd` is.
#[track_caller]
fn same(got: f32, want: f32, what: std::fmt::Arguments) {
    assert_eq!(got.to_bits(), want.to_bits(),
               "{what}: the game gives {got} ({:#010x}), the model {want} ({:#010x})",
               got.to_bits(), want.to_bits());
}

// --- the answers a human can check ------------------------------------------------------------------

/// Before any property: twelve cases with answers from primary school. If this fails, nothing else in
/// this file is worth reading — the oracle is calling the wrong code, or not calling it at all.
#[test]
fn the_obvious_answers_are_the_obvious_answers() {
    let names = ["float_max", "float_min", "float_abs", "vec3_dot", "vec3_length", "vec3_length_sq"];
    let Some((mut ee, f)) = ready(&names) else { return };
    let (max, min, abs, dot, length, length_sq) = (f[0], f[1], f[2], f[3], f[4], f[5]);

    for (a, b, hi, lo) in [(3.0f32, 7.0f32, 7.0f32, 3.0f32), (-1.0, -4.0, -1.0, -4.0), (2.5, 2.5, 2.5, 2.5)] {
        ee.call(max, &[], &[a, b], BUDGET);
        same(ee.f0(), hi, format_args!("max({a}, {b})"));
        ee.call(min, &[], &[a, b], BUDGET);
        same(ee.f0(), lo, format_args!("min({a}, {b})"));
    }
    for (x, want) in [(-2.5f32, 2.5f32), (2.5, 2.5), (0.0, 0.0)] {
        ee.call(abs, &[], &[x], BUDGET);
        same(ee.f0(), want, format_args!("abs({x})"));
    }

    // The 3-4-5 triangle, and a dot product small enough to do in your head: 1·4 + 2·5 + 3·6 = 32.
    let (a, b) = (ee.scratch(0), ee.scratch(1));
    put_vec(&mut ee, a, [3.0, 4.0, 0.0], 1.0);
    ee.call(length, &[a as u64], &[], BUDGET);
    same(ee.f0(), 5.0, format_args!("length(3, 4, 0)"));
    ee.call(length_sq, &[a as u64], &[], BUDGET);
    same(ee.f0(), 25.0, format_args!("length_sq(3, 4, 0)"));

    put_vec(&mut ee, a, [1.0, 2.0, 3.0], 1.0);
    put_vec(&mut ee, b, [4.0, 5.0, 6.0], 1.0);
    ee.call(dot, &[a as u64, b as u64], &[], BUDGET);
    same(ee.f0(), 32.0, format_args!("dot((1,2,3), (4,5,6))"));

    // And the degenerate one, because a length that reads uninitialised memory tends to pass the first
    // three and fail this.
    put_vec(&mut ee, a, [0.0, 0.0, 0.0], 1.0);
    ee.call(length, &[a as u64], &[], BUDGET);
    same(ee.f0(), 0.0, format_args!("length(0, 0, 0)"));
}

// --- then the same functions, ten thousand inputs at a time -----------------------------------------

/// `max.s`, `min.s` and `abs.s`, one instruction each, against the model a port will use.
///
/// Worth doing despite the triviality: `max.s` is not `f32::max` (it is a sign-magnitude compare, with no
/// opinion about NaN), and signed zero is the case everyone gets wrong — `max(0.0, -0.0)` has an answer
/// and it is not symmetric.
#[test]
fn min_max_and_abs_are_the_fpus_own() {
    let Some((mut ee, f)) = ready(&["float_max", "float_min", "float_abs"]) else { return };
    let (max, min, abs) = (f[0], f[1], f[2]);
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for _ in 0..N {
        let (a, b) = (rng.float(), rng.float());
        ee.call(max, &[], &[a, b], BUDGET);
        same(ee.f0(), fpu::max(a, b), format_args!("max({a}, {b})"));
        ee.call(min, &[], &[a, b], BUDGET);
        same(ee.f0(), fpu::min(a, b), format_args!("min({a}, {b})"));
        ee.call(abs, &[], &[a], BUDGET);
        same(ee.f0(), fpu::abs(a), format_args!("abs({a})"));
    }
    println!("min/max/abs: {N} inputs each, bit-exact");
}

/// A clamp, built the way the game builds one: its own `min` of its own `max`.
///
/// The game has no single clamp function — the call sites compose these two — so this proves the
/// composition, which is what [`F::clamp`] claims to be. Half the cases are the interesting ones, where
/// the bounds are the wrong way round and the answer is whichever bound the order picks.
#[test]
fn a_clamp_is_min_of_max_and_matches_the_float_model() {
    let Some((mut ee, f)) = ready(&["float_max", "float_min"]) else { return };
    let (max, min) = (f[0], f[1]);
    let mut rng = Rng(0x1234_5678_9abc_def1);
    for k in 0..N {
        let x = rng.float();
        let (mut lo, mut hi) = (rng.float(), rng.float());
        if k % 2 == 0 && lo > hi {
            std::mem::swap(&mut lo, &mut hi); // the sane half: lo <= hi
        }
        ee.call(max, &[], &[x, lo], BUDGET);
        let lifted = ee.f0();
        ee.call(min, &[], &[lifted, hi], BUDGET);
        same(ee.f0(), F(x).clamp(lo, hi).to_f32(), format_args!("clamp({x}, {lo}, {hi})"));
    }
    println!("clamp: {N} inputs, bit-exact");
}

/// The dot product and the two lengths, which is where the FPU's accumulator earns its keep.
///
/// All three are the same `mula.s` / `madda.s` / `madd.s` chain, and the chain is not three multiplies
/// and two adds: each product is chopped and then aligned against a running total that is wider than an
/// `f32`. Summing the same terms in another order gives a different answer, so this is really a test that
/// [`fpu::dot`] has the order right.
#[test]
fn the_accumulator_chain_matches_the_float_model() {
    let names = ["vec3_dot", "vec3_length", "vec3_length_sq"];
    let Some((mut ee, f)) = ready(&names) else { return };
    let (dot, length, length_sq) = (f[0], f[1], f[2]);
    let (pa, pb) = (ee.scratch(0), ee.scratch(1));
    let mut rng = Rng(0x0bad_f00d_dead_beef);
    for _ in 0..N {
        let (a, b) = (rng.vec3(), rng.vec3());
        put_vec(&mut ee, pa, a, 1.0);
        put_vec(&mut ee, pb, b, 1.0);

        ee.call(dot, &[pa as u64, pb as u64], &[], BUDGET);
        same(ee.f0(), fpu::dot(a, b), format_args!("dot({a:?}, {b:?})"));

        let square = Acc::mula(a[0], a[0]).madda(a[1], a[1]).madd(a[2], a[2]);
        ee.call(length_sq, &[pa as u64], &[], BUDGET);
        same(ee.f0(), square.to_f32(), format_args!("length_sq({a:?})"));

        ee.call(length, &[pa as u64], &[], BUDGET);
        same(ee.f0(), square.sqrt().to_f32(), format_args!("length({a:?})"));
    }
    println!("dot/length/length_sq: {N} vectors, bit-exact");
}

/// The three that write their answer back through a pointer, including one that goes through VU0.
///
/// `add` and `sub` are plain `add.s`/`sub.s` into three words; `scale` is the quadword path — `lqc2`, a
/// broadcast multiply in VU0 macro mode, `sqc2` — so it is also the test that the oracle's VU0 lane
/// masking is right. The masked-off `w` lane must come back untouched, which is the detail a port gets
/// wrong when it treats a vec4 as three floats.
#[test]
fn the_vector_writers_put_back_what_the_model_says() {
    let Some((mut ee, f)) = ready(&["vec3_add", "vec3_sub", "vec3_scale"]) else { return };
    let (add, sub, scale) = (f[0], f[1], f[2]);
    let (out, pa, pb) = (ee.scratch(0), ee.scratch(1), ee.scratch(2));
    let mut rng = Rng(0x5eed_1234_5eed_4321);
    for _ in 0..N {
        let (a, b, s, w) = (rng.vec3(), rng.vec3(), rng.float(), rng.float());
        put_vec(&mut ee, pa, a, w);
        put_vec(&mut ee, pb, b, 1.0);

        put_vec(&mut ee, out, [f32::NAN; 3], f32::NAN);
        ee.call(add, &[out as u64, pa as u64, pb as u64], &[], BUDGET);
        for k in 0..3 {
            same(ee.f32_at(out + 4 * k), fpu::add(a[k as usize], b[k as usize]),
                 format_args!("add({a:?}, {b:?})[{k}]"));
        }

        put_vec(&mut ee, out, [f32::NAN; 3], f32::NAN);
        ee.call(sub, &[out as u64, pa as u64, pb as u64], &[], BUDGET);
        for k in 0..3 {
            same(ee.f32_at(out + 4 * k), fpu::sub(a[k as usize], b[k as usize]),
                 format_args!("sub({a:?}, {b:?})[{k}]"));
        }

        // VU0 multiplies are not the FPU's: different rounding on overflow, different clamping.
        put_vec(&mut ee, out, [f32::NAN; 3], f32::NAN);
        ee.call(scale, &[out as u64, pa as u64], &[s], BUDGET);
        let got = get_vec(&ee, out);
        for k in 0..3 {
            same(got[k], vu::mul(a[k], s), format_args!("scale({a:?}, {s})[{k}]"));
        }
        same(ee.f32_at(out + 12), w, format_args!("scale({a:?}, {s}).w — the masked lane must survive"));
    }
    println!("add/sub/scale: {N} vectors, bit-exact");
}

// --- the two that taught the interpreter something new ----------------------------------------------

/// The quadword absolute value, which is where the interpreter first ran out of instructions.
///
/// It builds the mask `0x7fffffff` in all four lanes with `lui`/`ori`, `pextlw` against itself and
/// `pcpyld`, then `pand`s the vector — so this is also the test for `pextlw`, the MMI instruction this
/// task added. Its answer needs no model at all: clearing the sign bit is clearing the sign bit, and a
/// wrong interleave leaves some lane masked with zero or with `0xffffffff`, which shows immediately.
#[test]
fn the_quadword_absolute_value_clears_four_sign_bits() {
    let Some((mut ee, f)) = ready(&["vec4_abs"]) else { return };
    let abs4 = f[0];
    let (out, inp) = (ee.scratch(0), ee.scratch(1));
    let mut rng = Rng(0xabcd_0123_4567_89ef);
    for k in 0..N {
        let v: [f32; 4] = std::array::from_fn(|_| rng.float());
        for (j, x) in v.iter().enumerate() {
            ee.set_f32(inp + 4 * j as u32, *x);
            ee.set_f32(out + 4 * j as u32, f32::NAN);
        }
        ee.call(abs4, &[out as u64, inp as u64], &[], BUDGET);
        for j in 0..4 {
            let got = ee.u32_at(out + 4 * j);
            assert_eq!(got, v[j as usize].to_bits() & 0x7fff_ffff,
                       "case {k}: abs4({v:?})[{j}] came back {got:#010x}");
        }
    }
    println!("vec4 abs: {N} vectors, exact");
}

/// Normalise in place, the other function that needed a new instruction (`vmaxw`, used to conjure a
/// vector of ones out of `vf0`).
///
/// Unlike everything above it this one is *not* exactly checkable by hand, because `vrsqrt` is the VU's
/// approximation and not a correctly rounded reciprocal square root. So it gets both halves: the
/// generated cases compare against the same chain of VU operations spelled out in Rust, and a few fixed
/// cases check the answer is the direction a human would expect, to about a part in 4000.
#[test]
fn normalise_in_place_follows_the_vu_chain() {
    let Some((mut ee, f)) = ready(&["vec3_normalize"]) else { return };
    let normalize = f[0];
    let (out, inp) = (ee.scratch(0), ee.scratch(1));

    // (3, 4, 0) has to come back as (0.6, 0.8, 0) to within what `vrsqrt` can do.
    put_vec(&mut ee, inp, [3.0, 4.0, 0.0], 1.0);
    ee.call(normalize, &[out as u64, inp as u64], &[], BUDGET);
    let got = get_vec(&ee, out);
    for (g, want) in got.iter().zip([0.6f32, 0.8, 0.0]) {
        assert!((g - want).abs() < 2.5e-4, "normalize(3, 4, 0) gave {got:?}, expected (0.6, 0.8, 0)");
    }

    let mut rng = Rng(0x7777_1111_2222_3333);
    for _ in 0..N {
        let v = rng.vec3();
        let w = rng.float();
        put_vec(&mut ee, inp, v, w);
        put_vec(&mut ee, out, [f32::NAN; 3], f32::NAN);
        ee.call(normalize, &[out as u64, inp as u64], &[], BUDGET);

        // The chain the function actually runs: square the lanes, add x+y into the accumulator,
        // accumulate z through a multiply by one, then `vrsqrt` and a broadcast multiply.
        let sq: [f32; 3] = std::array::from_fn(|k| vu::mul(v[k], v[k]));
        let len2 = vu::madd(vu::add(sq[0], sq[1]), 1.0, sq[2]);
        let inv = vu::rsqrt(1.0, len2);
        for k in 0..3 {
            same(ee.f32_at(out + 4 * k), vu::mul(v[k as usize], inv),
                 format_args!("normalize({v:?})[{k}]"));
        }
        same(ee.f32_at(out + 12), w, format_args!("normalize({v:?}).w — the masked lane must survive"));
    }
    println!("vec3 normalize: {N} vectors, bit-exact");
}

// --- a table lookup, against the dumbest possible implementation of the same thing ------------------

/// The keyframe search: given a time and a sorted table of `(time, value)` pairs, how many keys are at or
/// before that time.
///
/// This one earns its place by being the first thing here with a *loop* — a binary search whose exit
/// depends on the data, with two different return paths and three special cases outside the loop. Its
/// model is a linear scan, which is obviously correct in a way the binary search is not, and the oracle
/// is what makes "obviously correct" and "what the game does" comparable at all.
///
/// Tables are built fresh each round, so the cases include a one-entry table, queries below the first key
/// and past the last, and queries landing exactly on a key.
#[test]
fn the_keyframe_search_agrees_with_a_linear_scan() {
    let Some((mut ee, f)) = ready(&["keyframe_index_stride2"]) else { return };
    let search = f[0];
    let table = ee.scratch(0);

    // An empty table is its own branch, and the only one a generated table cannot reach.
    ee.call(search, &[table as u64, 0], &[1.0], BUDGET);
    assert_eq!(ee.v0(), 0, "search over an empty table must be 0");

    let mut rng = Rng(0xfeed_face_cafe_babe);
    for _ in 0..N {
        // Strictly increasing keys, with the value word in between set to something recognisable so a
        // stride bug reads nonsense rather than a plausible key.
        let n = 1 + rng.next() % 24;
        let mut keys = Vec::with_capacity(n as usize);
        let mut t = -100.0 + (rng.next() % 1000) as f32 / 8.0;
        for k in 0..n {
            keys.push(t);
            ee.set_f32(table + 8 * k, t);
            ee.set_f32(table + 8 * k + 4, -1.0e30);
            t += 1.0 / 64.0 + (1 + rng.next() % 512) as f32 / 64.0;
        }
        // Half the queries land exactly on a key or just off it; the rest are spread over the whole
        // range, including outside it at both ends.
        let x = match rng.next() % 4 {
            0 => keys[(rng.next() % n) as usize],
            1 => keys[(rng.next() % n) as usize] - 1.0 / 128.0,
            2 => keys[0] + (rng.next() % 2000) as f32 / 8.0 - 100.0,
            _ => rng.float(),
        };

        ee.call(search, &[table as u64, n as u64], &[x], BUDGET);
        let want = keys.iter().filter(|&&k| F(k) <= F(x)).count() as u64;
        assert_eq!(ee.v0(), want,
                   "search({x} = {:#010x}) over {n} keys {keys:?}: the game says {}, a linear scan {want}",
                   x.to_bits(), ee.v0());
    }
    println!("keyframe search: {N} tables, exact");
}
