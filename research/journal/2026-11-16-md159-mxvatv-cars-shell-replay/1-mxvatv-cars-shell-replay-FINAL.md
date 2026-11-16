# 1 — mxvatv cars shell replay (FINAL)

Task MD159. 2026-11-16.

## What I did

- The shell's car (`MxvatvCarRide`, crates/openmx/src/vehicle.rs) got two additions:
  - `seed(ride)`: the car out as a given `atv_car::Ride`, taken where its node stands.
  - `step_with(pad, table, dt, rest)`: one tick the way `step` takes it (`rest` cleared, the pad's input
    step through `AxisRange::PLAYER` and the bindings), then `Ride::frame_with` with `rest` answering in
    place of an MX Unleashed world, then the wreck count. `step`'s wreck count moved into a shared
    `count_wreck`, with no change in behaviour.
- New test topic crates/game-sim/tests/mxvatv_car_frame/shell.rs, five tests `the_*_drive_through_the_shells_car`:
  - compiles openmx's `vehicle.rs` in by path, the way openmx's own tests do (it only uses game_sim and ps2_float);
  - builds the shell's car with `MxvatvCarRide::read` off MD39's `.phy` and `.psg` (context/mxvatv-ps2/md39),
    the way the shell reads a car pack;
  - **seeds it once** from the drive's RAM image (keys, state, input keys and words, car, body, vehicle laid
    into the car the shell makes), then **carries it**. Nothing of the car is read off the machine again;
  - each frame, at the original's world step, steps it with `step_with` on a real `Pad` (via `Manager`)
    playing tape tick `k − 2` through `Table::SHIPPED`. Each tape button goes to the commands MX vs. ATV
    binds it to (MD107's `button`), then onto the control the shell's bindings put each command on. The
    test asserts each asked command's value and the stick's bits equal the tape's;
  - world answers: MD135's `Beside` (Desert Oasis ground, the image's noise tables, the original's sort
    search) plus the collision record's answers laid in from the machine each frame (see below);
  - compares every word MD135 compares (car, body, pieces, vehicle, resolved record), plus the input step's
    nine words, after every frame. Asserts no wreck and no `Respawn`.
- drives.rs: `HANDED_OUT`, `sort_answer`, `Beside` (and its fields) and `world_step_inside` are now
  `pub(crate)` so shell.rs can borrow them. Visibility only.
- crates/openmx/tests/mxvatv_cars.rs: its module note now points at the new test, not "MD149a, still open".
- Filed **MD175**: `Ride::record_on` sets `Vehicle::touching` but never `State::touching`, the pieces'
  copy of the same record word (see below).
- Took nothing from opentennis: it has no MX vs. ATV car.

## What the original does

Same as MD107/MD135. The car's collision record runs as part of the world's step, before the car's
listeners. Its touching word is read twice: by the vehicle (`Vehicle::touching`, the tipped wreck) and by
the pieces (`State::touching`, the air control and the landing check). The truck's drive first touches at
frame 363. With only the vehicle's copy laid there, the shell's car parted from the original that frame,
in speed, slip and the node's rows. With both copies laid, all 730 frames held.

## How it was proved

Bit-exact replay beside the interpreter (SLUS-21104's sim frame off each MD39a image, MD107's tape),
crates/game-sim/tests/mxvatv_car_frame/shell.rs. The shell's car is carried from the image the whole way:

| drive    | frames | words compared | sort struck | record held contacts | landing / shift sounds |
|----------|-------:|---------------:|------------:|---------------------:|-----------------------:|
| buggy    | 730 | 1,539,570 | 16 | 0   | 4 / 11 |
| baja     | 730 | 1,537,380 | 17 | 0   | 5 / 10 |
| sandrail | 730 | 1,541,482 | 9  | 112 | 4 / 12 |
| golf     | 730 | 1,539,570 | 3  | 0   | 2 / 4  |
| truck    | 730 | 1,544,340 | 10 | 92  | 5 / 9  |

So the step goes further than MD149a, which rebuilt the Ride from the machine every frame: here the shell's
car carries its own words for 730 frames, with its own input step on the tape's pad.

## Not verified / not 1:1

- **The test is in game-sim's tests/mxvatv_car_frame/shell.rs, not crates/openmx tests/mxvatv_cars.rs** as
  the brief named. The machine readers and world answers (`read_car`, `read_vehicle`, `read_mx`, `Oasis`,
  the sort answer) live in that test crate. Pulling that crate into an openmx test would run its 30-odd
  tests twice. Instead the shell's own `vehicle.rs` is compiled in there by path, so the type proved is the
  shell's.
- **The collision record's answers are laid from the machine each frame, not computed by the shell's car**:
  its touch (`Vehicle::touching` and `State::touching`) and its held contacts. The record's pass is a world
  query: on MX Unleashed's worlds the Ride runs it (`record_on`) against MXU geometry, which these Desert
  Oasis drives do not have. The resolution of the held contacts is the car's, and it is compared.
- **The shell's car never sets `State::touching` on an MX Unleashed world** (filed MD175). The replay lays it
  in, so it holds here. In the shell, a car touching a prop still gets the air control.
- **`step_with` runs `Ride::frame_with`, not `frame_on`.** The record pass, the world box and area, the
  class update and the put-back spot that `step` runs are not replayed:
  - the record pass needs an MXU world (above);
  - the original runs the class update outside the world step this test hooks. Its effects on the
    compared words would show up as differences, and none did over 3,650 frames, but its own words
    (odometer, rumble) are not compared;
  - no drive wrecks, so the put-back is never reached.
- **The pad goes through the shell's shipped bindings, not MX vs. ATV's.** The tape names MX vs. ATV's
  buttons; the hand brake (circle there) is R1 in the shell's table. The test maps by command, and
  asserts each command's value and the stick bits are the tape's. So the input step sees the same values,
  but the bindings themselves are MX Unleashed's (as in the shell).
- **Frames 1 and 2 use an idle pad.** MD107's replay keeps the image's own input words there (no tape tick
  yet). The shell's input step on an idle pad gave the same words: the input compare passed on those frames.
- **The free09 drive is not re-run through the binary here**: MD39c's test still holds the binary's car
  to `atv_car::Ride`. It skips in this checkout, which has no mods/machines/mxvatv_cars.

## Next

None.
