# 1 — windmill replay (FINAL)

Task S211. 2026-11-02.

## What I did

- `tests/replay.rs`:
  - `in_the_track_with_toys` is `in_the_track_with_cars` plus free09's toys. It loads
    `game_render::animated_toy::Animated` from the track's archives, puts its `sim` in
    `traffic.toys`, and attaches the toys after the path cars.
  - `the_ragdoll_kit` is copied from `tests/ragdoll_cars.rs`'s `ride_kit`.
  - `s211_ride` builds the ride:
    - It seeds a Rider from S167's `south3-before.ram`: chassis, engine state, levers and the
      bike search, with the wreck files, the ragdoll and the body rays carried.
    - It moves every place the ride keeps by the script's teleport delta.
    - It leaves the windmill toy undrawn with `Bodies(0)`, as the original's was before contact.
    - For the control, it stands the windmill's prop 100,000 ft below the world.
  - `s211_capture` reads the csv.
- New test `the_rustic_windmill_throws_a_bike_as_the_original_does`.
- Filed R14 for the early cannonball.

## What the original does

- **Row alignment.** csv row 0 is the image's frame, 2734, and port tick `t` is csv row `t + 1`.
  The script's teleport writes land on the frame after the dump, so the first row repeats the start.
- **South3 (bike at rest beside the mill).**
  - Ticks 1–2: the bike settles. Vy is −0.994, then −2.088, and the port matches.
  - Tick 3: first contact with the windmill, whose body has never stepped (S167's identity "before",
    mover = place × 30). The throw is (7.79299, 18.6087, 133.41) and the bike wrecks.
  - Ticks 4–13: the wreck body flies a short arc. The port's body origin is within 1e-3 relative of
    the capture's vehicle position through tick 7, the last tick before the port's own put-back.
  - Tick 14: wreck 13, the off-map cannonball, throws (256.819, 100.312, −155.939). This is
    off_map::throw (S59): REACH/(2√ARC_G) level toward the map centre, which is 300.45 ft/s, plus a
    lift of 100.312. It fires because the rider's main bone (or the chassis cofg) is within the
    150 ft margin of the map's edge. The bone is the ragdoll's, so it is not the bike that left the
    map.
- **Apex (the same image moved by (0.25, 36, −7.6)).**
  - The bike falls onto the windmill's top and the velocities match through tick 24.
  - Tick 24: the wreck, with vel (11.2484, −23.9695, 0.0617). The port's reason is 2.
  - The windmill's mover then throws the wreck body. From tick 29 it covers ~1,967 ft a tick: the
    capture's "~59,000 ft/s vehicle flight", 59,688 ft/s at most. That is the windmill's bogus
    first mover acting on the wreck body (S167). It is not the cannonball.
  - Tick 85: the cannonball, (258.456, 100.312, −153.21).
- The capture's x, y, z column is the drawn bike. It sits ~1.5–2 ft off the chassis origin and the
  wreck node on apex ticks 19–28, so positions are compared to 2 ft and the flight by its per-tick
  step.

## How it was proved

Save-state capture replay. S167's `south3.csv` and `apex.csv` (s1 `context/capture/s167/`) against
the port from `south3-before.ram`, in
`tests/replay.rs the_rustic_windmill_throws_a_bike_as_the_original_does`:

- south: vel ticks 1–3 within 1e-4 relative (the capture prints six figures); the wreck on tick 3
  and not before; the body's position ticks 4–7 within 1e-3 relative.
- control (windmill gone): no wreck and |vz| < 1 for 20 ticks.
- apex: vel ticks 1–24 within 1e-3 relative; the wreck on 24 and not before; position within 2 ft on
  ticks 25–41; the per-tick step within 1e-3 on ticks 29–41.
- both: the cannonball's vector within 1e-4. Its tick is pinned at the port's current 8 and 42
  against the original's 14 and 85.

`./kit check -- -p game-sim --test replay`: 110 passed. Run alone with `--nocapture`, the test
printed its measurements, so it did not skip here.

## Not verified / not 1:1

- **The cannonball's tick.** The port's ragdoll main bone reaches the margin early: south tick 8
  against 14, apex 42 against 85. The port's bone flies out from tick 4. The original's bone path was
  not captured, so where they part is unknown. Filed as R14.
- **Apex has no image of its own.** It is south3's image moved by the script's apex delta. The
  original's apex run teleported from the same image, so the start matches only as far as the
  script's region-wide move does. Apex vel agrees through tick 24, which suggests it does.
- **Seeding.** Only the windmill's toy state (undrawn, `Bodies(0)`) is set. The other toys' clocks
  and bodies, and the windmill's spin clock, are the port's from attach, not read from the image.
- **The wreck's reason.** The capture does not log it. The port's reasons are 4 (south) and 2 (apex).
- **Positions.** The capture's position is the drawn bike, so positions are bounded at 2 ft, not to
  the bit. Tick 3's velocity is checked only to the capture's six printed figures.

## Next

Nothing for S211. R14 traces the early main bone.
