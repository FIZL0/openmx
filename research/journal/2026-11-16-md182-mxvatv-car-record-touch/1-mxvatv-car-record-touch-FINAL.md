# 1 — mxvatv car record touch (FINAL)

Task MD182. 2026-11-16.

## What I did

- `atv_car::Ride::record_on` (crates/game-sim/src/atv_car.rs) now sets both touching words from the
  record's pass: `Vehicle::touching` (the tipped wreck's) and `State::touching` (the pieces' air control).
  Before, only the vehicle's was set, so a car resting on a prop on an MX Unleashed world still ran its
  air control.
- New test `each_car_touching_a_prop_holds_its_air_control_off` in crates/game-sim/tests/mxvatv_car_mxu.rs:
  - each of the five cars, rigged, driven into MD149b's free09 prop: every tick both touching words equal
    the record's;
  - at the first tick the record touches (riding), the car is lifted 40 ft, levelled, given 30 ft/s along
    the run, its stale prop contacts cleared, and stepped two frames through `Ride::frame_with` (no record
    runs, so the touch holds) with the stick at 1 and at 0;
  - touching: body rates and moments equal (no air control). The same car not touching: they differ.
- Two frames, because the first frame's air control reads the grounded word the ground left.
- Mutant (the old one-word set) fails the test at the buggy's touch tick (`(false, true)`).
- Nothing taken from ../opentennis: it has no car.

## What the original does

The car's pieces and its vehicle each read the collision record's touching word (MD149b's record, the
word at the record's touching offset, tests/mxvatv_car_frame `mx::TOUCHING`). The task's own brief; I
did not reread the decompilation.

## How it was proved

- The game-sim test above, on free09 with the five cars' own files.
- No capture: the original has no car on an MX Unleashed world (MD149b).

## Not verified / not 1:1

- No proof against the original: there is no MX vs. ATV car on an MX Unleashed world to capture. The fix
  rests on the brief's reading that both words come off the same record word.
- The brief names a landing check that also reads the pieces' touch. In the port only the aero's air
  control reads `State::touching`; I found no landing check that reads it, so only the air control is tested.
- The truck, lifted and stepped with the stick over, ends 3.0 ft further along z than with the stick
  centred, all other body words equal. Not the air control (rates and moments match), so the test compares
  only rates and moments. Filed as R38.

## Next

None.
