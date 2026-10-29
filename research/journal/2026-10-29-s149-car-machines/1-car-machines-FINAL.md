# 1 — car machines (FINAL)

Task S149. 2026-10-29.

## What I did

- S149 is far bigger than one session: a car step, four tires with 16 surfaces each, an engine and
  gearbox, then models, cameras, audio and particles, for three machines. Following AGENT.md, I split it
  first and did the first piece. That piece is **the shared car `.phy` loader**, ported and proved word
  for word against the original. Everything else is filed as S149a–S149e: captures of each car being
  driven, the tire model, the engine and gearbox, the car step with S149's own "a captured drive replays
  bit-exact" test, and the app (parking, switching, models, cameras, carengine.aud, particles).
- `crates/game-sim/src/car.rs` (new): `Car::read(ini, Kind)` reads `[Mass Properties]` through
  `phy::Part`, then the four tires and their surfaces, `[Engine]`, `[Aerodynamics]` and wings,
  `[Preload]`, `[Wrecks]`, `[Joystick]`, and each machine class's `[Animation]`. It keeps what the
  loader works out on the way: spring and damper rates against the part's mass, radius²/1/r, 1/inertia,
  1/A2 and 1/A4, the torque table, first gear's ratio and the driven tires' top wheel speed, and the unit
  conversions.
- `crates/game-sim/tests/car_phy.rs` (new), with three tests:
  - `monster_truck_phy_matches_the_parked_truck`: 1269 words of the truck free09 parks, in
    `first_recording.p2m2_SaveState.eeMemory.bin`, against the port's parse of
    `MonsterTruck\monstertruck.phy` from machine.res. All bit-exact. I checked that it really runs rather
    than skips by forcing a panic at its end.
  - `the_buggy_and_the_baja_read`: both files are in machine.res under the names `Kind::phy` gives, and
    they read to what they write (the buggy has 5 gears, the keys the truck has and they lack fall back to
    the defaults, and the typo surface below).
  - `a_changed_surface_key_moves_its_word`: a synthetic file, for the surface count carrying over between
    tires.
- `research/oracle.txt`: an S149 block. It has one name line (`car_phy_load`); the rest of the field map
  is in comments, as the audit memory asks.

## What the original does

The loader is `./kit fn 0x00394450` (MXCar.cpp). The truck class calls it from `./kit fn 0x003e0200`, and
the buggy/baja class from `./kit fn 0x0039e7d0`. In order:

- The rigid-body part reader runs on `[Mass Properties]`. The car copies the part's `CofG` twice, as its
  base and its current centre of mass. `CofGAI` defaults to that `CofG`. Then come the travel limits,
  yaw-rate limits and handbrake keys, every one with a default of 0.
- Each tire section (`FrontLeftTire`, `FrontRightTire`, `RearLeftTire`, `RearRightTire`) reads into
  temporaries with defaults (1 for the compression keys, radius, inertia and gyro; `f32::MAX` for
  `MaxTireAngleRate`; 0 for the rest), then hands them to five setters:
  - Spring: `|32.2*(k*m)|`, against the **collision** body's mass.
  - Dampers: `(2*ratio)*sqrt(m*k)`. The disassembly has no `madd.s`, so this is plain mul/sqrt.
  - Radius, r², and 1/r, which is written only when r > 0.
  - Width.
  - 1/inertia, or 0 when the inertia is not positive.
  - Steer lock × DEG.
  - ABS and its slip window.
  `NumberOfSurfaces` is read with the getter that leaves a missing key alone, into **one** variable all
  four tires share. Surface sections `"<tire> Surface %d"` read every key with a default of 0, and the
  setter stores only indices below 16. `A2` and `A4` are stored as unguarded reciprocals.
  `VelocityFilter` is read on the front left tire only, into a temporary nothing uses.
- `[Engine]`:
  - `DriveTrain` picks the four drive slots: 0 = the front pair, 1 = the rear pair, anything else = all
    four.
  - The RPM/HP table becomes torque: `(hp*5252)/rpm + (k*rpm)*rpm`, where `k = EngineBraking*1e-6`, and
    0 for a non-positive rpm. The last RPM is kept as the redline.
  - The gearbox is put in first: ratio = `final*gear1`, plus its reciprocal, and each driven tire's top
    wheel speed = `(redline*0.1047198)*(1/ratio)`.
  - Then inertia (and its reciprocal when positive), loss × 0.01, MinRPM, IdleThrottle, the torque
    converter (`PumpTorqueScale × 0.001`), the shift points, NOX and TCS.
  - `HighLoadRPM` and the three clutch keys use the leave-alone getter.
- `[Aerodynamics]`: the airborne keys, `NumberOfWings`, and `Wing %d` (slope, AoA × DEG, drag, induced,
  position).
- `[Preload]` (leave-alone getter), then `[Wrecks]` `MaxWreckTime` (default 5).
- `[Joystick]`: rate limits, `MaxAnalogThrottle` (default 1), `DigitalThrottleSpeed` × MPH,
  `DigitalSideSlipAngle` × DEG, the three steer tables, then the rear steer (`RearSteerScale1`,
  `RearSteerSpeed2` and `RearSteerScale2` default to 1). Every speed is × MPH after the reads.
- Last, the tire collision volume is built from the model's anchors, not from the file.
- After the loader, each class reads `[Animation]` `PelvisBoxPosition`/`PelvisBoxSize`. The truck alone
  also reads `RearTireSteerScale` (default 1).

The loader never reads `[Skidding Limits]`, `[Camera Properties]`, `ImpactVelocityDamping`,
`ImpactAngularRateDamping` or `ClutchCofGTravelBack`.

The retail buggy and baja both have a typo, `s[FrontRightTire Surface 10]`. The reader takes that line for
neither a section nor a pair, so surface 10's keys append to surface 9, where surface 9's own keys win
(first match). Surface 10 is left all defaults, so its 1/A2 and 1/A4 are the EE's 1/0.

## How it was proved

- **A save-state capture**: `crates/game-sim/tests/car_phy.rs` compares 1269 words of the monster truck
  parked at the object 0x00e1e210 in first_recording's RAM image (vtable word +0x48 = 0x0045ed50, kind
  +0xd50 = 4) against the port's parse of the disc's file. The words cover:
  - the part's words on the car, body and collision record;
  - every tire field above, plus all 16 surfaces × 4 tires;
  - the engine fields, its rpm/torque/gear arrays through their pointers, and the drive slots as tire
    pointers;
  - the wings, the preload, clutch, wreck and joystick words, the steer tables, and the truck's
    `[Animation]`.
  All 1269 are bit-exact.
- The only word left out is the car's current CofG (+0x250). The loader sets it to `CofG` (3.5), but the
  parked truck holds `CofGAI` (1.0) by now, so it is the step's word, not the loader's.

## Not verified / not 1:1

- **The buggy and the baja are not proved against a RAM image.** There is no state with either one loaded
  (free07 parks the baja, free11 the buggy). They run the same loader as the truck; their class differs
  only in reading no `RearTireSteerScale` and putting the pelvis box at a different offset. Their files
  only use keys the truck proof covers, and leave out some (the defaults for those are read from the
  decompilation, not seen in RAM). Filed as S149a.
- **The typo surface's 1/0 is not seen in RAM**: the buggy's/baja's front-right surface 10 = `F::MAX` for
  1/A2 and 1/A4. It follows ps2-float's division by zero, which is proved elsewhere, but not on this
  word.
- **Missing-key cases the retail files never hit, taken from the decompilation and not run:**
  - A first tire without `NumberOfSurfaces`: the original reads an uninitialised stack word; the port
    says 0.
  - A missing `HighLoadRPM`: the original passes stack garbage; the port keeps `None`.
  - An empty RPM table: the original reads the word before the allocation as the redline; the port says
    `None`, and 0 in the top-speed product.
  - The gearbox only enters first when the engine's shift state is still 0 (true after construction).
    The port assumes that.
- **Left-alone fields are `Option`s**: the clutch, preload and `HighLoadRPM` keys. With a key missing, the
  port does not know the constructor's value, which a stepper will need. The truck's file has them all.
- **Not ported:**
  - The tire's anchor shift by width (tire +0x80) is the model's attachment point, so it is not compared.
  - The tire collision volume the loader builds last (`./kit fn 0x003942d0`) comes from the model, not
    the file.
  - `VelocityFilter` is dropped, as the original drops it.
- **Not done in this session, filed as S149a–S149e:** the car step, tire model, engine model, captured
  drives, models, cameras, carengine.aud, particles, and parking/switching in the app. S149's own test
  ("a captured drive of each replays bit-exact") lives in S149d.
- `./kit done` needs `--skip-audit`: the audit's 7 leaks are other tasks' addresses in plan/DONE.md
  (radar_update, biplane_tire_*). None are this task's.

## Next

S149a: make the captures. Boot free07/free11/free09 in PCSX2 copy 3 (S133's boot.py in s1 boots a track
by PINE). Switch to the machine from the pause menu, take a RAM image, then drive a generated .p2m2 and
record per-tick rows. With the images, extend `car_phy.rs` to the buggy and the baja: the class is at
+0xf20/+0xf30 for the pelvis box, the rest is the truck's layout.
