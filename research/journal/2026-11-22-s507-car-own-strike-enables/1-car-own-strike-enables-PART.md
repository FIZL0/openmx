# 1 — car own strike enables (PART)

Task S507. 2026-11-22.

## What I did

- In a driven car's world (S499's `Machine::cars_met`, crates/game-sim/src/car.rs), a parked car whose
  own search strikes the driven car's hull now calls `InPlane::struck` (body woken, driven word set),
  before its controls step. Its brakes come off on that frame and go on again after (`InPlane::preforce`
  clears the word). This is the same two lines `Flight::frame_met` has for the plane's world (S489).
- Before, only the driven car's rays on the parked car's hull set the word (the first of S489's two
  callers of the enable); the parked car's own rays on the driven car's hull did nothing.
- research/r48/index.md: a line under S489's fix saying the drivers' world now has it too, unproven.

## What the original does

- S489 (research/r48/index.md, 'What slowed the plane on the truck'): the car enable (`./kit fn 0x398c50`)
  has two callers in R48's plane-pushes-truck node. The second is the hit listener (`./kit fn 0x15acf0`,
  return 0x15ad28): the truck's own rays striking another collidable, with the wrapper's +0x58 set and
  the struck collidable's +0x84 & [0x468f68] (0x40); it calls vt+0x124, the enable.
- Nothing in that listener names the plane: the test is on the struck collidable's class word. R55
  gives a car hull's class as 0x52058, which has 0x40 set, so a parked car's rays on a driven car's hull
  pass the same test. That is a reading, not a capture.
- S491 (riders' world): the truck stays braked the frame after a bike strikes it. That is the struck
  path (the bike's rays on the truck), not the truck's own rays on a hull; bikes have no hull for the
  truck's search, so it does not speak against this change.

## How it was proved

- Not proved against the original: no drivers' capture exists. context/capture/s499/switch.csv is empty
  (S499's tape never put the buggy out) and S499 is blocked on a person recording the buggy driven into
  free09's truck (wizard S499).
- The port's S499 disc tests still pass with it (`./kit check -- -p openmx --bin openmx parked_truck`,
  7 tests): the buggy wakes and pushes the truck, alone and the same to the bit in split screen.

## Not verified / not 1:1

- The whole change: no capture of the original's car meeting a parked car exists, so whether the
  parked car's brakes really come off on the frames its own rays strike the driven car's hull is not
  checked. Reason: S499's ride capture needs a person (desktop locked, the pause switch by tape failed).
- That a driven car's hull passes the listener's class test (+0x84 & 0x40): taken from R55's class word
  for the truck's hull (0x52058); the buggy's and baja's hull class words were not read.
- That the wrapper's +0x58 is set on a parked car in the drivers' world as in the plane's: not read.
- How much the pushed truck's distance changed in the S499 disc tests: the tests only assert it moves;
  the numbers before and after were not compared.

## Next

- Once S499 lands its ride capture (context/capture/s499/ride.p2m2 and its rows), compare the truck's
  per-row velocity and brakes (car +0x4ac, the driven word) against the port with and without this
  change. Keep it only if the capture's truck drives brakes-off on the frames its own rays meet the
  buggy's hull; otherwise revert the two lines in `cars_met` and the r48 line.
