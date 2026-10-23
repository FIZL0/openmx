# 1 — plane accel brakes (FINAL)

Task P116. 2026-10-23.

## What I did

- **Measured first** (port, full `Flight::frame` at 30 Hz on a level floor; and once in PCSX2, see below).
  Before the change, on the runway: from rest, Cross until 50 ft/s, then Square to a stop:
  | Biplane acceleration | 0–50 | Square → stop | peak while held back |
  |---|---|---|---|
  | 100% | 86 ticks | 162 | 65.7 ft/s |
  | 200% | 58 | **252** | 82.0 |
  | 50% | 150 | 131 | 50.6 |
  So the 200% plane stopped *later* than the file's. The reason: held back, the throttle closes at
  `ThrottleIncrement` (0.35/s, about 2.9 s from full), and all that time it still pushes. At k = 2 that
  push is doubled (`MaxPower × k`) while the brakes stay the file's (`MaxBrakingForce` −500 per front
  wheel). So the plane keeps speeding up for longer before the brakes win.
- **What sets how the plane slows down**, item by item from the brief:
  - **`ThrottleIncrement`.** Unscaled before; now × k, both ways. The throttle opens and closes on a
    k-times clock.
  - **`ReverseThrottle`, and whether held back means reverse thrust.** Square alone is not reverse.
    The control law closes the throttle and applies the brake. A throttle below zero is reset to 0 and
    then closed one step, so held back it sits at −1 step (−0.0117 at 100%: the original's capture
    shows exactly this). Reverse (`ReverseThrottle`, −0.2) needs Cross+Square on the ground. Its
    thrust is `MaxPower × throttle`, already × k, so nothing was added. The −1-step creep is × k²
    (k for the step, k for the power), which is small: it rolls a stopped plane back at a crawl.
  - **The wheel brakes** (`MaxBrakingForce`, and `BrakeFriction`, the share of it that applies as
    rolling friction with the brake off). Unscaled before; now k times as a force. Below 100% the
    file's word is scaled. **Above 100% the extra (k−1) goes through the centre of gravity**, not the
    contacts. The first try scaled `MaxBrakingForce` at the contacts, and at 200% that stood the plane
    on its nose: the nose's `y` went to −0.998, then it fell back and rocked at up to 12 ft/s. Its weight
    is the file's, so twice the braking moment about the main wheels beats it. Through the centre of
    gravity, 200% puts the nose to −0.146 (100%: −0.043) and 500% not below +0.09.
    - `phy::Gear::cg_braking` (not a file key, zero as read).
    - `Flight::cg_braking` (wired in the snapshot). The state walk covers it under `plane.cg_braking`,
      but only when it is non-zero, so the file's plane hashes as before. `Sim`'s pinned size moved
      from 11336 to 11344 (tests/hash.rs).
    - `Tire::braking(times)`: the rolling-line part of the friction `Tire::roll` last pushed at the
      contact, as a world force.
    - `Flight::forces` adds `cg_braking ×` that after the tires' forces, only when it is non-zero.
  - **The idle throttle's thrust.** None to scale: the power is `MaxPower × throttle`, so a shut
    throttle gives no thrust at all.
- **`Flight::retune` and `Flight::put_back`** now also carry `ThrottleIncrement`, the tires' braking
  force, `cg_braking`, and `CDde`. P95 scaled `CDde` but retune never took it, so an in-flight change
  left the elevator's drag at the old value. `openmx/tests/remodify.rs`'s "retuned = built" test now
  runs PlaneAccel at 200% and 50% as well.
- The knob's notes: `fields()` adds "[Joystick] ThrottleIncrement x k; every tire's MaxBrakingForce x k
  (above 100%, the extra through the centre of gravity)". The menu blurb is now "the biplane's pick-up
  and braking, not its top".
- **After the change** (`crates/game-sim/tests/acceleration_modifiers.rs`,
  `biplane_acceleration_brakes_with_it`; on a floor at the runway's 138 ft, see the PCSX2 note):
  | Biplane acceleration | 0–mark | Square → stop | in the air, closed throttle → half speed |
  |---|---|---|---|
  | 100% | 78 ticks (30 ft/s) | 114 | 9.77 s |
  | 200% | 39 (×0.500) | 56 (×0.491) | 4.87 s (×0.498) |
  | 50% | 157 (×2.013) | 208 (×1.825) | 19.57 s (×2.003) |
  | 500% | 13 vs 64 at 20 ft/s (×0.203) | 17 vs 82 (×0.207) | 1.93 s (×0.198) |
  | 300% (probe only) | 26 vs 78 (×0.333) | 37 vs 114 (×0.325) | — |
- Nothing taken from opentennis: it has no vehicle brakes.

## What the original does

- The control law (C7's journal: the joystick step, vtable +0x19c, read from its disassembly). Held back,
  the throttle steps down by `ThrottleIncrement·dt` and the brake word is 1. Cross+Square on the ground
  jumps the throttle to `ReverseThrottle` and takes the brake off. The port's `biplane::controls` already
  has this.
- **PCSX2, 100%** (copy 3; `biplane.p2m2_SaveState` in slot 8; `watch/biplane.toml`; a pad script: Cross
  3.2 s, then Square 9 s, stretched ×4 because the copy runs at NominalScalar 0.25). Capture:
  `context/capture/p116/brake-100.csv`, git-ignored, 421 rows, 0 torn.
  - Free06's runway is level: body y 138.09 → 138.75 → 138.09.
  - The throttle rises 0.01166 a tick from tick 10991 and reaches 1.0 at 11076.
  - Square is first seen at 11087, at 48.4 ft/s. The plane keeps speeding up to 54.7 at 11113 while
    the throttle closes, and the throttle passes 0 at 11172 (33.3 ft/s).
  - It is down to 1.10 ft/s at 11234, **147 ticks after Square**. Then it rocks and creeps back (up to
    2.8 ft/s at 11248) on the −0.0116 throttle. The thrust column reads −15 to −31 lb there.
  - Thrust at 37.5 ft/s with throttle 0.99 reads 1799 lb, against 2334 for `MaxPower` at sea level: the
    engine's altitude fade (`MaxAltitude` 600 ft) leaves 77% at the runway's 138 ft.
- **The port at 100%**, on a floor at 138.09: Cross from rest reaches 45 ft/s in 96 ticks (the
  original: about 92 to 45.3). Square then stops it in 141 ticks (the original: 147 from 48.4), after a
  peak of 53.0 (the original: 54.7). On a floor at 0 the port is about 30% quicker, which is the
  altitude fade alone. So the test's floor is at the runway's height.
- **In the air the full flight does not slow at all.** With the stick centred, the autopilot holds the
  wing's trim angle of attack, so the airspeed is the trim speed (about 59 ft/s near the 600 ft
  ceiling) whatever the throttle; the throttle sets the climb. Held back from full gas, the plane sinks
  and phugoids around 44–53 ft/s and never reaches half speed, at any k: the swing is gravity's and
  the lift's, and neither is scaled. "Half speed in the air" is therefore measured on P95's level line
  (`level_slow`: the angle of attack that holds the weight, thrust less drag over the mass, the throttle
  closing at its own rate), and that goes as 1/k.

## How it was proved

- `crates/game-sim/tests/acceleration_modifiers.rs::biplane_acceleration_brakes_with_it`. At 200%, 50%
  and 500%, against 100% on the disc's own `biplane.phy`, the full flight is run on the runway: the
  run-up and the Square stop each within 10% of 1/k (12% at 500%, whose runs are a dozen ticks); the
  peak while held back within 10% (15% at 500%); the nose never below −0.2 while braking. The level
  line's time to half speed is within 5% of 1/k. Skips without a disc.
- `at_one_hundred_percent_the_new_knobs_touch_nothing` (P95's): at 100% the plane's file is unchanged
  to the bit (Debug of every field, now including `cg_braking`). And `Flight::forces` adds nothing while
  `cg_braking` is zero, so 100% runs the original's code path unchanged.
- `openmx/tests/remodify.rs::the_biplane_takes_a_change_in_flight`: a flight retuned to PlaneAccel
  200% / 50% is, to the bit, the flight built on it.
- The 100% baseline against the original: one PCSX2 capture (above), compared by hand on its shape and
  its tick counts. It is not a test.

## Not verified / not 1:1

- **Above 100%, the extra braking through the centre of gravity is not physical.** No real brake acts
  there. It is chosen so a harder-braking plane does not nose over against its unscaled weight (the
  contacts' ×2 put the nose to −0.998). A player at 200% sees the plane brake twice as hard with its
  tail only a little higher (nose −0.146 against −0.043).
- **In the full flight, holding back in the air does not slow the plane at any k** (above). The stick-
  centred autopilot flies the trim speed, and the throttle only changes the climb. The "half speed in
  the air" proof is on the level line, not the flight. Making the glide itself k-times quicker would
  mean scaling gravity or the lift, which changes everything else about the plane. Not done.
- **50% stops in ×1.83 the file's time, not ×2.** The tail stays down through the stop (nose +0.176,
  three wheels), so the wing sits at a draggier angle than the file's tail-up roll. That is gravity
  against halved thrust, and it is not scaled.
- **At high k the plane leaves the runway at lower speeds.** The lift is the file's, and the faster
  run-up reaches speed before the tail comes up. A 200% plane run to 45 ft/s lifts off, and so does a
  500% plane run to 25 ft/s (it overshoots while the throttle closes). The test's marks are therefore
  30 ft/s (20 at 500%). This was true of the run-up before P116 too.
- **The held-back creep is × k²** (the −1-step throttle times k-times power): a stopped 200% plane is
  pushed back four times as hard as the file's, still a crawl (≈130 lb against the file's ≈30 lb at
  3 ft/s). Not scaled back: it is the original's own quirk.
- **The PCSX2 baseline is one run, read by hand.** It is on Free06's terrain with the human's state's
  starting throttle (0.0117), while the port's run is on a flat floor at the same height from 0. Their
  agreement (stop 141 vs 147 ticks, peak 53.0 vs 54.7, run-up 96 vs ~92) is close, not bit-exact. No
  PCSX2 run was made at 200% or 50%: the original has no such knob.
- `Flight::put_back` does not carry P108's turn numbers (it keeps
  only P98's and now P116's). Seen, not mine: filed as P129.

## Next

Nothing.
