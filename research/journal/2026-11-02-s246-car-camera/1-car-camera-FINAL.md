# 1 — the cars' cameras (FINAL)

Task S246. 2026-11-02.

## What the original does

- **Which presets.** `camera_presets_by_kind` (0x0020a1c0) builds one camera-preset object per
  machine kind:

  | Kind | Machine | File |
  |------|---------|------|
  | 1 | biplane | `BiplaneCPresets.ini` |
  | 2 | helicopter | `HeloCPresets.ini` |
  | 3 | buggy | `BuggyCPresets.ini` |
  | 4 | monster truck | `MonsterTruckCPresets.ini` |
  | 5 | baja | `FourWheelCPresets.ini` |

  A car's camera is the same race camera class the bike and the plane use, on its kind's file. In
  each of S149a's switched states the car's camera is on preset 0, "Race Cam 1", and stays there
  for the whole drive:

  | Car | Race Cam 1 distance | Camera | Viewport (camera +0x7c) |
  |-----|---------------------|--------|-------------------------|
  | baja | 15 | 0x14311a0 | 0x150a6d0 |
  | buggy | 16 | 0x1411450 | 0x14dd310 |
  | truck | 25 | 0x14514e0 | 0x152bc70 |

  The camera's speed reference (117.3328) and height bias (3.0) are the plane's. A second race
  camera in each state points at the car's block too. That is the bike's, on `cpresets.ini`
  (distance 13), and it is never updated (its snap stays 1).

- **The target block** sits at car +0x8a0. The car's post listener fills it once a frame, after the
  measure.

  | Block field | Filled from |
  |-------------|-------------|
  | +0x10 point | see below |
  | +0x30 forward | the measure's +0x590 |
  | +0x40 up | the measure's +0x5a0 |
  | +0x50 velocity | the body's +0x70 |
  | +0x60 | the measure's local velocity +0x580 |
  | +0x80 | the measure's world rates +0x570 (the camera reads the yaw word +0x84 as its lean) |
  | +0xb0 flag | +0x530 == 0, i.e. no tire down |
  | +0xb4 speed | max(+0x55c, 0) |

  The point differs by class:
  - **Baja and buggy** (vtable 0x0045cf80; post 0x003a02f0, block writer 0x0039e9c0): the point is
    the body's +0x50 (the centre of mass where the last substep began), *plus* the centre of mass
    (+0x180) turned by the body's quaternion (+0x170).
  - **Monster truck** (vtable 0x0045ed50): post 0x003e1710 first stores the car's origin at
    car+0xfe0, which is +0x50 *minus* that same turned centre of mass. Its writer 0x003e0410 copies
    the origin into the block.

  So the truck's camera aims 3.5 ft under its centre of mass, the baja's 0.8 ft over it, and the
  buggy's by its own (0, 0.75, −1)-ish offset turned with the car.

## What changed

- **`crates/openmx/src/chase.rs`.**
  - `driven` builds the block as above from a `game_sim::car::Machine` and its `Kind`.
  - New `car_presets(kind)` names each car's presets file.
- **`crates/game-sim/src/car.rs`.** New `turned_cofg(body)`, the centre of mass turned into the
  world.
- **`crates/openmx/src/main.rs`.**
  - `Rig.car_cameras` holds one `Race` per car kind the garage can drive, built on its own file
    (from the track's archives or `free06_2.res`, as the helicopter's is), starting on "Race Cam 1".
  - The load prints `<car>: its camera starts on Race Cam 1, 6 presets in <file>`.
  - `Rig::follow`/`follow_mut` and the frame's camera pick hand a driven car its own camera, and
    `took` snaps it at the switch.
  - The car's camera shares the bike's slots: field-of-view ease, look-back, and framing (the Camera
    page's one set).
  - Display and layout changes reach it through `set_car_display` / `frame_cars`, next to the
    helicopter's.
  - The camera button walks the car's own presets. The car's preset is not saved as the seat's bike
    start.
  - Online rigs have no car cameras, so their cars keep the bike's camera as before.
- **`watch/car-camera.toml`, `capture.sh`.** The capture spec and the script that runs it.

## How it was checked

`crates/openmx/tests/car_camera.rs`, against `context/capture/s246/*.csv` (650 ticks per car):

- **`each_cars_block_is_the_originals`.** `chase::driven`, given the captured body and measure,
  writes the original's block word for word: point, forward, up, velocity, yaw word, flag and
  speed. Every row matches for the baja (650) and the buggy (650). The truck matches 610 rows; its
  other 40 are polls that straddled a tick (block and body from different ticks), and those are
  skipped.
- **`each_cars_camera_films_the_drive_as_the_original`.** The captured block goes through
  `game_sim::camera::Race` on the car's own file and "Race Cam 1". The surface hands the probe the
  original's captured ground lift, and free07's ground answers the eye's floor. Over 620 compared
  rows per car:

  | Car | Eye | View |
  |-----|-----|------|
  | baja | exact | within 4e-8 rad |
  | buggy | within 0.0017 ft | within 6e-6 rad |
  | truck | within 0.0006 ft | within 2.2e-5 rad |

- **`over_free07s_ground_only_the_lift_parts_the_view`.** The same replay over the port's own
  free07 `Ground`. The eyes are unchanged. The views part by the look-at lift that the two grounds
  set differently: 0.07 ft for the buggy and 0.14 ft for the truck, the same gap `bike_camera.rs`
  allows. That turns the view by at most 0.002 rad.
- **Control.** Giving each car another car's file fails: on the buggy's file, the baja's eye is
  3.6 ft off.
- **`a_driven_car_is_followed_by_its_own_camera`.** Headless shell runs of `car_switch.rs`'s tape on
  free09/free07/free11 with `--camera-view original`. The load line names the car's file, and from
  the switch in to the switch out the traced follow camera stands at 25/15/16 ft, against the
  bike's 13 before and after.
- I looked at 640×448 shots of the driven truck (free09) and baja (free07) at tick 150. Each is
  framed from behind on "Race Cam 1".
- The neighbouring tests pass: `bike_camera`, `biplane_camera`, `camera_view`, `car_picture`,
  `car_switch`, `kiowa_switch`, `machines_everywhere` and `right_stick_camera`.

- `./kit check -- -p openmx --no-fail-fast`: 2534 passed and 1 failed. The failure is `biplane_particles
  a_plane_flown_into_the_ground_puffs_where_it_hits` (F9), which S245 recorded as red on main
  before this task. `./kit audit` reports only the two `plan/DONE.md` leaks that were already
  there.

## Left open

- **P264:** the Camera page and the seat's own presets (P82/P163) don't cover cars. The cars use the
  Camera page's one framing and keep no start preset of their own. Their Lookback Cam and other
  presets have not been checked against a capture.
- **S268:** online rigs have no car cameras.
- A car's camera in split screen has not been checked against a capture.
