# 1 — split screen parked plane (FINAL)

Task S494b. 2026-11-21.

## What I did

- `InPlay::hand_to` (vehicle.rs) now hands the parked planes to every seat's Sim, not only on a
  one-seat ride. The helicopter stays one-seat only (S494a's rule), so the flag that gated both is now
  `helis_handed`.
- `World::simulate` (main.rs) shares the planes as F72 shares the cars: `World::planes_out` takes
  seat 0's copy and clears the others' (only with two seats or more and no AI riders, as
  `parked_out`); the planes step every frame after the cars (as `Sim::step` orders them, S494); their
  hulls follow the cars' in the riders' world, named `seats + cars + k`; every seat's bump carries
  their bodies after the cars'; each seat's pushes are written back to the one copy; after the pass
  every seat's Sim gets that copy back.
- The cars' write-back now only takes bump entries below `seats + cars`, so a plane's entry never
  lands on a car.
- Nothing taken from ../opentennis: it has no parked vehicles.

## What the original does

One plane in its dynamics world, which every rider's bike meets (S494's captures,
research/s494/index.md, for a lone rider). Split screen has one level node, so one plane.

## How it was proved

- New test `a_split_seats_bike_pushes_the_shared_parked_plane_as_a_lone_seats` (main.rs): the disc's
  biplane settled 25 ft up the bike's way; alone and as seat 1 (seat 0 100 ft off), the bike and the
  plane match tick for tick (within 1e-4) for 45 ticks, both seats hold the same plane each tick, the
  bike stops short of the plane's body and the plane is pushed > 0.1 ft along the bike's way.
  Seat 1's bike stops at z 9.02; the plane is pushed 0.43 ft. With `planes_out` forced to `None`
  the test fails at tick 11 (seat 1's bike rides on to z 11.08 against the lone seat's 10.08).
- F72's and F75's truck tests still pass.

## Not verified / not 1:1

- Against the original in split screen: no split-screen capture of a bike striking free10's plane.
  The proof is only "seat 1 matches a lone seat", and the lone seat is S494's port.
- Split screen with AI riders: the plane is not shared. Only seat 0's Sim holds a copy (its own
  step meets it with its players' bikes); the other seats' copies are dropped each pass so no two
  copies drift apart, so the other seats' bikes do not meet the plane there. No test covers this case.
- Two seats' bikes striking the plane in the same pass: not tested; the pushes land in pass order
  (last seat first), as for the cars.
- A downed rider's wreck bike does not meet the plane, in split screen or alone (S494 never added
  that for the lone Sim either).
- The helicopter in split screen is unchanged (still the AI riders' hull only); filed as S494c.

## Next

- S494c: the helicopter in split screen, the same way.
