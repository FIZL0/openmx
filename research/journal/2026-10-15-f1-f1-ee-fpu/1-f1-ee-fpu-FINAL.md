# 1 — the EE FPU (FINAL)

Task F1. 2026-10-15.

## What I did

`crates/ps2-float/src/fpu.rs` arrived with the kit already modelling the EE FPU, so this task was not
"write it" but "own it": check every rule in it against the reference emulator's own source, fix what was
wrong, and replace "I read it carefully" with a proof.

**Read the reference, not a description of it.** PCSX2 has two EE FPU implementations and they do not
agree with each other, so the first job was deciding which one the port is matching. From the upstream
sources (`pcsx2/FPU.cpp` — the interpreter; `pcsx2/x86/iFPU.cpp` — the recompiler; `pcsx2/Pcsx2Config.cpp`
— the defaults):

| | interpreter (`FPU.cpp`) | recompiler (`iFPU.cpp`, the default) |
|---|---|---|
| rounding | the host's, in `double` | MXCSR from `FPUFPCR`: chop, DAZ, FTZ |
| adder alignment | **none at all** | `FPU_ADD_SUB`: the one-guard-bit mask |
| `div.s` | `double` divide | MXCSR from `FPUDivFPCR`: **nearest** |
| `sqrt.s` | `double` sqrt | mode forced to **nearest** for the instruction |
| `rsqrt.s` | one `double` divide of a `double` root | `SQRTSS` then `DIVSS`, both in `FPUFPCR` → **chopped** |
| `x / 0` | `±max`, the two signs xored | same |
| `rsqrt(x, 0)` | `±max`, sign of the **divisor** | `±max`, sign of the **dividend** |
| operand with exponent 255 | saturated to `±max` (`fpuDouble`) | not touched (`fpuExtraOverflow` off by default) |
| result Inf/NaN | `±max` / `+max` (`checkOverflow`) | same (`ClampValues`, `fpuOverflow` on by default) |
| `max.s` / `min.s` | signed-integer compare of the raw patterns | `MAXSS` / `MINSS` |

The defaults that matter, from `Pcsx2Config.cpp`: `FPUFPCR` = DAZ + FTZ + chop, `FPUDivFPCR` = the same
with round-to-nearest, `fpuOverflow = true`, `fpuExtraOverflow = false`, `FPU_CORRECT_ADD_SUB = 1`. So
the recompiler is the one the game runs under, and the module now says in its own docs that it follows
the recompiler and where the interpreter differs.

**What was already right** (checked rule by rule against `iFPU.cpp`, not assumed): the alignment mask is
`0xffffffff << (d - 1)` for an exponent difference of 1..24, sign-only at `d >= 25` and nothing at
`d == 0`, exactly as `align` has it — including that the mask covers the mantissa only and that `sub.s`
is the same mask with the operand negated, since a sign flip cannot change an exponent. `chop`'s
truncation, `operand`'s exponent-255 and denormal cases, the `±max` saturation in place of an infinity,
the flush of a denormal result, `div.s` rounding to nearest with `±max` on a zero divisor (signs xored,
`0/0` included), `sqrt.s` rounding to nearest and ignoring the sign, `madd.s` being two roundings, and
`cvt.w.s` turning over at an exponent field of `2^30` (i.e. `|x| < 2^31` converts, everything else
saturates by sign) are all as the reference has them.

**Three things were wrong.**

1. **`rsqrt.s` rounded to nearest twice.** The recompiler emits `SQRTSS` + `DIVSS` without touching the
   rounding mode, and `sqrt.s`'s switch to nearest is specific to `sqrt.s` — so both steps of `rsqrt.s`
   chop. Added `sqrt_chop` and `div_chop` (each steps the host's correctly-rounded result one ulp inward
   when an exact `f64` check says it rounded outward — a 24-bit significand squared, or a quotient times
   its divisor, is exact in `f64`, so nothing is rounded in deciding), and `rsqrt` now uses them. It
   moves the result by one ulp either way: *down* from the chopped divide, *up* when the chopped root
   makes the divisor smaller. Roughly a third of random inputs differ from the old value.
2. **`rsqrt.s` with a zero divisor took the xor of the signs.** The instruction takes `|b|` before it
   looks for zero, so the divisor's sign cannot reach the result: it is `±max` with the sign of the
   dividend alone. `div.s` is the one that xors. (The interpreter does the opposite of both; see the
   table.)
3. **`max.s` / `min.s` picked the wrong zero.** They are an integer compare of the bit patterns in the
   reference, which resolves `+0` against `-0` to `+0` for `max.s` and `-0` for `min.s` whichever way
   round the operands come; the old float compare returned whichever operand came first, so
   `max(-0.0, 0.0)` was `-0.0`. Now the same signed-integer compare (`fp_max` / `fp_min`), on operands
   still read through `operand`.

Also: `crates/ps2-oracle/src/ee.rs` had the zero-divisor rule for `rsqrt.s` written out a second time,
with the old xored sign. That branch is now gone — the opcode calls `fpu::rsqrt` and the rule lives in
one place. (One line, in a file this task does not otherwise touch, because leaving the oracle
contradicting the model it exists to check is worse than the small diff.)

## What the original does

Nothing in this task came from the game's own code: an FPU model is about the machine, not the program.
The game's use of it — which of these instructions the compiler actually emitted, and where — is R-phase
and S-phase work.

## How it was proved

No console and no emulator: D1 is blocked on a BIOS dump, so the oracle (O1) and its float coverage (O2)
cannot run yet. Everything below is therefore a proof against *models that share no code with the one
under test*, which is the strongest thing available today. All of it is in
`crates/ps2-float/tests/fpu.rs`, run by `./kit check` (66 tests pass).

- **An independent integer model of `add.s`, `sub.s` and `mul.s`** (`mod int_model` in the test file):
  bit patterns in, bit patterns out, integer arithmetic only — no host float operation anywhere in it, so
  it shares neither code nor rounding with `fpu.rs`'s `f64` path. It applies the operand fix-up, the
  alignment mask, an exact integer sum or product, truncation of the significand to 24 bits, flush-to-zero
  and `±max` saturation. Compared against the model over **83,136 input pairs × 3 operations**: 3,136
  pairs built from 56 awkward patterns (both zeroes, both denormal ends, both normal ends, the infinity
  and NaN encodings, single-bit and all-ones mantissas at five exponents), 40,000 uniformly random
  pattern pairs (infinities and NaNs included) and 40,000 pairs drawn a random exponent *difference of
  −27..27* apart, which is where the adder's cliff is and where uniform random pairs never land. The test
  also counts how many of those inputs the chop actually changed and fails if it is under 1,000, so it
  cannot quietly stop exercising the thing it is checking.
- **The guard-bit rule, at every exponent difference 1..26**: with an addend whose mantissa is all ones,
  clearing exactly the bits the hardware drops changes nothing, in both operand orders and for both
  `add.s` and `sub.s`; an addend of nothing but its leading bit still moves a subtraction right up to
  `d = 24`, and from `d = 25` the whole operand is gone whatever its mantissa.
- **`div.s` and `sqrt.s` against the host**, over **60,000 random patterns**: IEEE 754 requires `/` and
  `sqrt` to be correctly rounded, so the host is an independent oracle for the two operations the EE
  rounds to nearest — on every platform, by specification. The test also asserts the zero divisor, the
  saturating and the flushing cases, and fails if fewer than 100 inputs hit each of the last two.
- **`rsqrt.s`'s two chops, exactly**, over 40,000 draws (>10,000 landing in the normal range): the root
  and the quotient are each pinned by the two inequalities that define a truncated result, checked in
  `f64` where both `r·r` and `q·b` are exact — so the expected value is derived, not reimplemented. The
  test requires at least a quarter of the inputs to differ from the nearest-rounding answer, and records
  two worked cases as literals (one where the divide's chop takes the result down, one where the root's
  chop takes it up).
- **`max.s` / `min.s`** over 50,000 random pattern pairs against a float compare of the same operands,
  with the signed-zero cases asserted explicitly; **`cvt.w.s` / `cvt.s.w`** at the `2^31` turnover and
  over 20,000 round trips below `2^24`.

## Not verified / not 1:1

- **Nothing here has been run against the PS2.** Every proof above is against the reference emulator's
  *source* (read and transcribed by hand) or against host operations that IEEE 754 specifies exactly. The
  real console is unreachable until D1 (BIOS) and O1/O2 land; **O2 is the task that turns this into a
  proof**, over the awkward inputs its line already names.
- **Where PCSX2 is known to differ from the silicon**, this module follows PCSX2 and so does not model:
  `mul.s`'s Booth/Wallace truncation (the comment in `iFPU.cpp` says the hardware's mantissa is sometimes
  one *smaller* than the chopped product, and that the hardware's multiply is not even commutative);
  `div.s` and `sqrt.s` rounding, which the EE manual describes as chop for every FPU operation where
  PCSX2 defaults the divider and the root to nearest. If a later capture disagrees with the port on a
  divide, this is the first place to look.
- **Inf/NaN *operands* are saturated to `±max` before use** (`operand`), which is the interpreter's
  behaviour and the hardware's intent; the recompiler does not pre-clamp operands at its default
  settings, so for values a game *loads from data* with an exponent field of 255 the two disagree — and
  the alignment mask then sees exponent 254 here against 255 there. Nothing the model itself produces can
  have exponent 255, so this only matters for patterns read out of RAM or a file.
- **`±0` from `max.s` / `min.s` is the interpreter's answer.** The recompiler's `MAXSS`/`MINSS` return
  whichever operand the register allocator happened to pass second, so there is no single right answer to
  match; the integer compare at least gives the same one every time and agrees with the interpreter.
  Likewise `sqrt(-0.0)` is `+0` here (the recompiler takes the magnitude before the root) where the
  interpreter returns `-0`.
- **`rsqrt.s`'s zero-divisor sign** follows the recompiler (sign of the dividend). The interpreter uses
  the divisor's sign. Only reachable with a zero or denormal divisor.
- **Found, not fixed, because it belongs to F3:** `vu::sqrt` in `crates/ps2-float/src/vu.rs` documents
  itself as chopped but computes `chop(sqrtf(x) as f64)`, and chopping a value that is already an `f32`
  is a no-op — so VU0's `vsqrt` is currently round-to-nearest. `vu::rsqrt` inherits it. F3 should decide
  what VU0 really does (the VU's divider is a different unit again) and fix both.
- The `f32`→`f64`→`f32` path in `chop` is exact for every operation here (an aligned sum needs 26 bits, a
  product 48, both well inside `f64`'s 53) and `div_chop`/`sqrt_chop`'s exactness arguments are written
  out at their definitions — but that exactness is an argument, not a test. The integer model above is
  the test of it for `add`/`sub`/`mul`.

## Next

F2 is the next line in this phase and touches the same file: `madd.s`/`msub.s` and the accumulator. The
rules it needs are already in the table above (`MADD_S` in `FPU.cpp`, `recMADDtemp` in `iFPU.cpp`: the
product goes through a register — so a chop to `f32` — and then through `FPU_ADD_SUB`, which is what
`madd` does today); what it is missing is a proof of the same kind, i.e. extending `int_model` to the
accumulator ops and checking `madda`/`msuba` separately from `madd`/`msub`.
