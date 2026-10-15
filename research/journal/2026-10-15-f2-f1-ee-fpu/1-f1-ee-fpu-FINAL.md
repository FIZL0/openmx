# 1 — madd.s, msub.s and the accumulator (FINAL)

Task F2. 2026-10-15. Continues `2026-10-15-f1-f1-ee-fpu/`, same file, same module.

## What I did

F1 left `madd.s` arithmetically right and unproved: `madd(acc, a, b) = add(acc, mul(a, b))`, which is
the recompiler's `MULSS` into a register followed by its correct add/sub, so the product *is* truncated
to a single before the adder sees it. F1's own "Next" section named what was missing, and that is what
this task did:

**1. The accumulator is now a type, not a convention.** `fpu::Acc` is `ACC`, the FPU's one accumulator
register: `Acc::mula` / `adda` / `suba` open it, `madda` / `msuba` accumulate into it, `madd` / `msub`
read it and hand back an `f`-register value. So the chain a compiler emits for a dot product ports
instruction for instruction:

```rust
// mula.s f1, f4 ; madda.s f2, f5 ; madd.s f0, f3, f6
let d = Acc::mula(x0, y0).madda(x1, y1).madd(x2, y2);
```

Nothing in it rounds differently from the free functions — `madda` *is* `madd`, and the accumulator
holds an ordinary single — the type exists so a reviewer can line a ported chain up against the
instructions, and so an accumulator cannot be passed where an `f`-register is meant.

**2. Two instructions that the module documented but did not have.** `adda`'s doc comment claimed
"`adda.s` / `suba.s` / `mula.s`", and only `adda` existed. `suba` and `mula` are now there.

**3. `mula.s` is not `adda.s $f0` followed by `madd.s`.** Opening a dot product the wrong way costs a
sign of zero: `mula(-1.0, 0.0)` is `-0`, where putting the first term through the adder (`0 + (-0)`)
gives `+0`. A sign of zero reaches a port's `copysign`, its `min`/`max` and its divisions, so `fpu::dot`
(and `F::dot`) — the `mula.s`/`madda.s` chain in index order, spelled out once so call sites do not each
re-derive it — opens with `mula`.

**4. One of F1's assertions could not fail.** `tests/fpu.rs` had
`assert_ne!(fpu::madd(acc, a, b), acc.mul_add(a, b), "and it is not the host's FMA either")`. Rust's
`mul_add` is `self * a + b`, so that compared `madd(acc, a, b)` against `acc·a + b` — a different
expression, unequal for trivial reasons, and no statement about fusing at all. With the arguments the
right way round (`a.mul_add(b, acc)`) the two are in fact *equal* for those inputs, which is why the
weak form survived. It now uses a product of three quarters of the accumulator's last bit, where the EE
returns `1.0` and an FMA returns `1.0 + 1 ulp`.

## What the original does

Nothing in this task came from the game's own code: like F1, this is the machine, not the program. Which
of these instructions MX Unleashed's compiler actually emitted, and in what order at each site, is
R-phase and S-phase work — and it matters, because the order is part of the value (see the dot-product
test below).

What is being matched is the reference emulator's recompiler, as transcribed in F1's table: `recMADDtemp`
in `iFPU.cpp` multiplies into an XMM register (one chop to `f32`, in `FPUFPCR`'s round-toward-zero mode,
which is also why an overflowing product saturates to `±max` instead of becoming an infinity) and then
calls `FPU_ADD` / `FPU_SUB`, i.e. `FPU_ADD_SUB`'s one-guard-bit alignment and the same chop again. Two
roundings, product first.

## How it was proved

All of it in `crates/ps2-float/tests/fpu.rs`, run by `./kit check` (73 tests pass, up from 66 at F1 —
five new tests and two new doctests). No emulator and no console: O1/O2 are still the tasks that turn
any of this into a proof against the real code (D1 is blocked on a BIOS dump).

- **`madd.s`, `msub.s`, `madda.s`, `msuba.s` against an integer model, over 77,080 input triples ×
  4 instructions.** `mod int_acc` composes F1's `int_model` — bit patterns in, bit patterns out,
  integer arithmetic only, no host float anywhere in it — into the chopped product and the aligning
  add. The triples: 17,080 built from the 61 awkward accumulator patterns against all 56 awkward
  product operands (both zeroes, both denormal ends, both normal ends, the infinity and NaN encodings,
  single-bit and all-ones mantissas at five exponents) and five multipliers; 30,000 uniformly random
  triples; and 30,000 where the product is *placed* a random −27..27 exponents from the accumulator,
  which is where the adder's cliff is and where uniform random triples never land. The test asserts its
  own input count, so the figures above cannot quietly stop being true.
- **The two wrong answers, counted on the same inputs**, so the test fails if they stop being
  reachable: 15,108 of the triples distinguish `madd.s` from a true FMA (`f32::mul_add`, one rounding),
  4,313 distinguish it from *the same chop applied once at the end* instead of to the product as well,
  and 66,607 of the products needed rounding at all. (Those two comparisons are counters, not models:
  the host's FMA is exactly specified, but the "rounded late" value is computed through an `f64` sum
  that itself rounds, so it answers "these are not the same function" and nothing finer.)
- **The truncation, in four exact worked cases** derived by hand before they were run, not read off the
  implementation:
  - `(1 + 2^-23)²` is `1 + 2^-22 + 2^-46`; the chop drops the last term, so with an accumulator of
    `−(1 + 2^-22)` the EE cancels to **exactly zero** where the real answer, `2^-46`, is a perfectly
    representable single. A port that rounds late never cancels and carries a residual for ever; an FMA
    agrees with the residual, not with the PS2.
  - `1 − 2^-23·(1 + 2^-23)`: here the product is exact (one factor is a power of two), so what is lost
    is the *alignment*, not the chop — the adder drops the operand's low bit and lands on `1 − 2^-23`,
    one ulp above what rounding late gives.
  - the product saturates on its way to the adder (`madd(−max, max, max)` is zero, not a NaN), and a
    product that underflows is gone before the add, sign and all.
  - where neither rounding has anything to do, rounding late gives the same value — so the two
    roundings are the whole of the difference.
- **The accumulate forms and the register wrapper, over 20,000 random triples**: `madda`/`msuba` equal
  `madd`/`msub` bit for bit, `adda`/`suba`/`mula` equal `add`/`sub`/`mul`, every `Acc` method equals the
  free function it names, reading the accumulator back is a move and not an operation, and `madd.s`
  leaves the accumulator alone where `madda.s` replaces it.
- **The dot-product chain, over 20,000 four-term dot products** with terms a few exponents apart (a
  normal against a direction, a matrix row against a vertex): `fpu::dot` and `F::dot` are the explicit
  `mula`/`madda`/`madd` chain bit for bit, and the chain differs from the host's own arithmetic on
  15,423 of them, from an FMA chain on 17,108, and **from the same four terms summed backwards on
  9,891** — order and grouping are part of the answer, which is why `dot` fixes one order and says so.
- **The drift, as a property**: a thousand `madd.s` steps of `x += v·dt`. Every chop is toward zero, so
  at every step the EE's running total is at or below the host's *and* below the exact sum, and the gap
  at 1000 steps is more than 1.5× the gap at 500 — it accumulates instead of cancelling. The same loop
  through `Acc::madda` drifts identically. This is the shape of the bug this task exists to prevent.

## Not verified / not 1:1

- **Nothing here has been run against the PS2, or against PCSX2.** As at F1: the proofs are against an
  integer model that shares no code with the one under test, against host operations IEEE 754 specifies
  exactly, and against hand-derived cases. **O2 is the task that makes this a proof**, and `madd.s` with
  an accumulator belongs in its input set alongside the two-operand ops.
- **The composition is F1's transcription of the reference, not a fresh read.** There is no PCSX2 source
  tree in this worktree and no network, so "`recMADDtemp` = `MULSS` then `FPU_ADD_SUB`" is taken from
  F1's table rather than re-read this session. Everything else in this entry is proved against that
  hypothesis, not evidence for it. If one thing in this module gets checked against the real code first,
  it should be this.
- **Under PCSX2's *interpreter* `madd.s` is effectively fused.** `MADD_S` in `FPU.cpp` multiplies and
  adds in `double` with no mask and no intermediate chop, so it rounds once, late — which is precisely
  the 4,313-in-77,080 difference counted above. This module follows the recompiler, because that is what
  runs the game; a capture taken with the interpreter would disagree with the port.
- **The FPU multiply hack is not modelled.** `CHECK_FPUMULHACK` makes `mul.s` a per-game lookup in the
  reference emulator; it is off by default and MX Unleashed is not on its list, but if it were ever
  switched on, every `madd.s` would change with it.
- **The accumulator is modelled as an ordinary single**, with no extra width and no separate rounding on
  the write to `ACC` — which is what both PCSX2 paths do (`ACC` is a 32-bit register there) and what the
  EE manual describes, but it is not verified against silicon. If the hardware's accumulator kept guard
  bits, `madda.s` chains would be the place it showed up, and nothing in this task would catch it.
- **`dot`'s order is the usual compiler output, not this game's.** It is `mula.s` on term 0 then
  `madda.s` in index order; a particular call site in MX Unleashed may interleave the terms differently
  (and a different order is a different `f32` — 9,891 of 20,000 above). Port each site from its own
  disassembly; `dot` is for the sites that match it.
- **`madda.s`/`msuba.s` are assumed to differ from `madd.s`/`msub.s` only in their destination**, per the
  reference; the 20,000-triple test above pins the model to that assumption rather than confirming it
  against hardware.
- Inherited from F1 and unchanged here: Inf/NaN *operands* are pre-saturated where the recompiler does
  not pre-clamp them; `±0` out of `max.s`/`min.s` is the interpreter's answer; `mul.s`'s Booth/Wallace
  truncation on real silicon is not modelled, and `madd.s` inherits all of that through its product.

## Next

The F-phase arithmetic for the FPU is now as proved as it can get without the real code. Next in the
phase is **F3** (VU0 macro mode) — which has the bug F1 found and left: `vu::sqrt` documents itself as
chopped and computes `chop(sqrtf(x) as f64)`, a no-op, so VU0's `vsqrt`/`vrsqrt` are round-to-nearest
today. F3 should also decide whether VU0's `vmadd` chops its product the way the FPU's does; the VU's
adder is full-width, so the answer is not automatically the same one this task proved.
