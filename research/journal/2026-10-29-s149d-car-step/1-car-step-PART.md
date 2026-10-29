# 1 — car step (PART)

Task S149d. 2026-10-29.

## What I did

- Mapped the car's once-a-frame and per-substep listeners (names in research/oracle.txt's S149d block,
  at its end): the preforce (`car_preforce`) runs the body part's preforce, the controls step
  (`car_controls`, vtable +0x1a0), the centre of mass's travel (`car_cofg_travel`, vtable +0x1a8), clears
  the tires' contacts, sorts the collision contacts onto the tires and runs the four ground probes; the
  force listener (`car_force`) runs the measure (`car_measure`), then the class's force step (vtable
  +0x2e0: buggy/baja and truck each have their own), then three more steps; the tire pass (S149b) follows.
- Ported, appended at the end of crates/game-sim/src/car.rs (so S149c's Drivetrain, inserted after
  `impl Engine` on its branch, does not conflict):
  - `Controls` with `Controls::input` (the vehicle's input step: stick, lean, pad word, gas analog
    below `DigitalThrottleSpeed` or past `DigitalSideSlipAngle` else digital, brake, hand brake
    (command 0x16, which bindings.rs has no name for — a local const), clutch, d-pad steer and lean) and
    `Controls::step` (steer rate limits for stick/d-pad, wheel straight through, squared steering, the
    three steer tables by forward speed, rear steer, brake and gas rate limits, both pedals = reverse at
    a 0.4 throttle with the brakes off, the clutch, the hand brake's rear friction / rear brake by
    ground speed / yaw limit / fade). The gearbox request comes back as `Option<Reverse>` for the caller
    to hand S149c's `Drivetrain::set_reverse`.
  - `travel_cofg` (CofG or CofGAI by human driver, z slid by the lean times CofGTravelForward/Back,
    body position and last position moved by the turned change).
  - `hold_yaw_rate` (the measure's first act: body-frame yaw rate held to the live limit).
- crates/game-sim/tests/car_step.rs: each of the four against the original under the interpreter.

## What the original does

- `./kit dis 0x00396870` (controls): no driver → clears stick, lean, pad word, pedals, steer input,
  throttle, steer, hand brake; brakes 1; yaw limit = file's; engine reverse off. Rear steer is *not*
  cleared there (the port keeps that). The d-pad branch comes before the wheel branch: a d-pad car on a
  wheel controller steers by the d-pad table. Squared steering writes the squared stick back over +0x4b8.
  The hand brake writes its friction to both rear tires' +0x9b4.
- `./kit dis 0x00396490` (input): every command's pressure is the pad record's +4 at the index
  `pad_control_index` returns; analog gas is remap(pressure, 0, MaxAnalogThrottle, 0, 1), 0 when the max
  is 0. Up wins over down; left over right; only a stick to the right (> 0) clears the d-pad flag.
- `./kit dis 0x00397390` (travel): human = controller set and controller +0x34 == 1.
- `./kit dis 0x00398150` first block (yaw hold): `|y| <= limit` keeps it, else limit × ±1 (int sign).

## How it was proved

Function oracle, tests/car_step.rs, on S149a's three switched images with S149's captured drives
(context/capture/s149b/{baja,buggy,truck}.csv, 650 rows each):
- the port's file values first checked word for word against the car in RAM (offsets of every
  joystick/hand brake/travel/CofG field);
- controls step: every row × wheel on/off (1,300 per car), plus 10,000 generated states per car (half
  with a made-up file: equal table speeds, zero rate limits, zero MaxAnalogThrottle…; dt 0, 1/60, 1/30,
  random; reversing; no driver); every written word and the reverse call compared — 0 off;
- input step: 650 rows + 10,000 generated pads per car — 0 off;
- CofG travel: 650 rows + 10,000 generated (lean ±1/0/−0/random, human or not, random last cofg) — 0 off;
- yaw hold: the measure run until its second call (hooked to end it), 650 rows + 10,000 — 0 off.
Checked they really ran (a temporary panic after the tally printed the counts in check.log).

## Not verified / not 1:1

- **The task's test — a captured drive of each car replayed bit-exact — is not written.** It needs
  S149c's engine (`car::Drivetrain`, on origin/task/S149c, not on main) and the rest of the car step
  ported: the ground probes (`./kit dis 0x0039b090`) and contact sort (`./kit dis 0x00397120`) against
  free07's ground, the rest of the measure (speed, local velocity/rate, the angles from the acos table,
  grounded/landing, normal query), the class force steps (`./kit dis 0x0039edb0` buggy/baja,
  `./kit dis 0x003e07b0` truck: airborne damping and control moment, unroll/unyaw, brake gyro torque,
  the wings), the three steps after it (0x0039ed00, 0x0039e9c0, 0x0039e850), preload
  (`./kit dis 0x00397c60`), and the wreck time. None of those is ported.
- The input step's pad mapping is stood in for: the test's hook makes command N's record N, so the
  binding table (`pad_control_index`) is not part of this proof (S-series bindings tests own it).
- The controls step's call to the engine's reverse switch is hooked out (only its argument compared);
  what the switch does is S149c's.
- NaN inputs are not generated: `hold_yaw_rate` uses `limit < |y|` where the original keeps on
  `|y| <= limit`, which differ only for NaN.

## Next

When S149c has landed: port the measure's remainder and preload, then the class force steps (each with
a function oracle like car_step.rs's), then the ground probes and contacts, and wire a `car::Step` that
runs preforce / force / tires / engine through `biplane::World` for the drive replay of all three
captured drives.
