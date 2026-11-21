# 1 — baja turn bit for bit (FINAL)

Task S498. 2026-11-21.

## What I did

- Found why free07's parked baja's turned quat was off by bits (x one, z two): not the port's
  quat-from-axes, but the test's second put-back.
- `tests/car_spawn.rs` `free07s_parked_baja_rolls_on_as_the_original`: before its put-back the test now
  writes the car's own nose and up copies (car +0x280/+0x290) back into the wrapper's store (W+0xa0/+0xb0),
  so the put-back replays the load's. It first asserts the image's store is `spawn_axes` of those copies
  bit for bit (the load squared them once), which is what justifies the swap. The quat is now asserted bit for bit (`assert_eq!` of all four words),
  replacing the 2-ulp tolerance.
- `car.rs` `Machine::new`: doc says its turn is the load's put-back's and how a later put-back differs.
- No change to the port's code: `flight::spawn_turn` was already the original's.

## What the original does

- The put-back (`./kit dis 0x0015a030 0x0015a180`) calls the wrapper's vt+0xe4 (`./kit dis 0x0015a1b0
  0x0015a3c4`) with **pointers into its store** (W+0x90 spot, W+0xa0 nose, W+0xb0 up).
- vt+0xe4 squares the pair **in place**: side = nose × up, up = side × nose, then nose and up each scaled by
  VU0's rsqrt of (x²+y²)+z². Then `axes_to_quat` (0x0013f9f0) of them, handed to the body (body vt+0x48).
  The port's `flight::spawn_axes` + `axes_to_quat` are these, instruction for instruction.
- So each put-back squares the stored pair again. Squaring is not idempotent in the last bits: on free07's
  baja the nose keeps its bits but the up goes 3d06e5fb 3f7e608d 3ddc225a (once) → 3d06e5fc 3f7e608f
  3ddc225d (twice).
- The B47 image's store holds the once-squared pair, so the load did one put-back. The test's extra one was
  a second: its quat (3d4d3378 3cd26c82) is spawn_turn of the once-squared pair, bit for bit; the load's
  (from the car's unsquared copies) is the port's 3d4d3377 3cd26c80.

## How it was proved

- Save-state RAM image (B47's first baja image, in the shared context's states) run in the interpreter:
  the probe compared the original's body +0x170 after a put-back with the port's `spawn_turn` from (a) the
  store and (b) the car's copies, and the store before/after with `spawn_axes` once and twice. All bit for bit
  as above.
- `./kit check -- -p game-sim --test car_spawn free07s_parked_baja_rolls_on_as_the_original` passes with the
  quat asserted bit for bit; the roll stays within 0.046 ft over 1810 frames, both image places passed.

## Not verified / not 1:1

- Whether the original puts a parked car back again in play (an event restart, a reset; its ResetTime is -1).
  If it does, its up drifts by bits each time; the port builds each parked car once (`Parked::settled`) and
  has no repeated put-back. Not traced: no capture of a restart on free07 here.
- Only free07's baja proved bit for bit; the other parked cars' turns rest on the same code path (S484/B47's
  tests), not re-checked here.

## Next

- none
