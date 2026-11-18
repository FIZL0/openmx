# 1 — take height and accel (FINAL)

Task P370a. 2026-11-18.

## What I did

- **Decision: the take sets the car down lower; the knob is left alone.** The take was wrong at every
  value, not just over 100%.
- Cause: `CarRide::rest_height` (`crates/openmx/src/vehicle.rs`) put a tire's bottom at its node less
  **two** radii. The car's suspension hangs the axle's bottom **one** radius under the node
  (`Wheel::drop`, `game_sim::car`). So each car was stood one radius higher than meant, plus the 0.5 ft
  `CAR_DROP`. The baja fell from 3.88 ft and first touched at tick 12; the monster at tick 13.
- Fix: one radius. Now each car is first down by tick 5-6. Baja rest 1.43 ft, monster about 0, buggy
  0.62.
- Gas from the take, speed 1 s / 2 s in (ft/s), after the fix, every 30% from 110:
  - Buggy 100%: 22.5 / 45.5. Over: 17.8-31.3 / 48.4-63.5.
  - Monster 100%: 36.0 / 61.4. Over: 39.3-50.0 / 66.7-106.3.
  - Baja 100%: 36.7 / 63.9. Over: 24.6-32.5 / 65.1-73.3.
  - Before: baja 11.7 against 28 at every value; monster 21 against 35 from 180%.
- The baja is still a little slower than 100% a second in (0.67x at its worst, 200%). That is wheelspin:
  more torque than its rear tires carry. By 2 s it is past 100% at every value. Not "held".
- Tried and dropped: the tires' inertia x k over 100% (to calm the spin swing). Mixed: the buggy and
  baja better at some values, worse at others, the monster slower. Not worth a change to the knob.
- The brief's other idea (drive torque x k only while grounded) does not fit: with the take fixed, the
  swing that is left happens on the ground.
- Tests (`crates/openmx/tests/car_modifiers.rs`):
  - `a_taken_car_lands_at_once`: each car first down within 6 ticks of the take.
  - `a_car_taken_on_the_gas_over_100_is_not_held`: every 10% from 110 to 500, 1 s in at least 0.5x
    100%'s, 2 s in at least 0.9x.
- opentennis has no cars; nothing taken.

## What the original does

Nothing to match: the take's height is the port's own (S149e; S245's bays use the same height). The
original swaps into a car at its `machine.txt` spot. Game Modifiers are an addition.

## How it was proved

- Rides of the port on the disc's `machine.res`, `./kit check -- -p openmx --test car_modifiers` plus the
  car_tire_tracks, car_particles, wrap_machines, machine_modifiers (P272's ride numbers), car_engine_sound,
  online_car, car_edge_throw, machines_everywhere and menu: all pass (695 tests). Sweep numbers come from throwaway `--nocapture` probes, deleted.

## Not verified / not 1:1

- **Not run against the original.** The take height and the knob are the port's own; there is nothing in
  SLUS-20888 to match.
- **Every car take moved, at 100% too**, and so did parked cars' bay heights (`Lot::stand_cars` uses the
  same height). Not looked at on screen; the parked cars should now stand on their tires instead of a
  radius over them, but that was not checked by eye.
- **The baja's wheelspin over 100%** (up to 0.67x of 100%'s speed a second in) is left. Fixing it needs
  more drive grip with the torque, as the ATV knob does; filed as P370b.
- **Only a flat floor.** Slopes and bays on real maps not swept.

## Next

Nothing.
