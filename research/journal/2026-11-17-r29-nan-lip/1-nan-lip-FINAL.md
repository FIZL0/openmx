# 1 — nan lip (FINAL)

Task R29. 2026-11-17.

## What I did

- `crates/game-sim/tests/mxvatv_air.rs`: pulled the backflip test's run-and-compare into
  `same_as_the_original` (no change to what it checks), and added
  `mxvatv_backflip_lip_takes_nan_speeds_as_the_original`.
- The new test went red at once: with the body's `vel.y` and the contact's `vel.z` both +NaN, the original
  armed the flip (`+0x11a0` = 1) and the port read "off the lip".
- Fix in `crates/game-sim/src/handling.rs` (`backflip`): the two lip speeds now compare through `F`
  (`F(chassis.vel.y()) > 5.0`, `F(air.contact_vel.z()) > 30.0`), not as host `f32`s.
- Nothing taken from `../opentennis`: it has no such compare.

## What the original does

- The lip test (SLUS-21104's backflip, `./kit fn 0x004e4f20` on the MX vs. ATV image) branches on `c.ole`
  against 5 and 30.
- The PS2 FPU has no NaN: an exponent-255 pattern is read as ±max. So a +NaN speed is over the threshold
  and a -NaN is under it.
- `F`'s compares already read operands that way (`ps2-float` `compared`). Through `F`, `>` and `!(<=)` are the
  same, so the plain `>` form is exact.
- Untamed's form (`!(v <= 5.0)` on host `f32`) gets a +NaN right but a -NaN wrong (host: over; FPU: under).
  Filed as R39; not touched here.

## How it was proved

- Function oracle, `mxvatv_backflip_lip_takes_nan_speeds_as_the_original` in
  `crates/game-sim/tests/mxvatv_air.rs`.
- 200 generated lip states × 48 speed pairs = 9,600 runs. Each speed is +NaN, -NaN, +sNaN, -sNaN, +inf, -inf
  or the state's own value (not both own).
- Every output word of the bike, body, frame and collision regions, plus the stunt, steep and wreck calls,
  must equal the original's.
- Red before the fix, green after. The 10,000-state `mxvatv_backflip_is_the_originals` still passes.

## Not verified / not 1:1

- Untamed's backflip lip with a -NaN speed: not checked; filed as R39 (its own file and test).
- NaN in the other compares of this backflip (stick, grace, torque, frame axes, roll and pitch): not run with
  NaN. They already compare through `F`, so they read NaN as the FPU does, but no NaN state proves it.

## Next

- none
