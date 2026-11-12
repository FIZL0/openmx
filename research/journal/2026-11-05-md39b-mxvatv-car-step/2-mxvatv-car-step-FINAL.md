# 2 — mxvatv car step (FINAL)

Task MD39b. 2026-11-12.

## What I did

- Closed MD39b on its splits. Part 1 (2026-11-05) did the first two clauses: `Car::read_mxvatv` with its
  `[Wrecks]`/`[Camera]` keys behind `MxvatvMachine`, and the field map on three RAM images. Part 1 then
  split off the rest, and those tasks have landed on main:
  - **C29**: remade the golf cart and sand rail states, the drive tape and all five drive csvs
    (`context/capture/md39a/*.csv`, `context/mxvatv-ps2/md39a/*.{p2s,ee.bin}`).
  - **MD106** (a–d): ported SLUS-21104's car frame as `game_sim::atv_car`: the listeners, the
    post-process, the update, and the world step around them.
  - **MD107**: `tests/mxvatv_car_replay.rs` replays each of the five drives through SLUS-21104's frame
    driver in the interpreter, seeded from the drive's RAM image and fed only the tape's inputs through
    the port's `atv_car::input`. Every row's car, body, engine and tire words are compared bit for bit.
- Did not rerun `mxvatv_car_fields`, `mxvatv_car_captures` or `mxvatv_car_replay` myself. My targeted check
  waited 50 minutes for a check slot under load and was cut off. `./kit done` ran 0 test binaries, because
  this part changes only the journal. The passes I rely on are the ones MD107's and C29's gates ran when
  they landed.
- No code changed in this part.

## What the original does

See part 1 for the car loader and MXVehicle's wrapping loaders. See MD106's and MD107's journals for the
frame and the input step. All addresses are in `research/oracle.txt` `# MD39b`, `# MD106*` and `# MD107`.

## How it was proved

- Field map: a save-state image check on the five RAM images (`tests/mxvatv_car_fields.rs`), every loader
  word bit-exact.
- Drives: bit-exact replay of the five 730-row captures (`tests/mxvatv_car_replay.rs`).
- Frame pieces: MD106's oracles (`tests/mxvatv_car_frame`), 3,000+ generated states each.

## Not verified / not 1:1

- **No test was rerun on this branch for this part**: see What I did. The proofs are the ones MD107's and
  C29's merges ran.
- **The drives are replayed by the original's frame, not the port's whole frame.**
  - What the replay proves: the captures, the seeding, the tape lag and the port's input step.
  - What it does not prove: that `atv_car`'s whole frame, run by itself, gives the same rows. That needs
    the car update's hand-outs (MD106e) and the calls MD106b's ported code hands out (MD128).
  - Filed as **MD135**, the port-side replay of the same five drives. It is still open.
- **Clutch and the pad word**: the tape presses no clutch, and the replay feeds the pad word as 0. The
  input-step oracle's generated cases are the only proof for them (from MD107).
- **Torn rows**: they are proved against moments within their frame, not against a single moment (from
  MD107).
- **Camera keys**: `[Camera]` keys are read and their words proved on the images. Nothing in the
  simulation uses them yet, so nothing tests how they behave.

## Next

MD135 (after MD106e and MD128), then MD39c (the cars in the game).
