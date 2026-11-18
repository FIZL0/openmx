# 1 — untamed terrain replay (FINAL)

Task MD234. 2026-11-17.

## What I did

- `tests/mxvatv_car_frame/untamed_terrain.rs` (new): eight tests, the four MD160 drives × (frame_untamed,
  Ride), with a `UPort` world:
  - the ground from `atv_ground::Terrain::of` over `context/untamed/sx_a/SX_A.ps2.trn` (MD168b's extract of
    the disc's `SX_A.res`; MD168b found Rocksbury is SX_A);
  - the noise from `SmoothNoise::from_tables` on the image's tables (Untamed's addresses in oracle.txt
    `# MD234`), checked built and a permutation.
- `untamed.rs`: `u_beside_with(d, ride, port)`. With a port, `UBeside` answers `GroundUnder`, `TireGround`
  and `Noise` from it. The original's logged answer is only used to check: same point, normal and record
  byte, and the same noise, bit for bit, on every compared frame. `u_beside` is the old way (`port: None`).
- Fixed MD161a's throttle tests (frame and Ride), red on main. MD131 merged after MD161a's work and made the
  tire step ask `StandsOn` / `ObjectCofg` instead of handing out `TireStep`. The harness now files those as
  "a tire on an object, frame not compared", as it did with `TireStep`. The frames are the same as before:
  161, 258–260, 268.
- Took nothing from ../opentennis: it has no terrain or car code.

## What the original does

- Untamed's ground query and terrain cell are the same code as SLUS-21104's (research/untamed-machines.md);
  the terrain file format is the same.
- Untamed's smooth noise (`./kit fn` does not reach it; `context/untamed/decomp/boot.c`, `// 001324b0`) has
  the same shape as SLUS-21104's, with its tables moved by +0x14b40.

## How it was proved

- Bit-exact replay beside the interpreter, on MD160's captures and tapes:

  | drive | frames compared | port ground answers | port noise answers |
  |---|---|---|---|
  | throttle | 305 (5 more on an object, not compared) | 1552 | 82 |
  | steer | 280 | 1400 | 148 |
  | jump | 370 | 1861 | 154 |
  | wreck | 414 (1 more put back, not compared) | 2081 | 1328 |

  The same counts both ways (frame_untamed and Ride). `./kit check -- -p game-sim --test mxvatv_car_frame
  untamed`: 20 tests pass.

## Not verified / not 1:1

- **The noise tables come from the image, not built by the port.** The original builds them off `rand()`
  on its first sample, before the image was taken. The rng state at that moment is not known, so
  `SmoothNoise::build` was not checked against them. SLUS-21104's drives do the same.
- **Frames not compared are not checked for their answers.** On the 5 throttle frames with a tire on an
  object and on wreck frame 83 (the put-back), the port's answers are used but not compared with the
  original's. The original's put-back query is Untamed's own put-back, which is not ported.
- **Each frame still starts from the original's words.** This is a beside-replay like drives.rs: the car,
  body and pieces are read from the interpreter before each frame. A free-running port replay is not this
  task.
- **The respawn's own query is not the port's.** The put-back (0x004ca900) is not ported (MD161a's note).

## Next

none
