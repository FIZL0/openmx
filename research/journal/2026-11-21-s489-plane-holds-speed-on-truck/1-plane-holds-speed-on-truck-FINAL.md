# 1 — plane holds speed on truck (FINAL)

Task S489. 2026-11-21.

## What I did

- Put the original's truck beside the Sim's per substep, rows 1569-1600: a test hook after the
  truck's tire step in the replayed node update, and the same print in the Sim (both removed).
- Found two things that slowed the push:
  1. The truck's launch boost was not taken off the image (Sim 1, original 1.886). `r48_seat_truck`
     now takes the launch timer, start and boost.
  2. The truck's own rays striking the plane's hull call its enable through its hit listener, so its
     brakes stay off. The Sim had only the plane's rays doing that. Now, in the plane's frame
     (`Flight`, crates/game-sim/src/flight.rs), a car whose own search struck the hull calls
     `InPlane::struck` too.
- `R48_PUSH_THROUGH` raised from 1587 to 1604 (tests/c16_fly.rs); the test's doc says where it parts.
- research/r48/index.md "What slowed the plane on the truck (S489)"; research/oracle.txt under S200's
  hit listener.

## What the original does

- The hit listener (`./kit dis 0x15acf0`): with the wrapper's +0x58 set and the struck collidable's
  category matching the mask (0x40), it calls the object's vt+0x124. On a car that is the enable
  (`./kit dis 0x398c50`), which sets driven; the controls step then writes the brakes 0.
- A hook on the enable over R48's replay: callers ra 0x15ad6c (the plane's rays) and ra 0x15ad28
  (the hit listener, from row 1580; alone on 1588 and 1589).
- The launch boost (engine +0x6c) scales the drive torque while the tire spins forward.

## How it was proved

- Against the original's node update replayed on states/r48/r48-1487.ram (the oracle the R48 tests
  already use), per substep: the truck's velocity, rates, brakes, engine and four tires.
  - Before: 1.4 ft/s slow by row 1573; brakes 1 on rows 1580, 1588, 1589.
  - After: within 0.008 ft/s on rows 1569-1587 (each tick's first substep), brakes 0 on both.
- The car's search hands back only its own entries on the plane's hull (`InPlane::search`), so the
  listener's category check is always met there; its controls step (`preforce`) runs after, in the
  substeps.
- `cargo test --release -p game-sim --test c16_fly r48`: the push holds to the capture through row
  1604; with 1605 it fails (truck 22.74 ft/s vs 24.48).

## Not verified / not 1:1

- From row 1587 the Sim parts from the original. The plane's wing ray (tip at x 7.5) meets the
  truck's hull 0.0015 ft short of its tip in the original and misses in the Sim, whose truck is
  0.0015 ft ahead. Reason: a 0.002-0.008 ft/s difference left since row 1572; not chased (S508).
- What leaves that difference at row 1572 is not known. Reason: not looked at; the truck's speed is
  right to 0.0006 ft/s on 1571.
- The same own-ray enable is not in the drivers' world (S499, car.rs `AmongCars`) or the riders'
  world. Reason: out of this task. Riders' world: S491 found the original's truck stays braked after
  a bike's strike, so it may not apply there. Filed as S507 for the drivers' world.
- Other parked cars seated from an image (not R48) don't take the launch words. Reason: only R48's
  test seats a car from an image.

## Next

- None for S489.
