# 1 — untamed backflip nan lip (PART)

Task R39. 2026-11-17.

## What I did

- Split `untamed_backflip_is_the_originals`'s body into `flip_harness` and `flip_same_as_the_original`
  (crates/game-sim/tests/untamed_reworked.rs), as R29 did for SLUS-21104's test. The 20,000-state test is
  unchanged otherwise (same seed, same paths).
- New test `untamed_backflip_lip_takes_nan_speeds_as_the_original`: 200 lip states (on the ground, not
  active, no spin, no brake), each with `vel.y` and `contact_vel.z` as +qNaN, -qNaN, ±sNaN, ±inf or their own
  value (48 combos each, 9,600 calls).
- It went red before the fix: state 1 (`vel.y` +NaN, `contact_vel.z` -NaN) — the original stays off the lip,
  the port armed the flip (`bike + 0x118c` 0 vs 1).
- Fix in crates/game-sim/src/handling/untamed.rs: the backflip's lip compares through `F`
  (`F(v) > 5.0`), as R29's SLUS-21104 fix. Green after.
- `flip_path` (the test's own branch labels) now reads the lip through `F` too. It drives the "binding asked"
  check, so a host compare there would have expected the binding call on -NaN states.
- Nothing taken from ../opentennis: R29's fix in this repo is the model.

## What the original does

The lip's two speed tests are `c.ole` branches. The PS2 FPU has no NaN: an exponent-255 pattern reads as
±max by its sign. So a +NaN speed is over 5 / 30, a -NaN under. Host `!(x <= 5.0)` is true for any NaN.

## How it was proved

Function oracle against the SLUS-21701 RAM image (nat_or.bin): the 9,600 NaN/inf lip calls above plus the
existing 20,000 generated states, all bit-exact on the bike regions and the stunt/steep/binding calls.

## Not verified / not 1:1

- The 360 (`untamed::spin`) and the preload (`untamed::preload`) have the same host-f32 lip compare. Not
  checked here (outside R39); filed as R42.

## Next

none
