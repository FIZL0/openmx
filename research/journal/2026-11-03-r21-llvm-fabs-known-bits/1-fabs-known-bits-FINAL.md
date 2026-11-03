# 1 — fabs known bits (FINAL)

Task R21. 2026-11-03. Follow-up to F17 (`2026-11-03-f17-branch-free-scalar-ops`).

## Result

The miscompile is in LLVM 23's target-independent SelectionDAG. It is not in the SLP vectorizer and not
in the x86 backend:

```
case ISD::FABS:
  // fabs clears the sign bit
  Known = computeKnownBits(Op.getOperand(0), DemandedElts, Depth + 1);
  Known.makeNonNegative();   // = Known.Zero.setSignBit(); Known.One keeps its sign bit
```

When the operand's sign bit is known to be 1, the `fabs` result's sign bit ends up in both `Known.Zero`
and `Known.One`. That is a conflicting state, and `KnownBits::umin` then takes the result's minimum as
≥ 0x80000000, so `umin(fabs bits, C)` folds to `C`. In F17 that was `vu_bits`'s clamp: the magnitude
came out as `max`, so `2 + (−tiny)` gave `−max`.

- Introduced: llvm-project 11bc6e8776d2, "[SelectionDAG] Add known bit for ISD::FABS" (#188335,
  2026-03-24). It is first in llvmorg-23.1.0; 20.1.0, 21.1.0 and 22.1.0 have no FABS case there.
- Fixed: 6bb86c2af2ba, "[SelectionDAG] Clear Known.One sign bit for ISD::FABS in computeKnownBits"
  (#227457, 2026-09-30, merged to main). It also adds a unit test.
- Not fixed yet in:
  - llvm release/23.x (still `makeNonNegative`);
  - Rust's fork, rustc/23.1-2026-07-22;
  - the llvm-project submodule of rust-lang/rust on main (nightly) and beta (both 0a5579eb), or on
    stable (1b9c0d5f).
  - Every rustc available today has the bug: the Arch 1.99.1 (LLVM 23.1.3) and the rustup 1.99.1
    (LLVM 23.1.1). Both fail the repro.
- `gh search` found no rust-lang/rust issue and no 23.x backport PR for it (2026-11-03).

## The reduction

`research/r21/clamp_magnitude.rs`: two lines of Rust, wrong at `-O` and right without it (both rustcs).

```rust
let neg = f32::from_bits(x | 0x8000_0000); // sign bit known set
neg.abs().to_bits().min(0x7f7f_ffff)       // gives 0x7f7fffff for every x
```

`research/r21/fabs_known_bits.ll` has five IR instructions (`or` sign, `bitcast`, `fabs`, `bitcast`,
`umin`), plus the two-lane shape F17's build had. `llc -O2` folds both to the constant on x86-64,
AArch64, RISC-V and wasm32. `llc -O0` (FastISel) and `llc -global-isel` keep the `and`/`cmp` and are
right.

Findings along the way:
- `lshr 31` of the same `fabs` gives 0 (right), and `icmp ult …, 0x80000000` keeps a real test (right).
  So only consumers that fold through `KnownBits` min/max show it, here `umin`.
- `fneg` then `fabs` does not trigger it: the `fneg` folds away first.
- A splat of negative denormals, or `-1.0` with an all-`max` bound, came out right in the all-constant
  version. So whether it fires depends on which combine runs first, and the variable-input `.ll` above
  is the dependable form.

How it was reduced:
1. The F17 test (`the_smaller_addend_keeps_exactly_one_guard_bit`'s loop, the old `vu_bits`) as a
   no_std `extern "C"` function. Its pre-optimisation IR, through the system `opt -O3`, gives
   `d=25/26 → 0xff7fffff`.
2. `opt -opt-bisect-limit` puts the change at pass 374, `slp-vectorizer` on the caller, as F17 found.
   But the IR after SLP is correct. Compiled at `clang -O0` it still gives the wrong answer, so the fault
   is in codegen.
3. `llvm-reduce` on (pre-SLP IR, `opt -passes=slp-vectorizer`, output differs) gave 40 lines.
4. Then by hand: `llc` on the post-SLP IR, then the three-node function, then the grid of lane
   values. That showed the trigger is a known-set sign bit, and `computeKnownBits(FABS)` in the 23.1.3
   source is the line above.

SLP's part is only that it reshaped the DAG. The scalar epilogue before SLP has the same
`umin(fabs(x | sign))`, and it came out right. I did not trace which combine order makes the vector form
fold.

## Is anything else in the port hit?

The fold needs an `fabs` whose result is read as integer bits, and whose operand's sign bit the DAG knows
to be 1.
- InstCombine turns `f.to_bits() & 0x7fff_ffff` into `bitcast(fabs f)`. So `fpu::abs`, `F::abs` and any
  hand-written sign mask become `fabs`.
- `x.to_bits() << 1` stays integer (checked in the optimised IR). So `vu_bits`'s workaround never builds
  an `FABS` node, and the bug cannot reach it.
- I built game-sim's lib in release as LLVM IR (`cargo rustc --release -p game-sim --lib -- --emit=llvm-ir
  -C codegen-units=1`, own target dir) and scanned it (`scan` below):
  - There are 4,867 `fabs` calls, and 23 of them have a result read back as bits.
  - None of those 23 has an operand whose sign bit is set by a constant. 14 are `daz_bits` outputs
    (`and v, select(c, 0x80000000, -1)`, which keep `v`'s sign, so it is unknown). The rest are loads,
    arguments, or the result of `fpu::min`.
  - So no other case was found in game-sim.

## For the user: the backport request (not filed)

Filing on GitHub publishes under the user's account, so I did not do it. Text to file at
rust-lang/llvm-project (or rust-lang/rust with label A-LLVM):

> **Miscompile: umin of fabs bits folds to the constant when the operand's sign is known (LLVM 23) —
> please cherry-pick llvm/llvm-project#227457**
>
> ```rust
> #[inline(never)]
> fn clamp_magnitude(x: u32) -> u32 {
>     f32::from_bits(x | 0x8000_0000).abs().to_bits().min(0x7f7f_ffff)
> }
> fn main() { assert_eq!(clamp_magnitude(std::hint::black_box(0x3f80_0000)), 0x3f80_0000); }
> ```
> Fails with `-O` on rustc 1.99.1 (LLVM 23.1.1/23.1.3), passes in debug. The function compiles to
> `mov eax, 0x7f7fffff`. Cause: `SelectionDAG::computeKnownBits(ISD::FABS)` uses
> `KnownBits::makeNonNegative()`, which leaves `Known.One`'s sign bit set, giving a conflicting
> KnownBits. Regressed in llvm/llvm-project#188335 (LLVM 23), fixed on main by
> llvm/llvm-project#227457 (6bb86c2af2ba), not on release/23.x or rustc/23.1-2026-07-22. IR repro:
> `or i32 %x, 0x80000000` → bitcast → `llvm.fabs.f32` → bitcast → `llvm.umin.i32(…, 0x7f7fffff)`,
> `llc -O2` on any target.

## Commands

```
rustc -O research/r21/clamp_magnitude.rs -o /tmp/r21 && /tmp/r21   # panics: 0x7f7fffff
llc -O2 research/r21/fabs_known_bits.ll -o -                       # both functions: constant
llc -O0 research/r21/fabs_known_bits.ll -o -                       # real and/cmp/cmov
```

`scan`: a Python pass over the `.ll` that maps each `llvm.fabs` result to a `bitcast … to i32` and prints
the operand's defining chain, three steps deep. It is not kept, since it is a one-off.

## Changes

- `crates/ps2-float/src/vu.rs`: `vu_bits`'s doc comment now names the real cause (fabs known bits, all
  targets) and the upstream fix, in place of "its x86 backend". No code changed.
- `research/r21/`: the two repro files.

## Not verified / not 1:1

- The upstream fix was not tried against our repro: no LLVM 24 build or patched rustc is here. That it
  fixes this case comes from reading the source (it is exactly the missing `One.clearSignBit()`). I also
  compared the 23.1.3 and main source of that one `case`.
- I did not find which DAG combine folds the `umin`. `KnownBits::umin` on the conflicting state is the
  likely path, but I did not step through it.
- The scan covers game-sim's lib only, built with `codegen-units=1`. It does not cover openmx, ps2-float's
  own uses, or code inlined across crates into the final binary. It is a syntactic check on the IR: a
  sign bit the DAG learns through deeper context (phis, other blocks) would not show up.
- `isKnownNeverNaN` and other users of the same conflicting KnownBits were not checked. Since #188606
  (2026-04) LLVM reads KnownBits for NaN tests, so an `fcmp` could also be affected in principle.
- The backport request was not filed (see above). Whether the workaround can go depends on a rustc whose
  LLVM has 6bb86c2af2ba. Revert it then and run the release fpu tests: they are the regression test.
