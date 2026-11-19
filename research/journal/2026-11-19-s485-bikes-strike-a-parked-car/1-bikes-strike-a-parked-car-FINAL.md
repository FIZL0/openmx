# 1 — bikes strike a parked car (FINAL)

Task S485. 2026-11-19.

## What I did

- `car::Parked`: a parked car the bikes meet.
  - `settled`: spawn, build and settle it, as S484's load does.
  - `frame`: one frame with nobody in it while awake, then sleep once at rest.
  - `hull` / `flown` / `pushed`: F26's driven-car pieces.
  - `struck`: the enable.
- `Sim::parked_cars` and `Sim::park_cars` (which also fills S461's `Sim::parked`).
- `Sim::step_all`, in this order:
  1. The awake parked cars step.
  2. The bikes gather with the cars' hulls in their world. A car with mirrors is woken.
  3. Each bike's bump carries the cars' bodies as its `planes`. The props come after them.
  4. The cars are written back after the bike's frame.
- A lone rider among parked cars now rides `step_all`. A ride with no parked cars runs as before.
- Hashed as a `parked_cars` blob only when present. Not on the wire yet (S486).

## Why that order

From `context/capture/s484/ride1.csv`:

- On the strike's row (58) the truck is awake, with vz 2.77 already set and its place not yet moved.
  - So the car's node steps before the riders.
  - The bike's strike wakes it, and the bike's resolve pushes it, in the same frame.
- Rows 68 and 70 wake it for a frame without a step. So the wake comes from someone else's strike, after the car's step.
- Enable also sets the driven word, but the car's first step clears it when nobody is in it. So the port keeps the controls undriven.

## How it was checked

`crates/game-sim/tests/replay.rs`:

- `a_bike_ridden_into_the_parked_truck_pushes_it_as_the_original`: recording2's bike, moved and zeroed as `truck.sh` does, gas held, with the truck settled at its spot. Lined up on the strike:

  | strike + | port vz | original vz |
  |---|---|---|
  | 0 | 2.742 | 2.774 |
  | 1 | 1.744 | 1.752 |
  | 2 | 1.159 | 1.164 |
  | 4 | 0.514 | 0.510 |
  | 8 | 0.102 | 0.094 |

  - Both sleep on the 9th frame after.
  - Slid 0.201 ft (original 0.203).
  - Both riders wreck on the strike and are put back about 88 frames later: port tick 146, original row 147.
  - After the put-back, both ride through the truck (ghost).
- `without_the_parked_truck_the_bike_rides_through_its_spot`: the control.

## Left open

- The port's bike pulls away from the script's standstill about 4% slower. It reaches the nose 2 ft (two frames) late, and its strike speed is 1% lower, so vz is 1% lower.
- The original's at-rest re-wakes (rows 68 and 70) are not reproduced. Filed in S486.
- The shell does not park the truck yet: P380.
