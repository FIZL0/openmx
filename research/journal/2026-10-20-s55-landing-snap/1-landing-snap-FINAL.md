# 1 — landing snap (FINAL)

Task S55. 2026-10-20.

## What I did

- Looked for the snap in the simulation, the shell and the drawn pose. Then followed openmx-41's note
  (P52) and checked a steady headless clock first. A one-frame snap is there even at a steady clock,
  so not all of it belongs to P52. It is in the **drawn chassis**, not the simulation.
- **The cause.** In the air, `bike_air_*`'s root channel twists the chassis' `Bike` node by the
  crossup sweep (45° at a crossup of −0.6 rad). On touchdown the vehicle machine restarts into a
  grounded state (30), and its blend fades out of the snapshot over a ramp (0.933, falling about
  0.067 a frame). The bike archive has no grounded clip, so `poses_the_chassis` is false (V20).
  `Chassis::drive` then drew the rest shape at once and threw the ramp away. The bike, and the rider
  drawn on it, turned 45° in one frame.
- **The fix** (`crates/game-render/src/vehicle.rs`):
  - On a no-clip frame, `drive` draws rest faded from the restart's snapshot by the blend's ramp
    (`fade_rest`, the same slerp/lerp as `Clip::pose_faded`).
  - The snapshot is now the pose last *drawn* (`Chassis::shown`), not the stale airborne clip's.
    Otherwise a take-off after a grounded spell would fade from an old crossup.
  - Now: 45.16° → 42.15 → 39.14 → … → 3.01 → 0 over 14 frames.
- **The simulation does not snap.** The original's ten touchdowns in recording2-tricks.csv were
  compared with the port's own landings on a steady clock. Rows 765 and 1830 are put-backs and row
  1188 is a wreck (reason 12), so they are left out. `research/s55_landing.py recording2-tricks`
  prints the rest.
  - In the original, the heading and roll ease through a touchdown: at most 10.54°/tick of yaw
    (row 222) and 13.25°/tick of roll (row 1657).
  - The velocity swings onto the heading in one or two ticks. Row 341 goes 46.5° → 2.6°, row 1614
    goes −25° → +2.3°. Two ticks after touchdown the crossup is at most 7.05° (row 1616).
  - The port does the same: its velocity swings in one tick, at a speed loss of about cos(crossup),
    and its roll and yaw ease.
- Tests:
  - `crates/game-sim/tests/ride.rs` `it_lands_crooked_without_a_snap`: race125p off P29's 20° jump.
    One landing is sideways (stick 0.5, 30.5° crossup); the other is rolled 27° with a 24° crossup
    (stick −0.5). Every tick from 3 before to 8 after must stay inside the original's per-tick
    envelope, and the crossup must be inside 7.05° two ticks after touchdown.
    - Sideways, yaw/roll/crossup: +29.6/−11.3/−30.5 → +28.4/−13.4/−27.5 → +20.2/−11.6/−3.9 →
      +17.0/−11.9/+6.5.
    - Rolled: −37.2/+26.8/+24.4 → −27.6/+23.0/+4.7 → −31.8/+21.6/−0.2.
  - `crates/game-render/tests/vehicle.rs` `a_crossed_up_landing_eases_the_bike_square`: no frame
    turns the `Bike` node more than 5°. It fails on the old code with 45.16° in one frame.
- `./kit check`: TESTS PASSED.

## What the original does

- Simulation: see the capture numbers above. The landing is not a special case to the eye. The ground
  bites, the slide goes in one or two ticks, and the heading and roll come round over the next ten.
- Drawing: V20 reads the original's blend as keeping the last pose when a state has no clip for a
  node. Fading it out over the restart's ramp is the same machine the rider's body already uses.

## How it was proved

- The simulation was compared with the original's measured envelope in recording2-tricks.csv: nine
  landings, through `research/s55_landing.py`. It was not compared tick by tick (see below). Two port
  landings (sideways and rolled) are in `it_lands_crooked_without_a_snap`.
- The drawing was checked in the test above, which fails before the fix and passes after it. The
  fade itself is matched by eye, not to a capture.

## Not verified / not 1:1

- **No tick-by-tick replay of a captured landing.** The port's ride diverges from the capture from
  tick 1 (S32). The capture has no angular velocity, so the port can't be seeded at a touchdown.
  The test bounds the port by the original's envelope instead of matching it bit for bit.
- **The sideways landing overshoots.** The port's crossup goes −30.5° → −3.9° → +6.5° and creeps on
  to +11.0° five ticks later. The original's largest after-landing crossup in this window is about
  7.5° (row 345 on). It is inside the two-tick bound, but the later drift is about 3.5° wider than
  any of the original's nine landings. Not chased: it is a tire/slip matter (P33's area), not a
  snap.
- **The chassis fade is not proved against the original.** The original's drawn chassis animation
  object was never found in RAM (P29). Fading over the restart's ramp is my reading of V20, matched
  by eye.
- **The rider body's pose across touchdown was not measured.** A probe (body.rs scratch) stayed in
  state 53, and its world matrices are skeleton-local. The rider is drawn at the chassis placement,
  so the chassis fix carries the rider's apparent twist, but its own blend was not checked.
- **The camera was not checked** at touchdown.
- **Wall-clock dt (P52).** The shell steps once per drawn frame, with a 30-frame dt average capped
  at 1/30 s. A slow frame stretches dt and can add a snap of its own. Per openmx-41, that is P52's
  and was left alone.
- The shell was not watched by eye after the fix. It was built (`cargo build --release -p openmx`),
  and the drawing path was covered only by the game-render test.

## Next

- P52 for the dt half. If a snap is still seen after P52, look at the rider body's blend at the
  touchdown restart, and at the camera.
