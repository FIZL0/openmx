# 1 — o2 float oracle (FINAL)

Task O2. 2026-10-15.

## What I did

`crates/ps2-float/tests/oracle.rs`: ten properties that run the ported arithmetic of F1-F3 against the
game's own code over 390,000 generated cases in all, from the input classes the brief asks for — zero and
minus zero, both ends of the denormal range, the smallest normal, the largest magnitudes, the Inf/NaN
patterns the EE cannot hold, exact powers of two, uniformly random 32-bit patterns, and *pairs* built with
a chosen exponent difference (0, 1, 2, 23, 24, 25, 26, 31, 60) so the adder's one guard bit is exercised
on purpose rather than by luck. `ps2-oracle` is a dev-dependency of `ps2-float` for this (a deliberate
dependency cycle, which Cargo allows for dev-dependencies only).

Finding the carriers was most of the work. The O1 set covers `max/min/abs`, the `mula/madda/madd` chain,
`sqrt.s` through the length, and VU0's broadcast multiply and `vrsqrt`; `div.s`, `rsqrt.s`, `msub.s`,
`cvt.w.s`/`cvt.s.w` and VU0's `vadd`/`vsub`/`vmul`/`vdiv` had nothing. So: disassemble the whole text
segment (`./kit dis 0x00100000 --len 0x35f800 --limit 1000000`), bucket every instruction into the
function it belongs to using `context/decomp/boot.funcs.tsv`, and list the *call-free* functions
containing each missing op, smallest first. Eleven of them turned out to be usable and are now named in
`research/oracle.txt` under an O2 section with their fields and argument registers. Two were rejected and
are written down there too, with why.

Ops per property, all bit-exact: `add.s`/`sub.s` 60,000 pairs · `mul.s` then `add.s` 10,000 ·
`mula.s`/`madda.s`/`madd.s`/`sqrt.s` 30,000 chains · `mula.s`/`msub.s`/`madd.s` 20,000 rows · `div.s`
20,000 · `rsqrt.s` 10,000 · `max.s`/`min.s`/`abs.s` 30,000 · `cvt.w.s`/`cvt.s.w` 40,000 ·
`vsub`/`vmul`/`vadd` 120,000 lanes · `vdiv`/`vmulq`/`vrsqrt` 60,000 lanes.

One real disagreement, and it is the reason for the whole exercise: **`abs.s` does not flush a denormal.**
It is a bit operation — clear the sign, no arithmetic unit, no rounding — where `max.s` and `min.s` do
flush their operands on the way in. `ps2_float::fpu::abs` has it right; my first model of it (flush, then
take the magnitude) did not, and the generator found it on the first denormal, at `abs(-6.625824e-39)`.
No ordinary input would ever have shown this.

## What the original does

The carriers, in the words of `research/oracle.txt`'s O2 section (`./kit fn` / `./kit dis` on the
addresses there):

- A mul-then-add accumulator into an object field, `*(obj+0x6c) += b*a` — two separate instructions, not
  a `madd.s`, which is what makes it the full-domain `mul.s` test.
- `32767.0 / x` into a field, and a second divider whose numerator is `(b - obj+0x5c)` and whose divisor
  is `0.5 * max(a, obj+0x64)` — between them, `div.s` with either side free.
- `1.0 / sqrt(x)` as a single `rsqrt.s` against a `lui`-built one: one argument, so negatives and
  denormals are legal cases.
- A quaternion to two rows of its rotation matrix: eleven `mul.s`, two `add.s`, three `sub.s`, and the
  two accumulator pairs `mula.s`/`msub.s` and `mula.s`/`madd.s`. The test transcribes that instruction
  order, so it proves the operand order of `msub.s` (`acc - fs*ft`) as much as its arithmetic.
- Two conversion helpers that write `(short)(int)f` and read `(float)short` back.
- VU0: `vsub.xyz` through the stack, `vmul.xyz` lane by lane, `vadd.xyz` into a matrix's fourth row, and
  a vector divided by a scalar as `vdiv` of `vf0`'s `w` lane (the constant one) by the argument, `vwaitq`,
  then the broadcast `vmulq.xyz`.

Two incidental findings, both recorded in `research/oracle.txt`: `./kit dis` prints `vdiv`'s two field
selectors the wrong way round (it shows `vdiv Q, $vf0x, $vf2w` where the encoding says `fsf = w`,
`ftf = x`; `ps2-oracle`'s interpreter reads the encoding correctly, so only the printout is wrong), and
`lqc2`/`sqc2` come out of llvm-mc as `ldc2`/`sdc2` — they are quadword, not doubleword.

## How it was proved

The function oracle, `crates/ps2-float/tests/oracle.rs`, 10,000 generated inputs per property and
390,000 bit-exact comparisons in total, in 0.06 s (`./kit check -- -p ps2-float --test oracle`). Every
comparison is against the instructions on the disc, with no emulator.

That the comparisons are live and not vacuous was checked by breaking one on purpose: changing the
modelled numerator from `32767.0` to `32766.0` fails with `the game stored 32767 (0x46fffe00), the model
32766`. Each property also prints its case count, which only happens when the loop ran — a missing disc
prints `SKIPPED: …` instead and the counts do not appear.

## Not verified / not 1:1

- **The rounding model itself is not proved by any of this, and cannot be.** `ps2-oracle`'s interpreter
  executes `add.s` by calling `ps2_float::fpu::add`, so both sides of every assertion in this file come
  from the same model: if the chop went the wrong way, the test would still pass. What these properties
  do prove is everything layered on top — which operation the game used, in which order, on which
  operands, with which constants, and what its own branches do at the edges. The model's own evidence is
  PCSX2's EE recompiler, which `crates/ps2-float/src/fpu.rs` documents itself as reproducing, and nothing
  here is independent of it. An independent proof needs the ops run on hardware or in the emulator and
  the results read back — a separate task, and worth one.
- **F4 (the game's own transcendentals) has nothing to compare against yet.** F4 is still open, and
  `crates/ps2-float/src/libm.rs` is the kit's stock implementation, not the disc's: no `sinf`, `cosf`,
  `atan2f` or `powf` in the image has been identified, so there is no carrier to call. `libm::sqrtf` is
  the one piece exercised, indirectly, through `sqrt.s` and `vrsqrt` (30,000 chains and 60,000 lanes) —
  and only up to the circularity above. When F4 names its functions, the generator and the carrier
  pattern in this file are what its coverage should use.
- **`sqrt.s` is only ever reached with a sum of squares.** Nothing call-free in the image takes a float
  and roots it, so the argument is always `dot(v, v)` as the real chain produced it: non-negative, which
  means `sqrt.s` of a negative — which the instruction defines as the root of the magnitude — is covered
  by `tests/fpu.rs`'s model alone. `rsqrt.s` does get both signs, through its one-argument carrier.
- **VU0's `vsqrt` has no carrier.** The only call-free use of it in the image (`0x003f2cd8`) wraps it in a
  dozen other lane ops; `vu::sqrt` is therefore reached only inside `vu::rsqrt`, where it is proved.
- **`cvt.w.s` is proved modulo 65536, and `cvt.s.w` over `i16` only.** Both conversion carriers store
  their result as a 16-bit field, so the `0x7fffffff` an out-of-range float saturates to is not visible
  here (it is in `tests/fpu.rs`), and `from_i32` is untested outside `-32768..32767`.
- **`msuba.s`, `suba.s` and `adda.s` are not covered.** The image's only call-free `msuba.s` sits inside a
  68-instruction VU0 routine, and every `adda.s`/`suba.s` carrier I found has its accumulator consumed by
  a `madd.s` that the quaternion carrier already proves. Reaching them needs either a bigger carrier with
  a RAM image behind it or the span technique I1 used.
- **F5 (matrices, quaternions) is out of scope here and still open.** `quat_two_matrix_rows` is proved as
  an *arithmetic* carrier only: nothing in the port claims to be that function yet, and when F5 ports it
  the comparison should be against the port's own helper rather than against a transcription in a test.
- The `mat4_row3_add` carrier's first three rows are only checked to be copied, not computed — that is
  what the function does, and it is the test's way of noticing if the quadword path changed.

## Next

Nothing outstanding for O2. The two tasks that should use this file as their starting point are F4 (name
the disc's `libm` and give it the same treatment; the carrier hunt recipe is in "What I did") and a new
task for the one proof the oracle cannot give: run the primitive ops on the real machine — a short EE
program or a PCSX2 hook fed the same edge classes — and compare the results against `ps2_float`, which is
the only thing that would make the rounding model itself evidence rather than assumption.

One thing outside O2 that the workspace gate tripped over, recorded here because the next session will
meet it too: this worktree's git-ignored `context/capture/` held three older recordings that the current
readers reject — the two S8 ones have no `ctl0.value` column (I2 added it) and `i1-input.csv` has no
`frame_counter` column (S8's `inputs_from_capture` needs it; the I1 recording predates that reader). The
tests are right to refuse them, so I moved them to `context/capture/pre-i2/` and `context/capture/pre-s8/`
rather than change another task's code or delete local data; `./kit check` is green with them aside. They
need re-recording, which needs `watch/input.toml`'s symbols back in `kit.toml` first.
