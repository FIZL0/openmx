# 1 — the end-face swing is the stand-in's engine (FINAL)

Task R16. 2026-11-17.

## What I did

- **Reproduced the swing from S146's image.** S89's `s89-along` image is gone (R32), so every traffic
  test that reads it skips. `s146-roof` has the same 21 cars at the same addresses (S146 already seeds
  them from it), so the ride was rebuilt on it. The brief's setup was: stock bike 12 ft behind the
  wagon's middle, +10 ft/s over it, 60 ticks. On tick 53 the front wheel names car 4 (the hopper ahead).
  On tick 54 the chassis is at x −135.00 ft/s, and on tick 55 at +135.00. Both are exactly the riding
  limit (`BIKE_MAX_SPEED_SQ` = 135²), so the clamp caught a much larger velocity.
- **Found where the velocity comes from.** I put temporary per-substep prints in `ride.rs`, then removed
  them. The file is back byte for byte.
  - On tick 53, substep 1, the force going into the integrate was x **−6,036,126**.
  - The front's tire forces left the force at (0, −400, 0). There was no body-ray bump.
  - The front's spin step added −8.
  - The **rear's** spin step added −6,036,118.
  - The rear was hanging (no contact of its own). It took the share path, copying the front's spring
    force (0) and its hit on car 4.
- **Why a hanging rear makes six million.**
  - Coasting stock bike: `engine_update` makes `torque = −EngineBraking·rpm²` at the 1,800 rpm floor.
    With the stock bike's `EngineBraking = 0.3` that is −894,238 (times its clutch scale).
  - While the clutch slips, the shaft is only `ClutchKineticFriction`'s +70. The revs sit above the
    road's, so the slipping clutch drives the rear forward. That is the "about 1.2 ft/s a tick, coasting"
    that S213's ride comment reads as the fresh bike.
  - Once the rear leaves the deck it spins up: road rpm 270 → 2,142 over six substeps. Its revs cross the
    engine's, the clutch locks, and the whole −894,238 × ratio goes to the rear's drive torque: wheel
    torque −6,760,443.
  - `tire::force` caps the drive force at `MaxDriveForce · load`. With load 0 the cap is 0, and the
    `cap <= 0` clause passes the drive force through whole (−6,760,443 / 1.12).
  - The next substep the clutch breaks away again, and the swing reverses.
- **The disc's number.** Every disc bike's `.phy` (`250sx/bike/{pro,race}250{a,p}.phy`) writes
  `EngineBraking = 0.00000005`, with MinRPM 4500. That is about 1 of engine braking at idle. The stock
  bike's 0.3 is 6,000,000× that. (`car.rs` scales a car's `EngineBraking` by 1e-6; the bike loader
  reads it raw, and the disc's 5e-8 is already in the bike's units.)
- **Reran with the disc's number** (the probe set `engine.engine_braking` on the `Phy` and the ride).
  From the brief's start the bike no longer speeds up, so it never reaches the front. From 10 ft ahead
  of the middle at +35 ft/s over the train:
  - the bike leaves the wagon's front;
  - it lands on the hopper's sloped back end;
  - it rolls down into the bowl (y drops about 7 ft) and up and out over the front slope;
  - the chassis x velocity stays between 82.3 and 92.4 ft/s.
- **Control, same start, the stand-in's 0.3:** the swing comes on tick 21 (−129, then +133). That is in
  the air between the two cars, with **neither wheel touching anything**. So the car contact plays no
  part in it.
- **The mover response takes the car's motion off once.**
  - The traces showed each wheel on the car with `+0x170` ≈ (34, …): the bike's 90 less the car's 55.
  - The bottom-out shove reads `chassis_velocity = vel − car` (`ride.rs` `tire_forces`,
    `wheel_contact_velocity`), so it also takes the car off once.
- **Added the test** `a_bike_rolled_off_the_wagons_front_meets_the_next_car_once` (`tests/traffic.rs`).
  - Disc braking: a wheel names the hopper on 32 ticks. Each wheel's contact x is within 10.1 of
    (bike − car), and at least 50.6 off (bike − 2·car). The chassis x never drops under 82.3, and the
    top speed is 93.8.
  - Control (0.3): x reaches −129.0 and the speed hits the 135 limit.
- **Filed S351 (low):** give the stock bike a disc-order `EngineBraking` and re-pin what moves.

## What the original does

- `engine_update` (`./kit fn 246fd0`) is the original's arithmetic bit for bit, over 10,000 generated
  states. That covers lock-up on the crossing, break-away, and the idle floor
  (`tests/move.rs` `the_engine_is_the_originals`, S15). Given the stand-in's numbers, the original's own
  step would throw the same torque. The disc never hands it such numbers.
- The tire's `cap <= 0` pass-through is `tire::force`'s port of the spin step's force law. I did not
  re-prove it here (see below).

## How it was proved

- Port measurement on S146's RAM image (21 cars seeded as the S146 replay seeds them) through
  `Sim::step`, 60 ticks, in two configurations. The control isolates the engine braking as the only
  change.
- Function oracle (existing, S15): `engine_update` over 10,000 states.
- `./kit check -- -p game-sim --test traffic`: passes.

## Not verified / not 1:1

- **The original was not recorded meeting a car's end face.** Staging it needs the bike turned along
  the train in PCSX2. S146's and S191's roof bikes lie 54–61° across the wagon, and `roof.py` moves the
  bike without turning it. There is still no pad (uinput is root-only). The swing turned out to be the
  stand-in's engine, not the contact, so nothing left points at a port bug here. A bike ridden along a
  roof onto the next car stays unrecorded (S191 noted the same gap).
- **The ride is the port's stock bike with the disc's `EngineBraking` patched in**, not a disc bike.
  Every other stock number is still the stand-in's.
- **The test reads `s146-roof`, not `s89-along`.** It skips where `context/states/s146-roof.eeMemory.bin`
  is missing, and still reads as passing there.
- **The tire's load-0 pass-through** (`tire::force`, `cap <= 0` keeps `drive_torque · inv_radius`) was
  not oracle-checked in this task with load exactly 0 and a huge drive torque. It is ported from the
  spin step; I did not look for an existing oracle case with load 0.
- **S213's comment on its own test** ("the fresh stock bike gains about 1.2 ft/s a tick, coasting") is
  left as it was. That gain is the stand-in's engine braking driving through the slipping clutch. S351
  will move it.

## Next

Nothing for R16. S351 fixes the stand-in's engine braking.
