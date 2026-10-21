# 1 — biplane camera (FINAL)

Task V29. 2026-10-21.

## What I did

- Traced the plane's camera in the original (below), captured it behind the human's biplane flight
  (`watch/biplane-camera.toml`, `play_camera.sh`: C7's playback recipe on the new spec; 1718 ticks,
  `context/capture/v29/biplane-camera.csv`, not committed), and replayed the captured block through the
  port's camera.
- `crates/openmx/src/chase.rs`: `flown(&Flight)`, the plane's block as the original writes it; `ridden`
  uses it whenever the plane has a flight (a parked plane with no `biplane.phy` keeps the old origin).
- `crates/openmx/src/vehicle.rs`: `Plane::step` also reports a wreck's recovery as a reset (it was the
  chord only), and `main.rs` snaps the plane's camera on that report.
- `crates/openmx/tests/biplane_camera.rs`: the replay, and the block's mapping.
- `research/oracle.txt`: a V29 section (`plane_camera_target`, `camera_snap_player`, the state's objects).

## What the original does

- The plane's camera is built by the race camera's constructor on the camera manager, from the kind-1
  preset object (`AppStart\BiplaneCPresets.ini`, five presets, "Flight Cam 1" first): the same code as
  the bike's (S7/S24), so the camera's maths, its lags, look-ahead, horizon and near-ground lift are
  those already ported. The shell already loaded the file and cycled it on L1.
- The plane fills the camera's target block once a frame in its post listener after the measure
  (`./kit fn 0x00354e00`): point = the body's `+0x50` — the position at the start of the frame's last
  substep, the centre of mass — not the drawn origin; forward/up rows off the body's quaternion; the
  body's velocity; the measure's world rates as the lean word (the camera reads the yaw rate); flag =
  no wheel down; speed = airspeed floored at 0. In the state the point is (0, 3.0, 3.08) off the drawn
  origin at rest: the old shell's camera was that much too low and too close, its eye 8.7 ft from the
  centre of mass where the original's is 13.1 (7.2 up, 11 back).
- The plane's reset (`./kit fn 0x00355b00`, run by the chord, the wreck's recovery and the swap) calls
  `./kit fn 0x00200a70`, which sets the race camera's snap; then it rewrites the block. So the camera
  jumps to the spawn with the plane instead of flying hundreds of feet back to it.

## How it was proved

- A save-state capture: `watch/biplane-camera.toml` over `biplane.p2m2` played in PCSX2 (frames
  10971–12688, 1718 rows: the take-off, the climb, the pitch-up past vertical, the dive, the crash and
  its respawn, and a second reset). `the_plane_camera_films_the_flight_as_the_original` replays every
  row's block through `game_sim::camera::Race` on the disc's "Flight Cam 1": 1549 rows compared, the
  eye within 0.0037 ft (the worst is the first settle tick), the view direction within 0.6 mrad, both
  resets seen. The test fails without the reset snap (hundreds of feet) — the evidence for the snap.
- `the_plane_block_is_the_flights`: `chase::flown` puts each of the flight's fields where the writer
  does.
- Looked right beside a screenshot: the shell at rest on free06 (640×448) now frames the plane like
  the original's state (wings inside the frame with a margin, the horizon above the top wing); before,
  the wings ran off both edges.

## Not verified / not 1:1

- The capture tears (C7's polled recorder): 169 of 1718 rows are not compared — rows whose block was
  read before the plane wrote it, and the 120 ticks after each while the camera's lags still remember
  the wrong block, and the first 30 while the camera settles from the state. Rows whose viewport was read a tick late are compared against the port's previous
  eye.
- The shell's own flight was not compared with the original tick by tick: the test feeds the
  original's block, and `the_plane_block_is_the_flights` only proves the mapping from `Flight`.
  That `Flight::body.last_position` is the original's body `+0x50` rests on the flight port's own layout,
  not re-proved here.
- Near the ground: the capture's ground lift (camera `+0x18c`) is 0 on every row, so the replay uses
  no terrain, and the near-ground lift on the plane is only the bike's proven code, not seen on a plane.
- Banked turns and a full loop are not in `biplane.p2m2` (it climbs, pitches past vertical, dives and
  crashes); no side-by-side screenshots of the flight moments, only the at-rest one, and the shell's
  frame is 640×448 where the original's is 4:3, so the plane looks a little larger in the port's shot.
- The other presets (Flight Cam 2, Slot Cam, Lookback, 1P Stunt) and the h/v/w/x split-screen
  variants were not captured: only Flight Cam 1 is proved.
- The writer's other branch (the block's flag word against `+0x8e4`, writing `+0x9c0..`) was not read.
- The snap's other arm (a camera other than the race camera active: `+0x5c`'s `+0xb4`) is not ported:
  the shell's plane only ever has the race camera.
- Online sessions: the snap comes from the seat's frame report, which a rollback re-simulation does
  not re-raise; not tried.

## Next

Nothing for V29. The other presets and a banked/looping capture would close the list above.
