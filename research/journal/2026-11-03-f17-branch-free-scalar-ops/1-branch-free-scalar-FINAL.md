# 1 — branch free scalar (FINAL)

Task F17. 2026-11-03.

## What I did

- `vu.rs`:
  - New kernels:
    - `vu_bits`: operand clamp and DAZ on a pattern, with no branch.
    - `chop_bits`: nearest `f32`, then one step back when that rounded away, then DAZ.
    - `add_bits`: TwoSum, with the step back as a mask.
    - `mul_bits`: the exact f64 product, then `chop_bits`.
  - The scalar `add/sub/mul/madd/msub` now use these kernels, and so does `Lane` through the macro. The
    four-lane `add4_bits`/`mul4_bits` are now `from_fn` over the same one-lane kernels.
  - Everything on the hot path is `#[inline]`, so it inlines across crates. Before, only the native build
    inlined.
  - `vu()`/`chop()`/`daz()` stay for div, sqrt, ftoi, itof and the flag ops, which were not in scope.
- `fpu.rs`:
  - `align` (a branch on `d`) is replaced by `align_keep(d)`, a mask. For `d ≤ 0` it is all ones. For
    `d ≥ 1` the low `d−1` bits are cleared. For `d ≥ 25` only the sign is left.
  - `add_ee` masks both operands (`align_keep(-d)` on one, `align_keep(d)` on the other), adds in f64,
    and chops.
  - `mul` is `vu::mul_bits`, the same operation.
  - `madd`/`msub` feed the product's bits straight into `add_ee`.
  - The `cfg_attr(feature = "native", inline)` attributes are now plain `#[inline]`.
- Tests:
  - `tests/fpu.rs`: the integer-model test also covers F-type `+ - *`, `sub(x, |y|)` and `add(x, -|y|)`.
  - `tests/vu.rs`: the old branchy scalar ops are kept as `mod reference`. The four-lane test and the new
    per-lane assertions (scalar ops, and `Lane` `+ - * madd msub`, including known-sign operands) compare
    against `reference`.
- I also tried a truncation `chop_bits` (mask off the f64's low 29 mantissa bits, then convert). It was
  correct but no faster on the microbench, so I took it out.

## The LLVM miscompile

The first branch-free `vu_bits` was written as `(b & 0x8000_0000) | min(b & 0x7fff_ffff, MAX)` plus a
DAZ mask. It made `the_smaller_addend_keeps_exactly_one_guard_bit` fail, but only in release: a sub at
`d = 25` gave `-3.4e38`.

How it was found:
- `-C llvm-args=-opt-bisect-limit` put the fault in the SLP vectorizer pass, running on the caller.
  First, InstCombine has turned the `& 0x7fffffff` into an `fabs`. The caller is a sub of a value already
  known to be positive, so the operand's sign bit is set.
- In the x86 output the `umin` clamp is gone: a register is preset to `MAX` and only ever zeroed.
- I saw this on rustc 1.99.1, both the Arch build and the upstream rustup build, at opt-level 3, and at
  opt-level 2 in a standalone repro crate. Debug builds are right.

The fix: take the magnitude as `b << 1`, clamp and DAZ it there, then shift it back. With the old form put
back, the release fpu tests fail ("sub of |b|" in the integer-model test, and the guard-bit test). That
means the fpu tests are the regression test for this. The vu lane test did not catch it.

## How it was proved

- **FPU:** `tests/fpu.rs` `add_sub_and_mul_match_an_integer_model` checks against an integer model written
  without f64. It covers the edge patterns and 40k×2 random pairs per op, now including the F-type and
  known-sign cases. Green in release and at the test profile.
- **VU:** `tests/vu.rs` compares 400k random vector draws, per lane and four-lane, with `reference` (the
  pre-F17 code). All 19 tests are green in release.
- **Oracles:** `tests/oracle.rs`, 10 tests, run against the game's code. Green with no skip.
  `./kit check -- -p ps2-float`: 62 passed.
- **game-sim:** `cargo test --release -p game-sim --no-fail-fast` gave 1115 passed and 2 failed:
  - `the_humans_tricks`: the known C6 red.
  - fly_and_collide's `the_drawn_rider_leans_as_the_original_through_the_turns`: DRAWN_TICKS
    (1793, 282) where (1793, 280) is expected. HEAD fails it the same way in this slot, with the same
    output.
- **Ride hash:**
  - free06, paced, 1500 frames: `0x4501646f3f6ff996` in 1P and 2P, for both HEAD and F17.
  - Unpaced, 1800 frames: exact `0xc0d5b0aa579bd109` for both HEAD and F17.

## Measurements

These were taken on the desktop, which is shared and noisy. The runs were unpaced:
`--headless --steady-clock --draw-fps 30`, 10 interleaved rounds. Paced `--timings` numbers are inflated
by the idle gaps between frames, so I don't use them. The load average was 2.2–3.4 in rounds 1–5.

Sim ms per frame, rounds 1–5 and the best of all rounds:

| build | 1P | best | 2P | best |
|---|---|---|---|---|
| HEAD | 2.80 2.78 3.15 2.84 3.09 | 2.78 | 6.13 5.87 6.05 5.91 6.30 | 5.87 |
| F17 | 2.72 2.79 2.98 2.61 2.72 | 2.61 | 5.54 5.52 6.65 5.50 5.61 | 5.50 |
| native | 1.71 1.76 1.86 1.72 1.75 | 1.71 | 4.26 5.32 4.04 3.98 4.31 | 3.82 |

Microbench (a dependent chain, ns per iteration, old crate against new): fpu 26.2 → ~25–26.7 (within
noise); vu 48 → 32.

So the ops themselves got faster, but they were a smaller share of exact sim than F15a guessed. `nm` shows
almost every fpu/vu op inlined now; HEAD had out-of-line global symbols for them.

## Not verified / not 1:1

- No proof over every input: it is random and edge-pattern sampling against two independent models.
- No Steam Deck numbers. Desktop only, and noisy.
- openmx's own test suite was not run. Only ps2-float and game-sim were run, and the ride hash.
- The `native` feature compiles, but its tests were not run. They are red by design (F15).
- The drawn-lean failure in fly_and_collide was there before F17 (HEAD fails it the same way in this
  slot). It was not looked into.
- The LLVM bug was not reduced to an upstream report. The workaround depends on the bug not also
  matching the `<< 1` form, and only the release fpu tests guard that.
- Div, sqrt, ftoi, itof and the flag ops are still branchy. They are out of scope.

## Next

Nothing for this task.
