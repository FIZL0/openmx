# 1 — mxvatv car ride replay

Task MD149a. 2026-11-15.

## What I did

- `atv_car::Ride`'s chain is now one generic function, `chain` (crates/game-sim/src/atv_car.rs):
  `Own` → `PutBack` → `Pieces` → any `rest`, the keys and state taken out of the Ride and put back.
  - `Ride::frame_on` calls it with `OnWorld` and the class update, as before (box, area and put-back
    spot unchanged).
  - New `Ride::frame_with(dt, rest)` calls it with any `rest` and the frame alone.
  - `PutBack` is now generic over the pieces' `rest` (it was tied to `OnWorld`).
- MD135's replay (crates/game-sim/tests/mxvatv_car_frame/drives.rs) takes a `Through` mode:
  - `Frame`: MD135's own `atv_car::frame` over `Own`/`Pieces` (unchanged);
  - `Ride`: the words read off the machine laid into one `Ride` per drive, `Ride::frame_with` with
    MD135's `Beside` (Desert Oasis ground, the image's noise tables, the original's sort answer) in
    place of `OnWorld`, then the Ride's words compared exactly as MD135 compares its own.
- Five new tests, `the_*_drive_is_the_rides`. They also assert no frame wrecks and no `Respawn` is
  handed on.
- Took nothing from opentennis: it has no MX vs. ATV car.

## Results

All five drives, 730 frames each, every compared word equal to the interpreter's through `Ride`:

| drive    | words compared | landing / shift sounds | sort struck | record held contacts |
|----------|---------------:|-----------------------:|------------:|---------------------:|
| buggy    | 1,539,570 | 4 / 11 | 16 | 0   |
| baja     | 1,537,380 | 5 / 10 | 17 | 0   |
| sandrail | 1,541,482 | 4 / 12 | 9  | 112 |
| golf     | 1,539,570 | 2 / 4  | 3  | 0   |
| truck    | 1,544,340 | 5 / 9  | 10 | 92  |

MD149's free09 test (`each_car_rides_free09_jumps_wrecks_and_is_put_back`) still passes on the
refactored `frame_on`.

## Not verified / not 1:1

- **The test lives in tests/mxvatv_car_frame/drives.rs, not tests/mxvatv_car_replay.rs** as the
  brief named. MD135's machine readers and world answers (`read_car`, `read_vehicle`, `Oasis`, the
  sort answer) are in the mxvatv_car_frame test crate. Copying them into another binary would
  duplicate about 2,000 lines.
- **The Ride's `PutBack` layer is never exercised.** No drive wrecks or respawns, so `Respawn` and
  `PutBack` only pass through. Its answers are MXU's put-back, not the original's (MD154). The tests
  assert this stays so.
- **`class_update` through the Ride is not replayed.** The original runs the vehicle update outside
  the world step this replay hooks. `frame_on` runs it on the same chain, but its words are not
  compared here.
- **`frame_on`'s own wrapping is not replayed**: the world box and area laid into `Vehicle`, and
  the put-back spot kept after the frame. Both need an MXU world, which the drives do not ride.
- **The Ride is rebuilt from the machine each frame** (keys, state, car, body, vehicle laid over).
  So this proves one frame at a time through the type, not the Ride's words carried frame to frame.
  The same holds for MD135's own replay.
