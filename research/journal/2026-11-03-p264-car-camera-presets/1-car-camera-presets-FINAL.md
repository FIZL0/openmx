# 1 — the cars' camera presets and their look-back (FINAL)

Task P264. 2026-11-03.

## What the original does

On each of the three cars, L2 (the original layout's look-back) turns the car's own camera to
preset 4 of its file, "Lookback Cam". The baja's and the buggy's then stand at 23 ft and the
truck's at 22 ft. Letting go turns it back to "Race Cam 1".

The turn happens on the tick whose pad buffer first holds the press, in both directions, on all
eight changes in each capture. That is the port's order too: the pad is polled, then the camera
runs.

The capture's pad column is not always that tick's pad. The recorder can read the buffer after the
pad library has written the next frame's and before the game runs that frame. In that case the
column leads by one row, and it does so through the whole capture:

The lead is 1 row on the baja and truck captures and 0 on the buggy's. On the baja, L2 from tape
tick 60 is in row 59's pad, and the camera turns on row 60. The test takes the lead from each
capture's first press and holds the other seven changes to it.

## What changed

- **`crates/openmx/src/menu_cameras.rs`.**
  - `Seat.car_start` holds the start preset of each car, by name.
  - `Seat::car_start(car)` reads it.
  - `camera_left_on` writes a car's start.
  - `settings.txt` gets one `camera_car_start = <seat> | <car> | <name>` line per car with a
    start. A file without them is written as before. A line naming something other than a car is
    dropped.
  - `PAGE` and `camera_choices` list each car's disc presets after the helicopter's, labelled
    `Buggy:`, `Monster Truck:` or `Baja:`.
  - Save as New Preset copies a car preset under that car.
- **`crates/openmx/src/menu.rs`.**
  - `Menu.car_camera_names` holds each car's cycling preset names.
  - `parse` takes the new `settings.txt` key.
- **`crates/openmx/src/main.rs`.**
  - `car_camera_names` reads each car's file out of free06's kit archive for the front end.
  - `frame_cars` lays the seat's own presets for that car over its camera, as `frame_heli` does.
  - On the seat's first frame, each car's camera starts on its kept preset. It prints
    `camera: the <car>'s starts on <preset>, where it was left`.
  - The camera button in a car saves that car's preset as its start, where before it saved
    nothing.

A car's camera still takes the Camera page's one set (the bike's sliders) wherever the seat has
no numbers of its own for that preset, as the helicopter's does.

## How it was checked

- **`tests/car_camera.rs` `the_cars_lookback_cam_films_as_the_original`.**
  - Each P264 capture is replayed through `game_sim::camera::Race` on the car's own file. Each
    tick's pad goes through `chase::LookBack::frame` on the shipped table before the camera's
    frame, as `main.rs` runs it.
  - The pad's lead (0 or 1 row) is taken from the first press alone. Then all 650 rows are on the
    original's preset: 165 looking back and 8 changes per car.
  - Eye and view against the original's viewport:

    | Car | Eye | View |
    |---|---|---|
    | baja | exact | 3.9e-8 rad |
    | buggy | 0.0015 ft | 5.9e-6 rad |
    | truck | 0.00055 ft | 2.1e-5 rad |

    These are S246's levels on Race Cam 1. The buggy's and truck's largest eye error is at row 30,
    in the settle, not at a look-back change.
  - Controls:
    - Without the look-back, the baja's eye is 52 ft off.
    - Polling each row's own pad for the baja turns the port's camera a row before the
      original's (row 59 against 60).
- **`tests/car_camera.rs` `a_cars_camera_starts_where_it_was_left`.** The shell is started from a
  paused front end (`--headless --menu --screen pause`) on free11. Its `settings.txt` keeps the
  buggy on "Race Cam 2" and the baja on a name its file lacks. The buggy's camera starts on Race
  Cam 2, and nothing else moves.
- **`tests/menu.rs`.**
  - `a_cars_camera_preset_is_kept_across_rides`: the start goes through a `settings.txt` round
    trip, beside the bike's and another car's. Odd lines are dropped. The kept preset is the one
    `chase::start_on` puts the buggy's camera on.
  - `the_cars_presets_are_on_the_camera_page`: the cars' presets come after the helicopter's. A
    slider change is that car's alone. A custom preset on the buggy joins the buggy's cycle and no
    one else's.
- A headless menu run lists each car's presets off the disc: Race Cam 1, Race Cam 2, Freestyle
  Cam (the baja's is "Freestyle"), Slot Cam.

- **`./kit check -- -p openmx --no-fail-fast`.** All green except
  `tests/menu.rs` `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest` (Fog reads
  "Whole Map", and the test wants "Farther Fog"). It fails the same way on a clean HEAD worktree
  (1c1e7577), so it is P269 and not this task's.
- Under "Mine only", a car's start falls back to the seat's first own preset when the kept one is
  gone, as the helicopter's and the bike's do (`chase::start_on`'s `mine_only`).

## Not verified

- **Pressing the camera button in a car in the shell.** It goes through `camera_left_on`, which
  the menu test covers, but it was not run through a front-end ride.
- **The cars' Camera page rows on screen.**
- **The cars' other cycling presets (Race Cam 2, Freestyle, Slot Cam) against a capture.** They
  are the same `Race` on the same file, and only Race Cam 1 and Lookback Cam have been compared.
- **Split screen and online.** The cars have no cameras online (S268).
