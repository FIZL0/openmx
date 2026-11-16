# 1 — car edge throw (FINAL)

Task S337. 2026-11-16.

## What I did

- Ported the car's edge throw as `Machine::throw_back` (crates/game-sim/src/car.rs). It sends the body
  back toward the map's middle, halves the vehicle's edge scale, spins it from three `rand()` units,
  and wrecks the car (reason 13, lifted cleared).
- Ported the unwreck as `Machine::unwreck`. The car's post now runs it when the wreck timer answers
  `CarEvent::Unwreck`, as it runs S335's stuck reset on `Reset`.
- Added `Machine::edge_scale` (the vehicle's +0x194, 20.0 from the constructor; nothing in the car
  reads it) and put it on the snapshot wire. `wreck_timer` is now `pub` so the oracle test can run it.
- Shell (crates/openmx/src/vehicle.rs): `throw_at_edge` runs right after the car's step in
  `CarRide::step_met` and `CarRide::step_traffic` (before the props' own step). It draws on
  `CarRide::rng`, the stand-in for the session's `rand()` that the engine noise already uses.
- Named `car_wreck` and `car_unwreck` in research/oracle.txt, with the throw's constants.
- Nothing in ../opentennis covers this (no vehicles); nothing taken.

## What the original does

- The wreck step (`./kit dis 0x00398820 0x00398bd0`): a whole car that `position_query` puts within
  150 ft of an edge is thrown:
  - target (W/2, y, D/2, 1); dir = normalised target − position, or (0,0,0,1) at zero length;
  - k = 2·√11.80127; speed = min(1612.8, W/2) / k; lift = 16.1·k + 190/k (plain mul and add, not
    fused);
  - velocity = dir·speed (VU, xyz), plus the lift on y;
  - +0x194 ×= 0.5;
  - rates = (0.5, 0.4, 0.7) × `rand()` units in x, y, z order, w 1;
  - then vt+0x1d0 with 13.
- vt+0x1d0 (`./kit dis 0x00398bd0 0x00398be0`): reason = 13, lifted = 0.
- vt+0x1d4 (`./kit dis 0x00398be0 0x00398c40`): reason 13 → +0x194 /= 0.5; reason = 0; the
  measure's latch (+0x2c0) = 1; body velocity xyz ×= 0.3.
- The throw is the plane's (S36) with the car's own numbers (the plane: 14.906832, 2073.6, 240).

## How it was proved

- Function oracle: `crates/game-sim/tests/car_wreck.rs`, `the_cars_wreck_step_matches_the_original`.
  It runs the original's wreck step in the interpreter on a car laid out by hand in recording2's RAM
  image, over 12,000 generated cases. Branch counts: none 6214, thrown 4072, unwreck 1527, reset 187.
  It compares every word either side writes, bit for bit: velocity, rates, edge scale, reason,
  lifted, latch, stuck clock, the generator's state, and whether the reset was asked for. All match.
- The shell: `crates/openmx/tests/car_edge_throw.rs`. Buggy, truck and baja start 84 ft inside a flat
  floor's edge, on the gas. Each is thrown on tick 0 (wrecked, heading back toward the middle) and
  lands whole on tick 216 (edge scale back to 20, speed cut).

## Not verified / not 1:1

- No live capture compares a whole throw. The C18 `truck-edge` capture and S149a's car RAM images
  are no longer in context/. C18 saw the original's truck wrecked for 211 frames on its shortened
  free07. The port's 216 is on a flat floor of a different size, so the two can't be compared.
- The oracle's car is laid out by hand, not taken from a real car's image. It holds only the words
  the wreck step reads. A word it reads that I missed would show as a mismatch, but none did.
- `rand()`: the port draws from `CarRide::rng`, the port's own seed (S247's choice). The original
  draws from the session's generator, wherever it stands. The arithmetic per draw is proved; the
  values a real session gets are not reproducible.
- Order: the throw runs in the shell right after the car's step returns, not inside the post. The
  car is the only body in its physics world, so nothing runs between, but this is reasoned, not run
  against the original. The unwreck runs inside the post, after the measure.
- Inherited from S335 and not changed: the wreck timer runs inside the measure, before `grounded` is
  updated. The original's wreck step runs from the post after the measure, so it may read this
  frame's +0x530 where the port reads last frame's. Not checked.
- The edge scale's start value (20.0) is read from the vehicle constructor's store at 0x003a22c0,
  not from a live car. No car code reads it, so it changes nothing that is simulated.
- Online seats: the throw draws from each seat's own `CarRide::rng`, the same generator the engine
  noise uses. Whether a rollback across a throw gives every peer the same spins was not checked.
- Not watched on screen: no pad here.

## Next

Nothing for this task.
