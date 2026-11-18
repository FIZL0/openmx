# 1 — nose down drive grip (FINAL)

Task P373. 2026-11-18.

## What I did

Decided **yes**: Car acceleration over 100% keeps a car's nose down, and with that the baja's forward
drive grip from P370b is back.

- `car::Tire::drive_grip` (not a key, 1 as read). `Mods::car` sets it to k on the driven tires over
  100% (stock cars only; MX vs. ATV's cars' knob is untouched).
- `Wheel::drive_grip` (`car.rs`), run from `Wheel::forces` only when it is over 1: while the tire
  pushes the car forward on a forward-spinning wheel, its forward force goes times
  `min(k, b / (h mu))`. `b` is the centre of mass's distance ahead of the contact along the ground,
  `h` its height over it, `mu` the surface's friction. That is the grip at which the drive force,
  with all the weight on that axle, lifts the other one. Read off the car's stance each substep,
  since the model's tire positions are not in the `.phy` that `Mods::car` sees.
- The cap may go under the file's grip. The monster truck's file is already past its lift line
  (rear mu 3.15 against b/h about 2.5 at rest), so over 100% its forward grip is held at about 0.8
  of the file's. That is what keeps it down.
- `last_force` gets the raised force too, so the wheel spins down on it.
- Tests (`tests/car_modifiers.rs`): `a_car_taken_on_the_gas_over_100_is_not_held` now asks 0.8x at
  1 s (was 0.5x). New `a_car_on_the_gas_over_100_keeps_its_nose_down`: every 20% 120-500, 3 s on
  the gas, never all four tires off, fronts up at most 8 ticks, faster at 3 s than at 2.

Why yes:

- The drive force does not tilt with the body. `Wheel::forces` builds the tire frame with
  `math::basis(forward, ground_normal, orthogonalise, keep_forward = false)`, so the forward axis is
  in the ground's plane. The rearing comes from the force acting at the contact patch, below the
  centre of mass (the moment `lever x force`). Both are the ported code, so the original does the
  same given the torque.
- But the knob is ours. Its job is "how fast the cars get up to speed". A truck that goes over onto
  its tail on a straight take at the knob's top does not get up to speed. Holding the grip at the
  lift line costs the monster little: 3 s speeds go up with k (79 at 100% → 114-117 from 300%).

Rides on the gas from the take, ft/s at 1 / 2 / 3 s (probe, every 20%, not kept):

- baja: 100% 36/63/82; 120% 31/75/97 (worst 1 s, 0.86x; was 0.67x at 200%); 200% 52/100/131;
  500% 79/145/190. Fronts up 1 tick, 5-7 from 440%.
- buggy: 100% 22/45/60; 500% 68/89/111. Never fronts up.
- monster: 100% 36/61/79; 300% 36/79/117; 500% 36/78/114. Fronts up 0-5 ticks, never off. Before:
  fronts up 7-26 ticks from 240 to 480%, over at 500% (3 s 85).

Nothing taken from `../opentennis` (no cars there).

## What the original does

Nothing to port: the knob is ours. The tire frame and the force at the contact point are the ported
`Wheel::forces`; at 100% (drive_grip 1) the new code does not run.

## How it was proved

`./kit check -- -p openmx --test car_modifiers --test mxvatv_car_modifiers`: all pass on the disc's
`machine.res`, flat floor. The 100% path is untouched (the raise is skipped at 1), so the replay
tests guard bit-exactness.

## Not verified / not 1:1

- The lift line is quasi-static (b / h from the stance each substep). Pitch and suspension dynamics
  still let the fronts up a few ticks (monster at 200-240% and 420%+, baja from 440%). Not tuned
  further: the test bounds it at 8 ticks.
- Only straight takes on a flat floor were measured. Turns, slopes, bumps and jumps with the raise
  were not ridden. On a slope b / h changes with the ground normal, which the cap reads, but no ride
  checked it.
- Reversing keeps the file's grip (the raise only works forward). Not measured whether the knob's
  torque tips a car backwards in reverse.
- `a_clutch_drop_over_100_is_not_held`'s doc numbers (the buggy at 490%, 22 against 34) are from
  before the raise. The test still passes; the numbers were not re-read.
- Nothing here is checked against the original. It never runs a stock car on k times the torque.

## Next

none
