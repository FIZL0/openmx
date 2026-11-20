# 1 — first person camera (FINAL)

Task P384. 2026-11-20.

## What I did

- Found the disc's first-person camera in SLUS_207.67 and ported its file reader and its frame:
  `game_sim::camera::{FpPreset, FpView, FirstPerson, Onboard}` (end of `crates/game-sim/src/camera.rs`).
- Read `appstart\fppreset.ini`. It is in every track's `_2.res` and `_ss_2.res`. All 56 copies are the same file:
  - `[General]`: CameraUnrollFilter 0.01, ObjectRollFilter 0.01, YawRateFilter 0.1.
  - `[1Player]`: FOV 85, FOVWideScreen 90, OffsetFromBodyPoint 0,4.5,0, LookFilter 0.05,0.19,0.05,
    LookSwing 0.66, MaxRoll 45, ObjectRollScale 0.5, CameraUnrollScale 1.0, ShakeMagnitude/ShakePercentage 0
    (the file says "These aren't used for MX").
  - `[2hPlayer]` FOV 80, `[2vPlayer]` FOV 60, `[3Player]` FOV 80. Everything else as 1Player.
- Shell (`crates/openmx/src/chase.rs`, `main.rs`):
  - `FirstPersonCamera`: the camera button (Select) walks onto "First Person" after the bike's last
    cycling preset, and off it onto the first. The follow camera keeps running underneath, so going back
    is seamless. The toast and the log say `camera: First Person`.
  - `--camera "First Person"` starts on it.
  - Eye: the rider's posed **Head node** in the world. With no posed body: the bike's point plus
    `OffsetFromBodyPoint` in the bike's axes (where a still head sits).
  - Wreck camera and thrill camera still take over from it, as they do from the follow camera.
  - A put-back or a marker's Up snaps it (it resets when the follow camera snaps).
  - A bike with only one preset can still walk onto it (the camera button was gated on 2+ presets).
  - Split screens read their own section (full 1Player, stacked 2hPlayer, side by side 2vPlayer,
    quarters/eighths 3Player). A 16:9 display takes `FOVWideScreen`. Switching section gives the look
    lags that section's `LookFilter` (`FirstPerson::set_viewport`), as the original's viewport select does.
  - `--trace-camera` says `first-person` while it has the screen.
- MXA-style adjust, as ideas only (MXA reference is not on this machine): `--fp-adjust BOB,UP,FORWARD,FOV`,
  `chase::FpAdjust`. BOB is how much of the head's sway the eye keeps (1 = rides the head, 0 = pinned at
  the still-head point); UP/FORWARD are feet along the bike's axes; FOV is degrees added. The default is
  `1,0,0,0`, the disc camera untouched. Camera-page sliders for it are filed as P389.
- `crates/openmx/src/player.rs` (named in the task) was not touched. The camera's state sits with the
  other seat cameras in `main.rs`'s Rig. `player.rs` is the desktop start (disc browser), and has no camera.
- `../opentennis`: no first-person or onboard camera there (searched its Rust for first person /
  onboard / helmet). Nothing taken.

## What the original does

- Camera manager init (`./kit fn 0x20a360` → `0x1feea0`) loads `AppStart\CPresets.ini`,
  `AppStart\FPPreset.ini` and `CinemaCam.ini`. The fp camera is set up at `0x1ff550`: alloc 0x220,
  ctor `0x317bb0`, init `0x317d20`.
- The lag is the same 0x14-byte filter the follow camera uses (`Lag` in camera.rs): alpha = min(dt, tau)/tau.
- Base reader `0x12efe0` (FirstPersonCam.cpp, `0x12e740`): sections `%sPlayer` with `1`, `2h`, `2v`, `3`.
  - FOV (FOVWideScreen when the widescreen global is 3). The default is the object's own +0x88.
  - OffsetFromBodyPoint, LookFilter (default 0.1 each), ShakeMagnitude, ShakePercentage.
- MX reader (`./kit dis 0x3186e0`; Ghidra has no function there):
  - `[General]`: CameraUnrollFilter, ObjectRollFilter, YawRateFilter (ctor defaults 0.15 / 0.55 / 0.85).
    These are the taus of the three extra lags.
  - Per viewport: LookSwing, MaxRoll (degrees × 0.017453292), ObjectRollScale, CameraUnrollScale.
- Viewport select `0x3185c0` / base `0x12edf0` copies fov, offset and the look taus for the viewport.
- The frame, `./kit dis 0x317ed0` (dt, cam). It reads a record its rider fills: position, forward, up, a
  "no swing" flag at +8, yaw rate at +0x54, roll at +0xb0.
  1. yaw target = yaw rate × LookSwing × 0.11; roll target = ObjectRollScale × roll; both 0 when +8 is set.
  2. On a reset, the lags are set straight to their targets.
  3. Side = cross(up, forward) with y zeroed, normalized. Unrolled up = cross(forward, side).
  4. Step the roll lag. Camera up = unrolled up turned by −roll about forward.
  5. Step the yaw lag, wrap it (fmod 2π, then into ±π). Look = forward turned by yaw about the camera up.
  6. Lag the look per axis with LookFilter, or snap when +8 is set or on a reset. Normalize.
  7. Eye = record position, lifted to at least ground + 1.5 (`0x318560`) when there is a ground query.
  8. Up = normalize(cross(cross(look, up), look)). Writes eye, look, up and fov to the viewport (`0x112ac0`).
  - CameraUnrollFilter/Scale and MaxRoll are read but not used in this frame.
- Who fills the record while riding:
  - The class with ctor `0x318c30` finds node `"Head"` (`0x318f50` / `0x319010`).
  - `0x3191b0` writes the Head node's world position and axes.
  - `0x319050` copies them into the fp record, but only on a branch that reads a null pointer.
  - MX vs. ATV's port (`crates/game-sim/src/handling/update.rs`, `sync`) fills it from the bike's point plus
    a rotated seat/eye offset.
  - So the MX writer is not pinned down. The port puts the eye on the Head node, which is what the task asks.

## How it was proved

- `crates/openmx/tests/first_person.rs`, 9 tests, on the disc's `fppreset.ini` (free06's `_2.res`):
  - The reader gets the file's numbers, all four sections.
  - The eye is the head node, tick by tick as the head sways; the look is along the bike; FOV 85.
  - With no posed body, the eye is OffsetFromBodyPoint (4.5 ft) over the bike's point.
  - `--fp-adjust`'s bob 0 pins the eye; the trim and FOV add.
  - A yaw rate of 1 swings the look in, lagged, and it settles on 0.66 × 0.11 rad.
  - The up rolls by ObjectRollScale (half) of the bike's roll; none while tumbling.
  - The eye stays 1.5 ft over the ground.
  - The camera button walks A → B → First Person → A, and leaves it out with no file. A one-preset bike
    walks A → First Person → A.
  - A section switch gives the look lags the new LookFilter.
  - A side-by-side split reads 2vPlayer (FOV 60); a wide display reads FOVWideScreen (90).
- By eye: `openmx --track free06 --headless --frames 60 --shot fp.png --shot-frame 50 --size 960x540
  --camera "First Person"`. It shows a level onboard view from the rider's head over free06, the HUD on top.
  It looked right. Not committed (scratchpad only).
- The real head, measured with a temporary print in the shell (free06 start, posed body): on tick 0, the
  spawn pose, the head is 2.4 ft over the bike's point. From tick ~20, once the rider sits up, it is 5.5 ft
  over it. That is about the file's 4.5 ft still-head offset plus the seat. The head's height was not
  compared against the original.
- Checked: `first_person`, `camera_view`, `bike_camera`, `car_camera`.

## Not verified / not 1:1

- **No roll on screen.** The renderer's camera has no up vector (`game_render::world::Camera` builds its view
  with world up). The fp camera computes its rolled up, and the test checks it, but it is not drawn. Adding an
  up would touch 80+ camera literals across the crates. Filed as V137.
- **Who fills the record while riding is not pinned down** (see above). The eye is the Head node of the
  posed body, by the task's ask. With the riding ragdoll on, the head comes from the clip pose, not the
  ragdoll's.
- **Roll and "no swing" inputs are the port's choice.** Roll = the bike's roll angle off its axes. "No swing"
  = the bike upside down (up.y < 0). In MX vs. ATV's port these come from other rider words (+0x368 for roll,
  flip/spin for the flag). Not traced in MX.
- **Yaw rate** = the chassis' world angular velocity y. Same source as MX vs. ATV's (+0x340 y), not traced in MX.
- **FOV default** when a section has no `FOV` is 100, read as the init's first argument. The disc's file has
  `FOV` in every section, so it never matters.
- **Viewport index**: the original picks the section by viewport count. The port picks it by layout
  (stacked = 2h, side by side = 2v). Not checked against a split-screen run.
- **No comparison against the original's picture.** The disc's fp camera may not be selectable in MX
  Unleashed's own game; no PCSX2 run was made. Matched by eye only.
- **MXA adjust is ideas only.** MXA's files (`MxaOnboardCamera.cs`, `MxaOnboardAdjust.cs`) are not on this
  machine. Head bob here is the head node's own motion; BOB scales it. Only a command-line flag, no
  menu sliders yet (P389).
- **Look-back under the fp camera** still moves the follow camera underneath; the fp view stays. Not asked,
  not changed.
- **The tests feed a stand-in head, not a posed body.** The shell's head transform was only checked by the
  print above and by eye.
- **Split screen and the wide display were not looked at by eye.** Only full screen, one rider.
- The dust emitters' view test (S420a) reads the first seat's drawn camera. Under the fp camera it reads the
  fp eye. This is presentation-only state for the emitters' culling, and only when the player picks the fp
  camera. With it off, nothing changes.

## Next

- Spec review done: one-preset walk and section lags fixed; the head-height worry was the spawn pose (see
  above). Roll on screen is V137, adjust sliders P389.
