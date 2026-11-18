# 1 — stock car launch and accel (FINAL)

Task P370. 2026-11-17.

## What I did

- Checked whether MX Unleashed's stock cars' **Car acceleration** knob over 100% holds a freshly taken car
  through its launch, as P368 found for MX vs. ATV's cars. **It does not.** Code left as it was;
  `crates/openmx/tests/car_modifiers.rs` pins the finding.
- A freshly taken stock car's launch is already over. The shell's `Machine` starts the launch timer at its
  length, and `launch_step` only resets it when the clutch is pressed. On the gas from the take, the boost
  is 1 on every tick, at 100% and over (timer 1.758 against the baja's 1.75).
- The only way to start the launch is a clutch drop (L1 is the clutch). I swept settle 2 s → clutch and gas
  1 s → let the clutch out on the gas, at 110–500% in 10% steps, on all three cars:
  - Launch kept (the code as it is): nothing held. 1 s after the drop: buggy 34–39 ft/s against 34 at
    100%, except 22 at 490%; monster 72–77 against 70; baja 37–44 against 41.
  - Launch off over 100% (P368's rule, tried here and backed out): the monster is much slower at 110–200%,
    52 against 72 ft/s at 110%. The buggy at 490% only gets to 25, so its dip is not the launch either.
- A slow start that is not the launch: on the gas from the take, the baja is at about 11.7 ft/s 1 s in at
  every value from 110 to 500%, against 28 at 100%. The monster is at about 21 against 35 from 180%. The
  shell takes the car about 4.7 ft over the floor. Its rear wheels spin up in the drop (23 against 14 at
  100%), swing between forward and back for about 10 ticks after landing, then spin. That is outside this
  task's question (the launch), so it is filed as **P370a**.
- Tests in `crates/openmx/tests/car_modifiers.rs` (new):
  - `car_acceleration_leaves_the_launch`: each car's `MaxClutchTorqueScale` / `ClutchTorqueScaleTime` are
    the file's at 50/110/150/500%.
  - `a_freshly_taken_car_has_its_launch_spent`: gas from the take, 60 ticks, boost 1 and timer past the
    launch at 100/110/150/500%, all three cars.
  - `a_clutch_drop_over_100_is_not_held`: the sweep above. 1 s after the drop, speed is at least half of
    100%'s; 2 s after, at least 0.8 of it. A held car is under 12 ft/s at 1 s.
- opentennis has nothing on cars; nothing taken.

## What the original does

Nothing for the knob: Game Modifiers are an addition. The launch step is the port's `Machine::launch_step`
(`crates/game-sim/src/car.rs`), already proved by the car tasks before this one. This task did not change it.

## How it was proved

- Rides of the port only, on the disc's `machine.res`: `cargo test -p openmx --test car_modifiers`, 3 tests
  plus `vehicle.rs`'s 10, all pass. Numbers above are from throwaway `--nocapture` probes, which I deleted.
- With the launch-off rule in place, the gas-from-the-take numbers were bit-identical to without it. That
  shows the launch plays no part there.

## Not verified / not 1:1

- **Not run against the original.** The question is about an addition (the knob), so there is nothing in
  SLUS-20888 to match. The rides are the port's only.
- **Whether the original also takes a stock car with its launch spent** was not checked against a capture
  here. The port's `Machine` starts the timer at the launch's length; I took that as already proved.
- **Only a flat floor and one clutch hold (1 s), released after 2 s settled.** Other holds, slopes and
  speeds at the drop were not swept.
- **The slow start on the gas from the take over 100%** (the baja, and the monster from 180%) is not fixed.
  It is a different cause (rear wheels spun up in the drop), filed as P370a.

## Next

Nothing.
