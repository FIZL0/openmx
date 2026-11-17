# 1 — f compare denormals (FINAL)

Task MD161d. 2026-11-17.

## What I did

- Found the work already done. MD161d was filed 11-14 at 20:49; F64 (c065dba93, 11-14 at 23:41)
  made `F`'s `PartialEq`/`PartialOrd` read both sides as FPU operands: denormals as ±0, exp 255 as ±max.
- `Acc` derives its compares from `F`, so it flushes too.
- The `F` doc in `crates/ps2-float/src/fpu.rs` ("denormals read as zero") is now true. No change there.
- Grepped ported compares for denormal workarounds. One stale comment: `hold_yaw_rate_untamed` in
  `crates/game-sim/src/atv_car.rs` said "`F`'s order is plain IEEE". Fixed the comment only. Its explicit
  flush through `F::from_bits` is now redundant but harmless, so I left the code alone.

## What the original does

`c.eq.s` / `c.lt.s` / `c.le.s` read a denormal as zero (F64's journal has the proof).

## How it was proved

- F64's test `comparisons_read_denormals_as_zero` (crates/ps2-float/tests/fpu.rs) and its ~200k-pair
  integer model.
- MD161's `untamed_yaw_hold_is_the_originals` oracle (crates/game-sim/tests/untamed_truck.rs), 20,000
  cases with denormal rates and limits, runs in the gate.

## Not verified / not 1:1

none

## Next

Nothing.
