# 1 — four-lane exact VU ops (FINAL)

Task F15a. 2026-11-02.

## What I did

- **`crates/ps2-float/src/vu.rs`, "four lanes at once"**: new `add4`, `sub4`, `mul4`, `madd4` and `msub4` on `V4`.
  Each is the scalar lane function (`add`, `mul`, …) rewritten without a branch:
  - `vu(x)` (exponent 255 → `±max`, denormal → `±0`) becomes an integer `min` of the magnitude against `max`'s
    pattern, then a mask that zeroes anything under the smallest normal.
  - `add`'s two-sum is unchanged: `s`, `bb`, `e`, `r = s as f32`, `over`. The step back toward zero is
    `bits − (over ≠ 0 ∧ r ≠ 0 ∧ (over > 0) = (r > 0))`, followed by `daz` as a mask.
  - `mul` is the exact f64 product, the convert, then `bits − (|r| > |x|)`, then `daz`.
  - Two branches are dropped because they can never be taken, so they are left out rather than masked:
    - `add`'s non-finite test: two clamped operands sum to at most `2·max`, which is finite in f64. When the
      convert rounds that past `max` to infinity, the step back turns infinity's pattern into `max`'s. That is
      the clamp, the same as before.
    - `chop`'s non-finite test for a product: `max²` is finite in f64.
  - `madd4`/`msub4` feed the product into the adder without a second `vu()`. A chopped product is already
    finite, and normal or zero, so `vu` would leave it as it is.
- **`Vf::{add, sub, mul, madd, msub, mul_q, add_q, opmula, opmsub}`** use the four-lane forms when `NATIVE` is
  off. `opmula`/`opmsub` shuffle their lanes into place first. `opmsub` puts the accumulator's `w` back after,
  and `opmula`'s `w` is `0·0 = +0`, the literal it had. The `native` paths are unchanged.
- The same methods are now `#[inline]` in both modes, so a chain of `Vf` ops in game-sim keeps its lanes in
  registers. That is worth 0.97 → 0.93 ms in 1P and nothing measurable in 2P. The SIMD itself is the gain.
- Not changed: the scalar `vu::add`/`mul`, `Lane`, and `Vu0`'s flag-setting ops (`add_e` and friends, which
  also carry the exact value for the MAC flags). The FPU's `F` is not changed either. The task was the `Vf`
  path.
- Codegen: on the release target (generic x86-64, so SSE2), `Vf::madd` compiles to 132 instructions with no
  branch: `cvtps2pd`, `addpd`/`subpd`/`mulpd`, `cmp*pd`, `cvtpd2ps`, `pand`/`por`/`pcmpgtd`. These are
  two-lane f64 ops, so a vector takes two of each. No `unsafe` and no intrinsics are used, so the crate keeps
  `forbid(unsafe_code)` and its no-dependency rule. wasm and arm64 compile the same Rust to their own vectors,
  or to scalar code.

I checked opentennis first. It takes ps2-float v0.3.1 from git (`hst-sim/Cargo.toml`), and its `vu0.rs` uses the
scalar lane functions. Neither has a vector path, so there was nothing to take.

## How it was proved

- **Lane against lane, `tests/vu.rs` `the_four_lane_ops_are_the_lane_functions_bit_for_bit`.** It runs 400,000
  vector triples through every changed method and compares each lane's bits with the scalar function's.
  - The inputs are drawn per lane from: random patterns (NaN, infinity, denormal), the edge patterns, near
    cancellations against the other operand (±4 ulp), exponents up to 60 apart, the overflow and underflow
    bands, and game-sized values.
  - I broke the kernel four ways to check the test. Three mutants failed it: the denormal mask, the mul step
    back's `>` changed to `>=`, and `daz`'s exponent mask. The fourth survived: dropping `r ≠ 0` from add's
    step back. That one is equivalent. With flushed operands, a sum that rounds to zero is exactly zero, so
    `over` is 0 anyway.
- **Against the original, `tests/oracle.rs` `the_vu_lane_arithmetic_matches_lane_by_lane`.** It now also checks
  `Vf::sub`, `Vf::mul` and `Vf::add` against the game's own `vsub`/`vmul`/`vadd` carriers: 120,000 lanes,
  bit-exact. Before this, the test checked only the scalar functions.
- **ps2-float**: every test is green in release (the 10 oracle tests ran, not skipped).
- **game-sim**: `cargo test --release -p game-sim --no-fail-fast` gives 1031 passed, 0 failed, 19 ignored,
  with this slot's (s1) `context/` captures. Many capture tests skip without their files, so this is a floor,
  the same as in F15.
- **Ride hash**: free06's headless ride gives `0x4501646f3f6ff996` on every SIMD run, 1P and 2P, SSE2 and
  x86-64-v3. That is the same hash as before and as F15's.

## Measurements

The desktop, `OPENMX_THREADS=1 openmx --headless --track free06 --players N --frames 1500 --timings`. The
figure is the session line's mean `sim` in ms a frame. The builds were run back to back in alternating
rounds. Load was 1.4–8.5, except round 3 at 16.7.

| build | 1P sim | 2P sim (quiet rounds) |
|---|---|---|
| exact (HEAD) | 1.26–1.30 | 2.12–2.20 |
| exact, four-lane `Vf` (this task) | 0.91–1.03 | 1.57–1.62 |
| native (F15) | 0.58–0.65 | 0.87–0.94 |
| exact, `-C target-cpu=x86-64-v3` | 1.23–1.24 | 2.06 |
| four-lane, `-C target-cpu=x86-64-v3` | 0.88–0.89 | 1.65 |

- The four-lane exact path costs about 0.74× of exact in both 1P and 2P. Native is about 0.46×, so this closes
  roughly half of the gap at no cost to exactness.
- AVX2 (`x86-64-v3`, which the Deck's Zen 2 has) adds only about 5% more. It is not worth a second build
  target. The release builds stay generic x86-64.
- The noisier 2P rounds (loads 1.4 and 16.7, exact 2.54–3.37) kept the same order: four-lane at 0.79–0.84×
  exact, native at 0.31–0.67×.
- These numbers are lower than F15's (exact 1P 4.4) because F15 timed without `OPENMX_THREADS=1` on a busier
  machine. Only the ratios carry over.
- What is left of the gap is mostly the scalar paths: `F` (the FPU, about 6,600 uses in game-sim against about
  2,900 for `Vf`) with its short adder's `align` and chop branches, plus the scalar `vu::` and `Lane` ops. I
  filed this as a follow-up.

## Not verified / not 1:1

- **No Deck numbers.** `ssh deck` timed out, as it did in F15. The ratios above are
  desktop ones. The Deck's Zen 2 runs the same SSE2 code, but its own figure needs the Deck awake. plan/HUMAN.md's
  F15 section has the command, and it applies to this build too.
- **openmx's own test suite was not run.** Its ride-hash asserts read the same sim, and the headless ride hash is
  unchanged in 1P and 2P, but the suite itself did not run.
- **The `native` feature's ps2-float tests stay red**, as they were in F15 (the exact-model tests). The new lane
  test is `cfg`'d out under `native`. There, both sides are the host's ops, and only a NaN's payload differs.
- **Exhaustive proof**: none. The lane test is 400k random draws aimed at the branch cases, not every input pair.
  The argument that the two forms are the same is in the code comments: the same IEEE ops in the same order,
  with only unreachable branches dropped.
