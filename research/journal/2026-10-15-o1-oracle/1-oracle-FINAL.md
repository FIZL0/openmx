# 1 — oracle (FINAL)

Task O1. 2026-10-15.

## What I did

Pointed the oracle at twelve of the game's own functions, picked for having answers nobody has to think
about, and wrote `crates/ps2-oracle/tests/basics.rs` around them: a handful of cases worked out by hand,
then 10,000 generated cases per property against a `ps2_float` model. 140,000 calls in all, and the
whole file runs in well under a second with no emulator.

Two instructions were missing from the interpreter and were added to `crates/ps2-oracle/src/ee.rs`:

* `pextlw` (MMI0, `sa = 0x12`) — interleave the two low halves word by word.
* `vmax`/`vmini` in their broadcast and full-register forms (COP2 `fun` `0x10..0x17`, `0x2b`, `0x2f`),
  behind a new `vu_pick` helper.

The twelve names and what each one computes went into `research/oracle.txt` under an `O1` heading.

## What the original does

All twelve are the inlined maths helpers a PS2 game's vector header leaves behind — one copy per
translation unit, called from everywhere, a handful of instructions each. `./kit fn` is no use on them;
`./kit dis <addr> --len 0x30` shows the whole thing.

* `float_max` / `float_min` / `float_abs` — one instruction each (`max.s`, `min.s`, `abs.s`), arguments
  in `$f12`/`$f13`. **There is no clamp function in the game**: the call sites compose these two, which
  is what `ps2_float`'s `F::clamp` claims to be.
* `vec3_dot`, `vec3_length`, `vec3_length_sq` — the FPU accumulator chain, `mula.s` then `madda.s` then
  `madd.s`, in index order, with `sqrt.s` on the end for the length. Not three multiplies and two adds:
  each product is chopped and then aligned against a wider running total, so the order is part of the
  answer.
* `vec3_add`, `vec3_sub` — six `lwc1`, three `add.s`/`sub.s`, three `swc1`. All six loads happen before
  any store, so the output may alias either input.
* `vec3_scale` — the quadword path: `lqc2`, `mfc1`+`qmtc2` to get the scalar into a VU register,
  `vmulx.xyz` off its `x` lane, `sqc2`. The masked `w` lane comes back from the input untouched.
* `vec3_normalize` — `vmul.xyz` for the squares, `vmaxw.xyzw vf3, vf0, vf0w` to conjure a vector of ones
  out of `vf0` (this is what wanted the new instruction), `vadday.x`/`vmaddz.x` to sum them, then
  `vrsqrt` and `vmulq.xyz`. `vrsqrt` is the VU's approximation, not a correctly rounded reciprocal
  square root.
* `vec4_abs` — builds `0x7fffffff` in all four lanes with `lui`/`ori`, `pextlw` against itself and
  `pcpyld`, then one `pand`. This is where the interpreter first ran out of instructions.
* `keyframe_index_stride2` — the only one with a loop: a binary search over 8-byte entries keyed on the
  float at `+0`, returning how many keys are at or before the query time. Three special cases outside
  the loop (empty table, below the first key, at or past the last) and two different return paths inside
  it, which is exactly the shape that gets ported wrong. The same search over `0x28`-byte entries lives
  at the address noted beside it in `research/oracle.txt`.

## How it was proved

The function oracle, `crates/ps2-oracle/tests/basics.rs`, eight tests:

| test | cases | compared against |
| --- | --- | --- |
| `the_obvious_answers_are_the_obvious_answers` | 12 fixed | arithmetic: `max(3,7)`, `abs(-2.5)`, the 3-4-5 triangle, `(1,2,3)·(4,5,6) = 32`, the zero vector |
| `min_max_and_abs_are_the_fpus_own` | 10,000 × 3 | `fpu::max` / `fpu::min` / `fpu::abs`, bit-exact |
| `a_clamp_is_min_of_max_and_matches_the_float_model` | 10,000 | `F::clamp`, bit-exact, half of them with the bounds the wrong way round |
| `the_accumulator_chain_matches_the_float_model` | 10,000 | `fpu::dot` and an `Acc` chain written out, bit-exact |
| `the_vector_writers_put_back_what_the_model_says` | 10,000 | `fpu::add`/`fpu::sub`/`vu::mul`, bit-exact, plus the untouched `w` lane |
| `the_quadword_absolute_value_clears_four_sign_bits` | 10,000 | `bits & 0x7fffffff`, exact, no model involved |
| `normalise_in_place_follows_the_vu_chain` | 10,000 + 1 fixed | the VU chain written out, bit-exact; `(3,4,0)` within 2.5e-4 of `(0.6,0.8,0)` |
| `the_keyframe_search_agrees_with_a_linear_scan` | 10,000 tables of 1–24 keys, plus the empty one | a linear scan, exact |

Generated inputs come from a fixed-seed xorshift and are deliberately spread over the magnitudes a game
uses (1e-8 to 1e6, both signs, and exact ±0), not 0..1 — the point of the oracle over a recording is the
inputs the game never produced.

The suite was checked for being able to fail: changing the expected `length(3,4,0)` to 5.5 produced
`the game gives 5 (0x40a00000), the model 5.5 (0x40b00000)`, i.e. the interpreter really did run the
game's own `sqrt.s` over a dot product it computed. `./kit check` is green (128 tests) and `./kit audit`
clean.

## Not verified / not 1:1

* **The bit-exact comparisons are partly circular.** The interpreter's arithmetic *is* `ps2_float`, so
  comparing it against a `ps2_float` model proves the decode, the operand plumbing and the order of
  operations, but not the arithmetic itself — that was F1's job (`ps2-float` proved against an integer
  model of the FPU). The non-circular part of this task is the fixed cases, `vec4_abs` (pure bit
  manipulation) and the keyframe search (compared against a linear scan).
* **Nothing here was compared against the real machine.** Every number comes from the interpreter; no
  PINE capture or save state was taken, because these functions are pure and the emulator cannot be
  asked to call them with chosen arguments anyway. If the interpreter and the hardware disagree about,
  say, `vrsqrt`, this task would not have noticed.
* **`pextlw` is proved only in the form the game uses it**, `pextlw rd, rs, rs` — the two source
  operands are the same register, so the test cannot tell whether `rs` and `rt` are the right way round.
  The implementation follows the R5900 manual (`rt`'s words land in the even positions).
* **`vmini` and the full-register `vmax` are implemented but unexercised.** Only `vmaxbc` appears in the
  twelve functions. They are two lines sharing the broadcast form's helper, added so the next person
  does not hit the same panic, but no test calls them.
* **`vu_pick`'s ±0 tie-break is unproved.** It orders the operands as sign-magnitude integers, which is
  what the hardware does and which differs from `f32`'s own ordering only when one side is `-0.0`. No
  function in this task reaches that case.
* **The keyframe search is only proved on strictly increasing tables.** Its behaviour on a table with
  duplicate or out-of-order keys is undefined by the linear-scan model, so those cases are not
  generated. The game builds these tables itself; nothing yet shows it never produces a duplicate.
* **The near-duplicates named in `research/oracle.txt` were read, not called**: the thunks beside
  `vec3_length`/`vec3_length_sq`, the second `vec3_scale`, and the `0x28`-stride keyframe search. They
  are byte-identical or structurally identical in the listing; no test calls them.

## Next

The oracle is a tool now rather than a suspect, so the next function to prove with it can be one that
matters. Two obvious starts, both already named in `research/oracle.txt`: the string and memory routines
(hand-written with MMI quadword ops, pure functions of their arguments and RAM — easy and high-value),
and anything in the vehicle update once R6's field map is trusted. Expect more unimplemented
instructions the moment a function leaves this maths-helper neighbourhood; the panic names the
instruction and `./kit dis` shows it in context.
