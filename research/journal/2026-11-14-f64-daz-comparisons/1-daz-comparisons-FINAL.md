# 1 — daz comparisons (FINAL)

Task F64. 2026-11-14.

## What I did

- `F`'s `PartialEq`/`PartialOrd` are no longer derived (`crates/ps2-float/src/fpu.rs`). Both sides go
  through `compared`, which reads each value as an FPU operand (denormal → ±0, exponent 255 → ±max),
  then compares. `lt`/`le`/`gt`/`ge` are written out so they stay one host compare each.
- The `F`↔`f32` cross comparisons and `Acc == f32` go through the same path. `Acc`'s own derive
  delegates to `F`, so it follows too.
- The `native` build compares plainly, like the rest of its arithmetic (no denormal flush there).
- Nothing in game-sim was changed. The hand-written `F::from_bits(..) < ..` reads in race.rs's
  snap-to-ground are now redundant but harmless; left alone to keep the merge clean.
- Nothing taken from opentennis: its float layer has the same derived compare.

## What the original does

- `c.eq.s` / `c.lt.s` / `c.le.s` under PCSX2's recompiler run with MXCSR denormals-are-zero, so a
  denormal compares as zero. The ps2-oracle interpreter (`crates/ps2-oracle/src/ee.rs`, COP1) already
  reads both sides with `fpu::operand` before comparing; the port's `F` now does the same.

## How it was proved

- `crates/ps2-float/tests/fpu.rs` `comparisons_read_denormals_as_zero`: every comparison operator, and
  `partial_cmp`, `F<f32`, `f32<=F`, `Acc==f32`, against an integer model (operand read, then
  sign-magnitude order with ±0 equal). Inputs: all edge-pattern pairs (~1,600) plus 200,000 generated
  pairs, weighted to the denormal band. The test also asserts >1,000 pairs where the host order differs,
  so it would fail on the old derived compare.
- The oracles were rerun by the gate (`./kit done`): ps2-float touches the camera and tick `[[slow]]`
  oracles' paths, so they run too, along with ps2-oracle and game-sim's suites.

## Not verified / not 1:1

- The exponent-255 operand (Inf/NaN pattern) read as ±max in a compare follows the ps2-oracle
  interpreter and the module's own operand rule. Not checked against PCSX2's recompiler, which may compare
  such a pattern unclamped unless a game's compare-clamp setting is on. The port's arithmetic never makes
  such a value, so only host-made `F(f32::INFINITY)`/NaN can reach this.
- Not checked live in PCSX2 (no capture). The DAZ compare itself is MXCSR behaviour, and S296's oracle
  run of the game's own `c.lt.s` is what showed the difference.

## Next

- none
