# 1 — mxvatv preload nan lip (FINAL)

Task R45. 2026-11-18.

## What I did

- Added `mxvatv_preload_lip_takes_nan_speeds_as_the_original` to `crates/game-sim/tests/mxvatv_step.rs`:
  200 lip states (on the ground, not firing, no brake) × 48 speed pairs, `vel.y` and/or `contact_vel.z`
  set to ±qNaN, ±sNaN or ±inf. Same patterns and shape as R42's Untamed test.
- It went red at state 1: a -NaN speed, the original stayed off the lip (`+0xc50` ready = 0), the port
  set ready = 1. Host `NaN <= 5.0` is false, so any NaN read as on the lip.
- Fix in `handling/step.rs` `preload`: both lip speeds compared through `F`, as R39/R42 did for Untamed.
  The test's own path classifier (`preload_path`) reads them through `F` too.
- Split the 10,000-state test's body into `preload_same_as_the_original` so both tests share it;
  the post hook is `hook_preload_post`. Nothing else in the file moved.
- Nothing from `../opentennis`: this is MX vs. ATV's preload only; the fix copies R42's.

## What the original does

SLUS-21104's preload (`./kit fn` on PRELOAD in `mxvatv_step.rs`) branches on `c.ole` for `vel.y <= 5` and
`contact_vel.z <= 30`. The FPU reads an exponent-255 pattern as ±max, so +NaN/+inf are over (on the lip)
and -NaN/-inf are under (off it).

## How it was proved

Function oracle: 9,600 NaN/inf lip states in the new test, plus the existing 10,000 random states in
`mxvatv_preload_is_the_originals`, both bit-exact against the ELF with the event post hooked. Red before the fix, green after.

## Not verified / not 1:1

none

## Next

Done.
