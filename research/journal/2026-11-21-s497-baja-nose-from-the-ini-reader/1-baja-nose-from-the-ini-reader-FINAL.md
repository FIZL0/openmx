# 1 — baja nose from the ini reader (FINAL)

Task S497. 2026-11-21.

## What I did

- `car_spawn.rs`: `BAJA_SPOT` split into `BAJA_AT` (the position, still `f32` literals) and
  `baja_facing()`, which reads free07's `[Machine1]` `Direction` line through `phy::Ini::vector`, as
  the game reads `machine.txt` (truncating, not rounding to nearest).
- `free07s_parked_baja_rolls_on_as_the_original` now asserts the nose's three words bit for bit
  against the original's car `+0x280` (was within 1e-6). Dropped the "one bit under the port's" comment.
- Turned quat (body `+0x170` vs. `Machine::new`): printed the bits. Port `3d4d3377 3f3220f5 3cd26c80
  3f374c7b`, original `3d4d3378 3f3220f5 3cd26c82 3f374c7b`: x one bit, z two bits off, y and w exact.
  The check went from `abs < 1e-7` to at most 2 ulps per lane.
- The port's settle at the end of the test now starts from the file's exact nose too; still passes
  (0.0046 ft from the original's at play).

## What the original does

Reads `Direction` with `strtod` then its own double-to-float routine, which truncates (R65).

## How it was proved

R52's free07 image (`r52-free07-baja.ee.bin`): the nose at car `+0x280` after the original's own
put-back and enable equals `Ini::vector("Direction")` bit for bit. The test passes.

## Not verified / not 1:1

- The turned quat is not bit for bit: `Machine::new` is x one bit and z two bits off the original's
  put-back from the same nose and up. Fixing it is a change to `car.rs`, outside this task's file;
  filed S498.
- The `Position` stays as `f32` literals: the test only uses it for `parked_spawn`, whose y is checked
  within 1e-4 against the ground anyway; not asked, not checked bit for bit.

## Next

S498: port the put-back's quat-from-axes bit for bit.
