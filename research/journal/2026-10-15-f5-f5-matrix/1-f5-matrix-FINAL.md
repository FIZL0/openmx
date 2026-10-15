# 1 — f5 matrix (FINAL)

Task F5. 2026-10-15.

## What I did

Added `crates/game-sim/src/math.rs` — eight helpers, one module, no changes to anything that already
existed except the `pub mod math;` line in `lib.rs` and a new section in `research/oracle.txt`:

| port | what it is |
| --- | --- |
| `Mat4`, `Mat4::IDENTITY`, `row`, `to_bits` | four `Vf` rows, row-vector convention, translation in row 3 |
| `mul` | the 4×4 product |
| `transform_point`, `transform_points` | a point, and a run of them, through a matrix |
| `basis` | the look-at frame, with the two orthogonalisation flags |
| `quat_to_mat4` | a quaternion and a position into a matrix |
| `quat_axes` | the forward and up axes of a quaternion on their own, on the scalar FPU |
| `mat4_to_quat` | the rotation back out of a matrix |
| `quat_deriv` | q̇ from an angular velocity |
| `slerp` | the spherical interpolation, including its arc cosine table lookup |

Finding the set was the first half of the job. These live in a region of a few dozen functions and
most of them are inlined, so rather than reading the dump I indexed it: one `awk` pass over
`context/decomp/boot.c` counting `_vmadd` / `_vmula` / `_lqc2` / `_sqc2` per function, which sorts the
region into "this is a maths primitive" and "this is a caller of one" in a page of output. The eight
above are the primitives with no call in them.

## What the original does

Recipes, not names — `./kit dis <addr>` for each.

**The product (`0x001131d0`, carrier).** There is no product function: the compiler inlined it at
every site. The smallest call-free function containing one multiplies the matrix at `obj+0x80` by the
one at `obj+0x40` into `obj+0xc0`, as four `vmulax`/`vmadday`/`vmaddaz`/`vmaddw` chains — so each
output row is `((B₀·a.x + B₁·a.y) + B₂·a.z) + B₃·a.w`, which is exactly what `ps2-float`'s
`vu::transform` already is. The port is four of those.

**The point loop (`0x0013e3f0`).** Loads the four rows once, then walks the arrays. Two details that
are easy to miss and both matter: the input's `w` lane is *replaced*, by `vaddx.w vf5, vf0, vf0x`, so
a point's own fourth lane never reaches the arithmetic; and the store is a full `sdc2`, so the
transformed `w` is written out, it is not a three-lane write. `in_stride == 0` means "the same as
`out_stride`", and `n <= 0` returns before loading anything.

**The look-at (`0x0013e330`).** Row 1 is the normalised up, row 2 the normalised forward, row 0 their
cross, row 3 the position with `w = 1` while the other three rows get `w = 0`. Both normalisations are
`vrsqrt`, the approximate one, which is why this cannot be written with a library `normalize`. The two
flags choose whether to orthogonalise first (`right = up × forward`, then either `up = forward × right`
or `forward = right × up`). Every cross product is the `vopmula`/`vopmsub` pair, with the operands
swapped between the two halves.

**Quaternion to matrix (`0x0013ebd0`) and the two axes (`0x0013f380`)** are the same mathematics on
different units — the first is VU0 with broadcasts off a doubled quaternion, the second is the scalar
FPU with `mula`/`madd`/`msub` — and they are *not* the same bits, which is why both are ported and both
are proved. `0x0013f380` writes three floats per axis, not a quadword.

**Matrix to quaternion (`0x0013f7b0`).** The largest of the four `0.25·(1 ± m₀₀ ± m₁₁ ± m₂₂)`
candidates, picked by a chain of `c.ole.s` compares that branch on false — so a tie loses, and the
order of the `±` inside each candidate is not the order a textbook writes it (`0.25·(m₂₂ + ((1 + m₀₀)
+ m₁₁))`, and so on). Then `sqrt.s`, which takes the magnitude, so a negative candidate does not trap.

**The derivative (`0x0013ec40`).** `½(q ⊗ ω)` for a pure `ω`, as `vopmula` / `vmaddaw` / `vopmsub` for
the vector part. The scalar part is *not* a dot product halved afterwards: the half comes out of a
constant quadword `(0.5, 0.5, 0.5, −0.5)` and the `w` lane is accumulated as three separately-halved
products. Writing it the obvious way loses the last bit.

**The interpolation (`0x0013ee10`).** Four-lane dot, `((x + y) + z) + w`. When `|dot| ≥ 1` it branches
*over its own store*, so the destination keeps `a`, the quaternion it loaded first. θ never calls
`acos`: it is two entries of the 30001-entry startup table, interpolated as `lo − w·lo + w·hi` through
the FPU accumulator. The three sines — θ, ±t·θ, (1−t)·θ — are a seventh-order Taylor polynomial run on
VU0 three lanes at once, and `1/sin θ` is a `vdiv`.

## How it was proved

Function oracle, `crates/game-sim/tests/math.rs`: nine tests, 10,000 generated inputs each (the point
loop's second test runs 1,428 calls of one to seven points), every one comparing raw 32-bit patterns —
never values, so `0` and `−0` are a failure. The generator spends about a third of its lanes on the
awkward classes: both zeros, `±MAX`, the smallest normal, a denormal, and the infinity and NaN patterns
the EE has not got. `./kit check`: 307 tests, up from 298.

Three things the harness had to get right, each of which started out wrong:

1. **A load is not an operand.** The first draft handed the port `Vf::from_bits`, which flushes
   denormals and clamps the exponents the EE lacks. That made the position row of every matrix and the
   `|dot| ≥ 1` return of the interpolation disagree, because those lanes are *copied*, and a copy keeps
   its pattern. The port now gets the raw quadword; the lanes that reach an arithmetic unit are
   filtered there, on both sides, by the same code.
2. **`−0.1666667` is not the coefficient.** Spelled as a decimal it rounds to `0xbe2aaaad`; the
   quadword at `0x00426e10` holds `0xbe2aaaab`. Two of the last bits, and they showed up as one or two
   ulp on all four lanes of every interpolation. The coefficients are now the words themselves.
3. **The carrier pins two lanes.** `obj+0x68` and `obj+0x78` are lanes `[2][2]` and `[3][2]` of the
   right-hand matrix, and the carrier writes its own `f12`/`f13` arithmetic into them before loading
   it. The test reads both matrices back out of RAM after the call, and generates `f12` and `f13` too,
   so those two lanes vary across the run instead of being 1.0 and −0.0 ten thousand times.

The fourth branch-coverage check is in the test itself: the matrix-to-quaternion proof asserts that all
four candidates won at least once (they came out 2278 / 2407 / 2543 / 2772), and the interpolation proof
asserts the nearly-parallel guard was reached (3496 of 10,000).

## Not verified / not 1:1

- **`slerp` reads `crate::air::AcosTable`, which belongs in `ps2-float`.** The table is right — S10's
  `the_acos_table_is_the_games_own` proves all 30001 entries against the game's own builder — and this
  module reads the entries itself rather than calling `AcosTable::at`, because `at` weights the same two
  entries as `(1 − w)·lo + w·hi` through a `mula` and the interpolation does `lo − w·lo + w·hi` through
  an `adda`/`msuba`, which is a different last bit. But a maths module depending on the air module for a
  table is backwards. The S10 journal already suggests moving `AcosTable` down into `ps2-float`; I did
  not, because this run shares the tree with other agents and the instruction was to add rather than
  move. Whoever moves it should delete nothing here but the `use`.
- **`transform_points` models one stride.** The original takes independent `out_stride` and `in_stride`
  byte strides (with `0` meaning "same as out"); the port takes two slices, which is stride `0x10` on
  both sides. The proof covers `0x10`/`0x10` and the `in_stride = 0` default path, not a stride that
  walks a bigger struct. A Rust caller with a bigger struct writes the loop, so this is a deliberate
  API difference rather than a missing case — but it is a difference.
- **The flags are proved as 0 and 1 only.** `basis`'s two flags and `slerp`'s `shortest` are `bool` in
  the port and integers in the original. The original's `shortest` reaches the arithmetic through
  `neg`, so any nonzero value behaves alike, but I did not generate one; `basis`'s flags are plain
  branches, so the same holds with more confidence. Nothing calls them with anything else.
- **Nine functions proved, five neighbours not ported.** The larger composite helpers at `0x0013e040`,
  `0x0013e1e0`, `0x0013f110`, `0x0013f410` and `0x0013fa60`, and the two compressed-quaternion keyframe
  decoders at `0x0013ef20` and `0x0013f000`, sit in the same region and call into these. They are not
  in F5's scope and are not written down as missing anywhere else, so they are written down here.
- **`Mat4::IDENTITY` is not proved against anything.** It is the obvious constant, not a transcription
  of the game's identity initialiser, which I did not find or look for.
- **`./kit check` does not run clippy**, so `math.rs`'s `#![deny(clippy::float_arithmetic)]` — the lint
  that is the whole reason a bit-exact port stays bit-exact — is not enforced by the gate. I verified it
  by hand with `cargo clippy --no-deps -p game-sim --all-targets`, which is clean for `math.rs` (the
  `--no-deps` is needed because `ps2-float` itself does not pass clippy today, and `air.rs` and
  `trick.rs` have pre-existing findings). Nothing re-checks it on the next change.
- **`./kit dis` prints the wrong target for `bc1f`.** At `0x0013ee80` it says `-> 0x13ef18`; the encoded
  offset is 36 words, so the target is `0x0013ef14`, the store. This is the difference between "the
  guard skips the store" and "the guard skips nothing", i.e. between returning the destination's old
  contents and returning `a`, and I only caught it by decoding the word by hand. Same class of kit quirk
  as the `vdiv` field selectors already noted in `oracle.txt`.
- No emulator was used and none was needed: all nine are pure functions of their arguments, and the
  oracle runs the real instructions. Nothing here is checked against the game actually running — if a
  caller passes these something I did not generate, that is a caller's proof, not this one's.

## Next

Nothing is waiting on this task. The one thing worth doing out of it, for whoever gets there: move
`AcosTable` and its `acos` out of `crates/game-sim/src/air.rs` into `ps2-float`, so that `math.rs` and
`air.rs` both read a table from the float crate instead of the maths module importing the air module.
Both lookup orders need to survive the move — they are genuinely different last bits, not a duplicate.
