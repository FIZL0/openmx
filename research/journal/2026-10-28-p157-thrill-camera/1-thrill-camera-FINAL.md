# 1 — thrill camera (FINAL)

Task P157. 2026-10-28.

## What I did

- `crates/game-sim/src/camera.rs`: `Thrill`, the original's thrill camera (MXCameraThrill). It keeps the
  takeoff on the first airborne frame (`watch`), picks one spot when it comes up (`place`: the ground
  lead or the takeoff lead), and each frame gives a `View` with the eye on that spot, the look-at 3 ft
  over the rider's target point, and the zoom from the flat distance (`frame`, 75° near → 18° at 200 ft).
- `crates/openmx/src/chase.rs`: `OtherCamera`, the manager's press and release for the other-camera
  command (0x22), plus `LookBack::hold` for "let go with look-back still held → look-back".
- `crates/openmx/src/main.rs`: a `thrill` field on the rig, run after the rider each tick on the bike
  (not in tricks), its view drawn over the follow and wreck views, a `camera: thrill` line, and the
  `--trace-camera` label. The look-back takes no press while the thrill camera has the screen.
- `crates/openmx/tests/thrill_camera.rs`: five tests (see below).
- `research/oracle.txt`: a P157 block, and P78's "not ported" note on camera_thrill_off updated.
- Nothing came from opentennis. The camera code there has no counterpart to this one.

## What the original does

- The press handler (`./kit fn 0x00200000`) sends 0x22 to camera_thrill (`./kit fn 0x00200740`). That
  works only while the race camera is active, and only when the press lands: the target's velocity has
  to be longer than 3. Holding the button never re-tests the speed, so a press made too slow does
  nothing until it is pressed again.
- On release (`./kit fn 0x002008b0`), if the thrill camera is up and no wreck has latched +0x14c,
  camera_restore (`./kit fn 0x001ff140`) brings back the race camera on the player's preset. If the
  look-back is still held, the race camera goes to the look-back preset instead.
- The wreck switch (`./kit fn 0x001ffcb0`) does not take the screen from the thrill camera.
- The camera picks its spot once, when it comes up (`./kit fn 0x003212a0`), and never moves. Its three
  lags are set to the spot itself. The spot logic is in oracle.txt's P157 block.
  - On the ground: the spot is a lead ahead along the velocity (stretched to 30 ft/s), shortened where
    the ground ahead rises, a fifth of the velocity back, and 0.45 of it to the side. A sweep from the
    current eye stops it at a wall, and it sits halfway down to the ground but at least clearance + 1
    over it.
  - In the air: the spot comes off the takeoff point and velocity, at takeoff height less a foot.
- The fov follows `./kit fn 0x00320650`, and the look-at follows `./kit fn 0x003206e0` (3 ft over the
  target, read from RAM: target +0x24 in recording2's state).

## How it was proved

- `crates/openmx/tests/thrill_camera.rs`, via `./kit check -- -p openmx --test thrill_camera`. All
  tests pass.
  - `the_other_camera_held_above_3_is_the_thrill_camera_under_either_layout` covers R2 under Original
    and Cross under New.
  - `at_3_or_slower_the_press_does_nothing_until_pressed_again`
  - `a_wreck_cameras_screen_takes_no_press`
  - `let_go_with_the_look_back_held_is_the_look_back`
  - `the_spot_is_the_originals` checks spots worked by hand from the decompiled formula. These are
    ground, a stretched slow ride, a wall in the sweep, the far fov clamp with its expired flag, and the
    takeoff spot. It also checks the 3 vs 3.01 speed edge.
- By eye. In the original (KIT_PCSX2=3, recording2's free09 state), I held gas, then R2. The shots are in
  context/shots/p157-0..5 (not committed). The camera jumps to a fixed low trackside eye, the rider
  rides away from it and is lost over a rise while R2 stays held, and on release the follow camera is
  back. The port on free09 (`--play` a hand-written tape, shots context/shots/p157-port-*.png) shows
  the same kind of shot: a fixed eye low beside the line, the rider zooming in as they come. On release
  (tick 200 in `--trace-camera`) it prints `camera: Race Cam 1` and the follow view is back.
- A first port run pressed R2 mid-drop and got the eye behind the lip, looking into the ground. The
  airborne branch has no floor on its lead, and the original's code does the same (oracle note). I did
  not reproduce that case in the original.

## Not verified / not 1:1

- No frame-for-frame comparison. The by-eye shots in the two games are from different rides and moments
  (the state's line and the tape's line are not the same), so they only show the kind of shot.
- Float order: the original computes the spot with VU0 macro ops (vmulbc/vadd). The port uses
  ps2_float ops in the C order. They are not proved bit-exact. This is presentation only.
- The sweep is the port's `Obstacles::nearest` segment from the follow camera's eye to the spot. The
  original sweeps its collision object (+0xf4), whose shape and size I did not read, so a spot by a
  wall may sit a little differently.
- The three lags: the port keeps the eye on the spot. The original sets its lags to the spot every pick,
  so they should not move, but I did not trace their step to prove it.
- Releasing after a wreck: in the original, a wreck latches +0x14c and the release then leaves the
  thrill camera up. The port always hands back on release. Not ported because the port's wreck camera
  has no manager latch to hang it on.
- The wreck camera still switches on under the thrill camera in the port. The thrill view is drawn over
  it, so the screen matches, but the wreck camera's own state runs underneath.
- The follow camera keeps stepping while the thrill camera is up. In the original, the race camera is
  not the active camera then, and I don't know whether it lags or snaps when it comes back.
- Bike only. Whether the biplane's manager builds a thrill camera was not checked, so the plane gets
  no other camera.
- The +0xc4 "expired" flag is kept (`Thrill::expired`), but no reader of it was found.
- Release is detected as "not held". A release lost during a pause still drops the camera, where the
  original waits for the release event.
- The look height of 3.0 was read from RAM in one state only (recording2).
- The track's ceiling uses `Race::ceiling_for_fog`'s rule (200 when fogged past 1500 ft). The original
  reads track +0x3b8 +0x1c. Same rule, not re-read for this task.

## Next

None for this task. To prove the spot exactly, capture the original's thrill eye (the camera's +0x110 spot) with PINE and replay the same frame through `Thrill::place`.
