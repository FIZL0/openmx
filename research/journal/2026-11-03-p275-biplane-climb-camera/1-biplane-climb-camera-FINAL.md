# 1 — biplane climb camera (FINAL)

Task P275. 2026-11-03.

## What the original does

- The plane's camera is `game_sim::camera::Race`, the original's race camera, on `biplanecpresets.ini`
  (V29). It builds its heading and its eye-to-look-at basis on the plane's velocity flattened to the
  ground, and both trail behind lags. In a steep climb the look-at stays near the horizon and the
  plane rises out of the top of the picture. In a loop it goes over the eye and behind it.
- V29's capture (the original's own eye, direction and fov per frame, with the plane's centre, in
  context/capture/v29/) answers what the original does. Projecting the plane through each row: at the
  top of the climb (rows ~515-573) the plane is up to 46 degrees above the middle of the view. The
  vertical half-fov at 4:3 and 85 degrees across is ~34.5 degrees. So the original loses the plane too,
  on 36 of 1718 rows. No PCSX2 capture of a full loop by the original was made (see below).
- The brief says: if the original loses it too, keep the plane framed anyway and write down what
  differs. That is what was done.

## What changed

- `chase::keep_in_view(view, point, fov, aspect)` (crates/openmx/src/chase.rs). If the plane's centre
  is inside `IN_VIEW` (0.7) of the half picture across and up, and the look is no steeper than 80
  degrees, it returns without touching anything. Otherwise it turns the look from the camera's yaw and
  pitch towards the plane's. It bisects for the least turn that has the plane inside the box, or turns
  all the way if none does. The pitch is capped at `STEEPEST_LOOK` (80 degrees), because the renderer's
  `look_at` builds on the world's up and has no sideways axis looking straight up. The eye, the look's
  distance and the camera's own state (lags, angles, `Race`) are untouched, so the next frame's camera
  is the original's again, and V29's bit-exact replay still holds.
- An earlier version turned about one axis at a time with the cap applied last. It could not swing over
  the top when the plane was just past overhead (the stacked split read 1.002). It was replaced by the
  yaw/pitch path with bisection.
- main.rs, the per-seat camera frame: called after the fov ease, only while flying the plane on its own
  camera (`flying`), with the seat's own viewport shape and vertical fov (`aspect::fov`). So it covers
  every preset and every split (the h/v/q/w/x/z viewports). Wreck, thrill and other vehicles' cameras
  are not touched.

## How it was proved

- `crates/openmx/tests/biplane_climb_camera.rs`:
  - `the_plane_stays_in_the_picture_through_a_loop`. A scripted `Flight` (throttle 30 ticks, then full
    throttle and the elevator full back for 330) goes through a whole loop. The test asserts the nose
    points up past 0.95 and the plane is on its back past -0.9. The original's Flight Cam 1 at 4:3 with no
    turn loses the plane (it goes behind the eye at tick 116). With the turn, every preset (Flight Cam 1,
    Flight Cam 2, Slot Cam, Lookback Cam, 1P Stunt) on every shape ("" 640x480, h 640x240 stacked, v,
    q, w, x, z) keeps the plane within 0.7 (+1e-3) of the half picture on every tick. In level flight
    (ticks 20-30), only 1P Stunt h, w and z are turned. Its own level framing has the plane at 0.989 of
    the half height on the short stacked split, and at 0.729 on the wide shapes. That is asserted exactly.
  - `the_originals_climb_loses_the_plane_and_the_shell_keeps_it` replays V29's captured eye and
    direction. The original lost the plane on 36 rows. With the turn the worst is 0.700, and 1667 of
    1718 rows are left untouched to the bit (every row already inside 0.99 of the box is unchanged).
  - Both skip without the disc or the capture.
- Existing tests still pass: biplane_camera, camera_view, thrill_camera, wreck_camera and
  right_stick_camera (release).
- Shell shots (`--vehicle biplane --fly-at 2304,2281,180,350 --play TAPE --headless --shot`, 640x480,
  before with the turn switched off on a scratch build, after with it): Flight Cam 1 at ticks
  100-160 of a climb into a stall, presets 1-4 by Select presses, and split h at 112 and 125.
  A 450-tick tape (level 150, stick back after) climbed to 550 ft and stalled at 7 mph. The trace
  (`--trace-camera`) shows the turn acting at ticks 245-248, 284-301, 336-347 and 367-368, and nowhere else.
  At 292 (falling tail-first out of the stall, the eye at the nose), before shows only the cowl from
  below and after shows the plane with its wings. At 344 the two differ by a few pixels. The shots
  were looked at by eye and kept in the session's scratch folder, not committed.

## Next

none. Not filed: a switch to turn the framing off. Cameras are presentation (AGENT.md: match by
eye, and no follow-ups to chase render differences), the brief asks for the plane kept framed, and
the simulation is untouched, so replays stay bit-exact with or without it.

## Not verified / not 1:1

- Not 1:1 with the original, by the brief's choice: the original loses the plane off the top in a
  steep climb and the shell keeps it, always (there is no setting to get the original's framing back).
- The original's camera through a full loop was not captured on PCSX2. Only V29's climb is. The
  other presets and the split screens of the original were not captured at all.
- The shell's scripted pilot could not fly a whole loop from the free06 spawn (it stalls at ~550 ft).
  The full loop is proved in the test's scripted `Flight`, not in shell shots.
- 1P Stunt on the short stacked split and the wide shapes is turned in level flight too, because its
  own framing puts the plane near the edge there. Small, but a change from the original's picture.
- The turn is done on the frame's view, after the camera, so render interpolation between ticks, if any,
  sees the turned look.
- The heli and other vehicles' cameras were not looked at.
