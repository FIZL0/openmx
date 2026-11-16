# 1 — car roof and prop capture (FINAL)

Task C34. 2026-11-16.

## What I did

- **Roof capture.** Switched into the buggy on free07 (pause menu → Switch To Machine, with the cheat
  flag set), turned it on its roof over PINE (`research/c34/flip.py`), and played a tape that holds the
  gas until the stuck timer (`MaxWreckTime`) resets it, then drives on (`research/c34/roof.pad`).
  Captured with S149b's `car_tire.toml` through `research/c34/capture.sh` →
  `context/capture/c34/roof.csv`, 299 rows. The car image is `context/states/c34-buggy-switched.ee.bin`.
- The first take lost the reset frame to a recorder gap. Lowering this slot's `NominalScalar` from 0.3
  to 0.15 fixed it. It is set back to 0.3.
- **Roof replay.** Added `the_roof_reset_replays` and `the_roof_reset_runs_on_its_own` in
  `car_drive.rs`. They use S149's replay with the image given by name (`replay_from`).
- **Found and fixed an order bug** in `car.rs`. The port ran the reset after the measure. The original
  runs it inside the measure, before the speeds and frame are worked out. So:
  - the reset frame's speeds and frame are the reset body's (all zero, alpha π/2);
  - the reset's turn uses the forward from the frame before.
  - `Machine::measure` now calls `put_upright` straight after the wreck timer. The call after the
    measure in the post is gone.
- **Prop capture.** Did the same on free06 (`context/states/c34-free06-buggy-switched.ee.bin`):
  - Found the six balls' bodies in RAM by the positions the port loads them at (`research/c34/bodies.py`).
  - Set the car down 40 ft short of ball 0, facing it, as S335's test does (`research/c34/place.py`).
    Saved that as the start (`context/states/c34-ball-start.ee.bin`).
  - Drove it on full gas (`research/c34/ball.pad`). The spec also logs every ball's body
    (`research/c34/balls.toml`) → `context/capture/c34/ball.csv`, 220 rows, no gaps.
  - The car hits ball 0 at fc 776. Balls 2, 5 and 3 wake at 778, 781 and 783.
- **Prop replay.** Added `the_ball_strike_runs_on_its_own` in `car_drive.rs`. It runs free06 the way
  the shell builds it (new `in_track`; `in_free07` now calls it) and loads the disc's props. It steps
  the car through `Machine::step_props` and the props as `car_props.rs` does, and compares the car's
  body and every ball's body to every row.
- Notes in `research/oracle.txt` (C34 block under S335). Follow-up **S340** filed for the strike gap.

## What the original does

- `./kit dis 0x00398150` (the car's measure):
  1. the body wrapper's post;
  2. the engine's shift;
  3. the wreck step through vtable +0x1cc (its jalr at 0x003981d4);
  4. only then the yaw hold's results, speeds, matrix and angles.
- The wreck step calls the reset (vt +0x19c) when the stuck clock passes `MaxWreckTime`.
- The reset turns the body to face the frame before's forward (car +0x590) on the ground normal, so the
  measure that runs after it sees a stopped, upright body.
- S335 ported the reset as a step after the measure. That gives the same body, but the wrong measure
  words on that frame and a different frame after it once carried.

## How it was proved

- **Roof** (`context/capture/c34/roof.csv`, bit-exact replay):
  - Row by row: 293 rows (6 skipped as paused or double frames), every word to the bit, the reset
    frame (fc 1425) included.
  - Carried: the same 293 rows, the port running on its own state, with no mismatch.
  - Before the fix, the only mismatches were the reset frame's measure words, and the carried run
    went off from there.
  - S335's `a_car_on_its_roof_is_put_back_on_its_wheels` and the S149 drive replays still pass.
    All 15 tests in `car_drive.rs` and `car_props.rs` pass.
- **Prop** (`context/capture/c34/ball.csv`, bit-exact replay, carried from row 0):
  - All six balls' places and turns as loaded from the disc match the original's sleeping bodies to
    the bit.
  - The car's body matches to the bit on every row up to and including fc 776, which is 49 frames of
    driving on free06's ground and scenery.
  - On all 219 rows every ball is awake exactly when the original's is.
  - The test asserts these three things.

## Not verified / not 1:1

- **The ball's answer to the strike is not the original's.** On fc 776 ball 0's body is about 2% off.
  - Velocity: port (39.32, 0.79, 14.03), game (38.43, 0.95, 13.50).
  - Spin differs too.
  - The car is exact on that frame and goes off from fc 777 on, as the ball pushes back. Everything
    after 776 differs.
  - Not tracked down: it needs the car's push on the ball and the ball's own step taken apart. Running
    the props without the car as striker panics, because the ball's contacts name it. Filed as **S340**.
- Only the buggy was captured, for both the roof and the strike.
  - The truck and the baja reset through the same code. Not captured, to save time; the reset is
    shared, but their measure words are unproven.
- The ball capture's start was set by PINE writes during a running frame, so its first rows are a car
  dropped from 1 ft, not a normal drive. Rows are compared only from the saved state on, so this does
  not affect the proof.
- The ball replay takes the car's constants (masses, gravity) from the free07 roof image, not from a
  free06 image. They are the buggy's own and do not change with the track. The bit-exact car rows
  before the strike bear this out.

## Next

Nothing for C34. S340 holds the strike gap.
