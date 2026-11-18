# 1 — untamed spin preload nan lip (FINAL)

Task R42. 2026-11-17.

## What I did

- Split `untamed_spin_is_the_originals` into `spin_harness` and `spin_same_as_the_original`, and
  `untamed_preload_is_the_originals` around `preload_same_as_the_original`
  (crates/game-sim/tests/untamed_reworked.rs), as R39 did for the backflip. Same seeds, same paths,
  same checks.
- New tests `untamed_spin_lip_takes_nan_speeds_as_the_original` and
  `untamed_preload_lip_takes_nan_speeds_as_the_original`: 200 lip states each, `vel.y` and `contact_vel.z`
  as +qNaN, -qNaN, ±sNaN, ±inf or their own value (48 combos, 9,600 calls per test). The patterns and combos
  are shared helpers (`ODD_SPEEDS`, `odd_lip_speeds`, `with_odd_lip_speeds`); R39's backflip test left as is.
- Both went red before the fix:
  - preload, state 1 (`vel.y` +NaN, `contact_vel.z` -NaN): original off the lip, port set `preload_ready`
    (`bike + 0xc40` 0 vs 1).
  - spin, state 97: original off the lip, port armed (`bike + 0x11c8` 0 vs 1).
- Fix in crates/game-sim/src/handling/untamed.rs: both lips compare through `F` (`F(v) > 5.0`), as R39's
  backflip. All 15 tests in untamed_reworked green after.
- `spin_path` / `preload_path` (the tests' own branch labels) read the lip through `F` too, so the NaN
  states are labelled as the original branches (spin's drives the "binding asked" check).
- Nothing taken from ../opentennis: R39's fix in this repo is the model.

## How it was proved

Function oracle against the SLUS-21701 RAM image: 9,600 NaN/inf lip calls per function plus the existing
20,000 generated states each, bit-exact on the bike regions, event posts and stunt/steep/binding calls.

## Found on the way

- SLUS-21104's preload (`handling::step::preload`) has the host form `vel.y() <= 5.0 || …`: a -NaN reads as
  on the lip. Filed as R45 (outside R42's files).

## Not verified / not 1:1

none

## Next

none
