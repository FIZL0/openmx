# 1 — mod follow in the shell (FINAL)

Task MD82d. 2026-11-14.

## What I did

Before this, the shell already gave each mod machine its own `FC_` presets in most cases:

- the quad (MD136) on `Cam\FC_ATVDefault.pb`;
- the fliers off `mxvatv_fliers` (MD108) on `FC_BiplaneDefault.pb` and `FC_HeliDefault.pb`.

But all of them still stepped as MX Unleashed's race camera (`Race::update`). MX vs. ATV's five cars
(MD39c) rode the bike's camera on the track's `cpresets.ini`. This task changes that.

- `chase.rs`:
  - `mod_follow(cam: &mut Race, …)` steps one of the shell's follow cameras as SLUS-21104's
    `MXCameraFollow` (`game_sim::camera::ModFollow::update`, MD82b). MD82c proved the biplane and the
    Kiowa use the same class.
    - The shell keeps every follow camera as a `Race`, because the look-back, the right stick, the
      framing and the preset starts all reach into it there. So the helper swaps the camera into a
      `ModFollow` for the frame and back out afterwards.
    - For that frame it uses `ModFollow`'s ceiling (none) and speed reference. The shell's own (the
      fog's ceiling) are put back after the frame.
  - `mod_follows(up, own)`: the quad, the MX vs. ATV car, the biplane and the Kiowa step on it, each
    only while its camera holds presets from its own `FC_` file. A mod machine that fell back to the
    bike's or the disc flier's presets keeps MX Unleashed's step.
  - `mxvatv_car_machine` maps an MX vs. ATV car's kind to its `ModMachine`, so `mod_presets` gives
    `FC_BuggyDefault.pb`, `FC_MonsterDefault.pb`, `FC_BajaDefault.pb`, `FC_GolfCartDefault.pb` or
    `FC_SandrailDefault.pb`.
- `main.rs`:
  - New `mxvatv_car_camera` reads the car's own `FC_` file from its pack. The car gets a camera in
    `car_cameras` (`Kind::MxvatvCar`), built like the quad's: the presets are read by `flier_layout_presets`
    for each display and layout, and it starts on the file's first preset, not on "Race Cam 1". It says
    which file it read, or that the pack lacks it, in which case the bike's camera follows the car as before.
  - The seat's camera frame picks `chase::mod_follow` or `Race::update` through `mod_follows`.
- MD165 (low) is filed for the `RC_` files; see below.

Nothing was taken from opentennis: it has no per-vehicle camera.

## What the original does

SLUS-21104 follows every machine with one camera class, `MXCameraFollow`, on the `FC_` file its
machine's kind picks. MD82a found this, MD82b ported the class, and MD82c checked the fliers against
it. In a race it swaps `FC_` for `RC_`. See `research/oracle.txt`, `# MD82b`.

The camera frame in the shell's free ride is therefore the `FC_` one.

## How it was proved

New file `crates/openmx/tests/mod_follow_camera.rs`:

- `the_shells_mod_camera_steps_as_modfollow_bit_for_bit`:
  - It takes every preset of the `mxvatv_atv` pack's `FC_ATVDefault.pb`, builds a camera on it the way
    the shell builds a car's (fog ceiling 200, speed reference, reverse 5, snap) and steps it through
    `chase::mod_follow` for 2000 generated frames. The frames cover standing, driving, airborne fast
    and slow, straight up, backing up, and the short-viewport trim, over rolling ground.
  - A `ModFollow::new` on the same presets steps beside it. The eye, look-at, fov, all six lags with
    their taus, distance, yaw, pitch, lift, lock and snap must be equal to the bit on every frame.
  - The shell's ceiling and speed reference must survive every frame.
  - MX Unleashed's `Race::update` on the same camera must differ on more than 50 frames per preset,
    or the test proves nothing.
    - With the camera behind the machine (yaw a half turn, the preset's own angle), the first run found
      the two games' cameras step alike on every frame. MD82c saw the same: the rear fades sit at their
      ends. So from frame 1000 the test turns the camera to one radian off straight ahead, through the
      player's yaw nudge (the right stick, P114), where the fades bite.
    - The look-back preset is exempt from this check only. Both games drop its look-ahead
      (camera.rs:1704/2008) and the file gives it no lags, so it steps alike in both.
  - Result: `./kit check -- -p openmx --test mod_follow_camera`, 13 tests passed. All 8 presets of
    FC_ATVDefault.pb, 2000 frames each, are equal to the bit.
- `only_mod_machines_on_their_own_file_follow_on_slus_21104s_camera`: the truth table of `mod_follows`.
- `each_mxvatv_car_reads_its_kinds_file`: the five cars' file names.

`ModFollow` itself is proved against SLUS-21104 by MD82b's and MD82c's capture replays
(`tests/mxvatv_camera.rs`). This task proves that the shell's path is that camera.

## Not verified / not 1:1

- **No capture replay this session.** The MD82b and MD82c captures (`context/capture/md82b`, `md82c`)
  and `context/mxvatv-ps2/md82/cam` are gone from the shared context, so `tests/mxvatv_camera.rs`
  skips. The shell path is proved equal to `ModFollow`, not re-proved against SLUS-21104 here.
- **The shell's target blocks for mod machines are not SLUS-21104's.**
  - `chase::quad` and `chase::mxvatv_car` say themselves which words MX vs. ATV writes there was not read.
  - The biplane and the Kiowa off `mxvatv_fliers` fill MX Unleashed's flier blocks (`chase::flown`,
    `chase::heli`).
  - So the camera is SLUS-21104's, but what it is handed is the port's best guess.
- **The MX vs. ATV car's camera was never run in the shell here.** No `mxvatv_cars` pack is installed
  in this checkout, and this checkout's openmx-mods has no importer for one. So it is unknown whether
  the pack ships the `Cam\FC_<kind>Default.pb` files at all. Without them, the car keeps the bike's
  camera and the shell prints a line saying so.
- **No shell ride was run or watched on screen.** The wiring was checked only by the build and the
  tests above.
- **Shell additions stay on top of SLUS-21104's camera:** the framing (P82), the field-of-view ease
  (P119), the look-back and stick nudges (P78, P114), and the plane's keep-in-view (P275) still run
  on top of it. The fog ceiling is turned off only for the frame of the step.
- **The `RC_` race-camera files are still not read.** SLUS-21104 uses them only when its race camera is
  current, and that camera class was neither found nor captured. Filed as MD165 (low).
- **A flier pack file with no presets at all:** `flier_cameras` is still set, so the disc presets would
  step as SLUS-21104's camera. No real file is like that; the reader falls back to 4x3 and to the full
  screen first.
- **Untamed's machines: nothing to wire.** MD81's inventory (openmx-mods `notes`, f9ca315) found that
  Untamed adds no machine, only retuned `.phy`. None of its machines rides in the shell as a mod yet;
  MD160 only captured the monster truck.

## Next

Nothing for this task.
