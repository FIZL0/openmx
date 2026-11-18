# 1 — drawn root lags (FINAL)

Task S474. 2026-11-18.

## What I did

- Brief: S466 saw the original's riding request root snap from ~30° of air lean to ~1.6° on the
  windmill's verdict tick. Does the shell's drawn root hand the riding body the same snap?
- Answer: no. The shell hands it one tick late. Each riding tick gets the root of the tick before.
- The snap is not the wreck's. The drawn bike plays the airborne clips at the crossup. The crossup is
  0 while the chassis' speed word (from the end of the last tick) is under 10. The teleport stopped
  the bike but left that word at 29.1, so tick 1 still measured a crossup (-0.357 rad off a 1e-5 ft/s
  sideways velocity). Tick 2 read 1.04 and measured 0.
- research/r14_windmill_bones.py: new `cross` flag. Each row also logs the rider's crossup (+0x600),
  shown pose and fore-aft (+0x9e8/+0x9ec) and the chassis' speed word (+0x34).
- Recaptured S466's run with `req cross` into context/capture/s474/south3-cross.csv (not committed).
  Its 61 rows equal b15/south3-bones.csv on every shared cell (the test checks).
- tests/replay.rs:
  - `s474_ride_drawn`: r24's ride with the chassis drawn beside it as the shell draws it
    (want_under, riding_under, Chassis::ride, grips and "Bike" node handed to the next tick). The
    motion driver is seeded on the image's airborne idle and pose, and the chassis is drawn off the
    image before tick 1, so tick 1 does not restart it.
  - `the_shells_drawn_root_lags_the_originals_a_tick` pins the lag and what it costs the bones.
- Filed S474a: make the riding step use this tick's root.
- No product code changed. Nothing taken from opentennis (it has no riding body).

## What the original does

- The request's root on each row is the airborne pair posed at that row's own crossup:
  31.9° at the image's -0.387 rad, 29.7° at tick 1's -0.357, 1.6° at tick 2's 0.
- The crossup and shown pose on both ticks are bit for bit the port's.
- Capture recipe: put context/capture/b15/south3-2487.p2s into slot 9; copy started with
  `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 WAYLAND_DISPLAY=wayland-1`, NominalScalar
  0.25; under `./kit pcsx2 with --`:
  `r14_windmill_bones.py PID 9 2439 140 4067.5 8 context/capture/s474/south3-cross.csv bones ride=0 req cross`.
  The first try paused late (frame 2561); the second landed on 2487.

## How it was proved

- A save-state capture compared frame by frame: context/capture/s474/south3-cross.csv, 61 rows.
- The shell's root handed to tick t equals the original's root on row t (the tick before's) to
  0.000°. Against the original's own row (t + 1) it is 2.36° off on tick 1 and 28.36° off on the
  verdict tick (29.67° against 1.56°).
- With the shell's roots and the original's hands and feet, the bones' mean velocity is
  (+0.09, -0.02, +0.30) ft/s off on tick 1, (-1.21, -1.91, -0.49) on the verdict tick and
  (-0.32, +0.42, +1.48) on the entry. With the original's roots S466 measured under 0.6.
- Checked: `./kit check -- -p game-sim --test replay the_shells_drawn_root`: 1 passed.

## Not verified / not 1:1

- The port still hands the riding body last tick's root. Not fixed here: the root comes off the
  render side's clips after the sim tick, so a fix changes what the shell hands the sim. Filed S474a.
- How the original poses the "Bike" node before the request fill on the same tick is not read in
  the decompilation. Only the observation is proven (the root tracks the same row's crossup).
- The drawn grips (hands and feet) were not compared with the original's. The cost test uses the
  original's, to isolate the root.
- The motion driver's state at the image (action 25, ramp 0) is assumed in the test, not read:
  its pose and fore-aft are read from the image, and the crossup and pose then match bit for bit.
- The chassis is drawn at a fixed 1/30 s step, not the capture's per-row dt.
- Only one run (south3, a teleported state) was checked. A natural mid-air wreck has no stale speed
  word, so its crossup does not drop the tick before. There the lag costs the riding tick's change
  of crossup, not 28°.

## Next

- S474a.
