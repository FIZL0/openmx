# 2 — the whole car step (PART)

Task S149d. 2026-10-29. Builds on part 1 (controls, CofG travel, yaw hold) and S149c's drivetrain,
now on main.

## What I did

- Appended `car::Machine` to crates/game-sim/src/car.rs: the car's body under
  `biplane::World { iterations: 1, rate: 115 }`, with one preforce, one force and one post listener:
  - **preforce, once a frame** (`car_preforce`): shock peak cleared, `Controls::step` (a returned
    `Reverse` goes to `Drivetrain::set_reverse` on the driven tires), the rear tires' friction scale
    from the hand brake, `travel_cofg`, every tire's contacts cleared, the four ground probes
    (`car_tire_probe`, `Wheel::probe`: up to 5 ground queries from the upper to the lower point until
    two normals agree), and the lone-front flag;
  - **force, every substep** (`car_tires_step`): the body's gravity, `car_anti_slide`, `car_preload`,
    `car_launch`, `Drivetrain::step`, the front tires (suspension, roll, forces), the hand-off to the
    rears, the rear tires, then `car_aero` (wings, and off the ground the control moment, damping,
    unroll/unyaw and brake gyro);
  - **post, once a frame** (`car_post` → `car_measure`): yaw hold, grounded / was grounded, ground
    speed, local velocity and rates, alpha and side slip from the acos table, euler, matrix, forward and
    up, and `car_wreck_step` (wreck lift, unwreck, the edge throw, the stuck timer), which comes back as
    `Option<CarEvent>` for the caller (thrown / reset / unwreck are the game's, not the step's).
- **Correction to part 1 and S149c's oracle lines:** 0x003a02f0 is the post listener (once a frame),
  not the force. The force listener is the tire pass 0x00397f40 itself, and `car_measure` runs once a
  frame. oracle.txt is fixed (`car_force` → `car_post`), and the S149d names are added: `car_launch`,
  `car_anti_slide`, `car_aero`, `car_wreck_step`, `car_tire_probe`, `car_tire_contacts_clear`,
  `car_contact_sort`.
- crates/game-sim/tests/car_drive.rs: S149b's three drives (context/capture/s149b/{baja,buggy,truck}.csv
  on S149a's switched images, free07) replayed two ways:
  - `the_<car>_drive_replays`: every row is seeded from the capture and stepped a frame. Compared
    against the next row: body position, velocity, quaternion and rates; the car's words (controls,
    preload/launch, grounded, speeds, local velocity, alpha, side slip, euler, matrix, forward, up); the
    engine (everything but the pitch); and per tire the probe points, drop, lever, both normals, ground
    point and normal, velocities, frame, state and loads. All of it bit for bit.
  - `the_<car>_drive_runs_on_its_own`: one machine carried from row to row, taking only the pad's
    words from the capture, reseeded only after a row the capture can't step to.

## How it was proved

`./kit check -- -p game-sim --test car_drive`, all 7 green, every compared word bit-exact:

| drive | rows stepped | skipped | torn |
|---|---|---|---|
| baja | 641 | 8 | 0 |
| buggy | 648 | 1 | 0 |
| truck | 603 | 46 (8 struck) | 1 |

Both modes give the same counts. A row is skipped if its frame counter isn't +1, or if the engine's
sound clock (+0x134) didn't move by exactly one frame's n·h, which catches paused rows and two-frame
rows. Coverage, counted with a temporary print: air control ran (truck 222, baja 66, buggy 10 rows
summed over both modes), each car's one wing, and the brakes, the launch and preload's ready test.

The row phase matters. The input step runs *after* the world step, so a row's pad words are what the
next frame steps on: seed and inputs both come from row N, compared against row N+1. Taking the inputs
from N+1 failed at baja fc 410 (throttle 0.1333 against a row-410 gas of 0).

## Not verified / not 1:1

- **The car's collision object.** The truck runs into free07 scenery at fc 720–727 (129 → 21 ft/s in a
  frame; +0x2cc touch-not-grounded is set at 727). The car's collide (its .col against the scene,
  vtable 0x00454a00 +0x7c, object at car +0x170) is not ported, and neither is the resolve call in the
  body's force (`body_wrapper_force` → `collision_record_resolve`). Those 8 rows are excluded
  (`STRUCK`). Filed as **S224**.
- **The contact sort (`car_contact_sort`) and `touching`.** Not ported: no S149b row has a contact, so
  `touching` stays false. That leaves the stuck timer's counting path and the touching branches of
  aero/unwreck unexercised. Other objects' contact velocities are taken as 0.
- **The truck's fc 1105 is torn.** Its tire blocks hold probe points about a frame's travel behind the
  body (FL upper z 1044.94 against the port's 1047.85 at ~87 ft/s), beside a body that matches exactly.
  car_tire.rs already lists it in TORN. Its body, car and engine words are compared; its tire words are
  not.
- **Preload never fires.** Ready is reached (truck 172, baja 158, buggy 78 rows), but no drive arms it
  (lean past 0.8) or releases it, so the armed/timer/force path is ported from the disassembly only.
- **Wrecks.** No drive wrecks, so the wreck lift, unwreck and the edge throw (`position_query`, 150 ft)
  are unproved. The edge throw's random throw isn't part of the step anyway; it comes back as
  `CarEvent::Thrown` for the caller.
- **Animation, effects, landing sound and the class's +0x2e0 step** (rider animation only) are left out,
  being outside the simulation. Shift events from the drivetrain are ignored by the step.
- **The engine's pitch** isn't compared: it reads the sound noise, and the test passes a 0 noise.
- **Driver kinds.** Only human = true and steering wheel = false are proved. The AI CofG and the wheel's
  steer path come from part 1's function oracle, not from a drive.
- **The tire frame's w lane** is stack garbage in the original and taken as 0. It is never read.
- **No `Machine::new`.** The test seeds the machine from RAM, so the constructor's defaults for a fresh
  car spawn aren't ported. A ride using `Machine` needs them (part of wiring cars into the ride, not
  this task).

## Next

S224 (the collision object). After it, wire `Machine` into the ride with a constructor from the car file.
