# 1 — backwards crash (FINAL)

Task S68. 2026-10-24.

## What I did

- Recorded two backwards hard turns in the original, from the recording's save state (free09,
  first_recording.p2m2_SaveState.p2s, cr125 on race125p) at a quarter of the speed (S58's recipe:
  `capture/setup.py 0.25` puts the state in slot 8 and sets NominalScalar; `capture/capture.sh CASE ROWS`
  under `./kit pcsx2 with --` records `watch/s68-backwards.toml --state 8` while `./kit pad script
  CASE.pad` drives the pad; pad times are ×4):
  - `flat` (330 rows): gas + brake held at a stop → reverse; backs at ~-14 ft/s, then full right lock for
    the rest of the run, over a small bump.
  - `slope` (360 rows): a forward full-lock turn at the top of free09's steep slope, brake to a stop
    facing back, gas + brake → reverse, over the edge and down the slope backwards at full lock, to
    -30 ft/s with pitch 0.74 and a short hop in the air.
  The csvs are `context/capture/s68-backwards/{flat,slope}.csv` in s1 (git-ignored, not committed).
- `watch/s68-backwards.toml`: S58's s58-slip lanes plus the reverse (+0x648), bike speed (+0x438), the
  brake lever, the sticks and leans, on_ground/airborne/flipping, and the chassis' pitch/roll angles.
- Read the original's wreck test and the port's beside it (`./kit fn 0x2522c0`, `air::wreck_test`), and
  the ground roll command (`./kit fn 0x255d20`, `move::no_hander`).
- Test: `crates/game-sim/tests/replay.rs` `the_captured_backwards_turns_never_wreck`: replays both
  captures from the save state in the port, the stick from the captured pad (S58's one-tick pad lead),
  the gas/brake/clutch levers and the reverse flag from the capture's lanes.
- Brought over two of S58's (task/S58, not merged yet) replay.rs hunks byte for byte, because the replay
  needs them: `quarter_ticks` ignoring the pad's `stamp_` columns, and the save state seeding the front
  fork's `steer_axis`. They are the same lines as S58's, so the two branches merge either way.

## What the original does

It never wrecks a bike that backs up and turns hard into the ground — the brief's premise does not hold.
Both captures run out with wreck reason 0 on every row.

- Reverse (P73's note on the pad bindings): gas and brake both held while the speed +0x438 is under 5 sets
  +0x648 = 1 and zeroes both levers; the bike then backs up under the reverse push. +0x438 is signed
  (negative backwards).
- The wreck test (`./kit fn 0x2522c0`) has no direction in it: the g-load limits, the ground pitch limits
  (only while on the ground), the roll limit (only in the air and not flipping), the landing crossup and
  the head ray. None reads the speed's sign.
- The ground roll command (`./kit fn 0x255d20`) maps the speed from 0..max_roll_speed to 0..max roll; a
  negative speed clamps to 0, and the backwards fade sets the target upright. So a reversing bike does not
  lean into a turn: on flat at full lock its roll stays under 0.06 rad, and down the slope under
  0.21 rad (most of that in the hop). Nothing comes near a limit, so nothing wrecks.

The port's `air::wreck_test` and `move::no_hander` are the same as the original's on all of this, and the
replay agrees:

- flat: no wreck in 330 ticks; roll and pitch within 0.03 rad and the signed speed within 1 ft/s on every
  tick, reverse and full lock included (steer angle, wheel spin, wheel side forces print the same to
  about 1 %; the largest gap is 0.022 rad of pitch over the bump at tick 138).
- slope: no wreck in 359 ticks. It agrees through tick 89 and parts in the forward turn at the top (pitch
  -0.065 vs -0.096 at tick 90), before it ever backs up. After that the backwards ride follows the
  original closely to the edge (tick ~270), then goes its own way down the slope. Largest roll while
  reversing: port 0.295, original 0.206.

So there is no port fix: the port behaves like the original going backwards. The user's 10-21 report
("turns hard into the ground and never crash") describes what the original also does. Also, the port
has no reverse from the pad today: `Controls::through` leaves `reverse` at 0, and the gas + brake walk
belongs to P72/P77. Whatever the user rode on 10-21 was not the original's reverse.

## How it was proved

Two save-state PINE captures (flat 330 rows, slope 360 rows) replayed tick by tick in the port by
`the_captured_backwards_turns_never_wreck`. It asserts:
- the original's wreck_reason is 0 on every row;
- the port never wrecks;
- the reversing bike's roll stays under 0.35 rad in both games;
- the ratchet `S68_AGREES_THROUGH` = [330, 89] (roll/pitch 0.03 rad, speed 1 ft/s).

Ran with `./kit check -- -p game-sim --test replay -- --nocapture the_captured_backwards`. Its log
says "2 of 2 backwards captures ridden", so it is not skipping.

## Not verified / not 1:1

- The port cannot reverse from the pad (P72/P77). The test feeds it the captured levers and reverse flag,
  so it checks the ride, not the gas + brake → reverse walk.
- The slope replay parts at tick 90, in the forward turn before the reverse (a 0.03 rad pitch gap), so the
  backwards half down the slope is compared only loosely: no wreck in either, and roll under 0.35. The
  port's path goes over the edge on its own line, not tick for tick.
- Only one bike (cr125, race125p) on one track (free09), at a quarter of the speed. Captures at full speed
  were not taken.
- The head ray / body touching the ground never fired in either game in these runs. I did not find a
  backwards case that drives the body into the ground. Wrecks from a reversing bike tipping over were not
  exercised, because neither game lets it lean.
- The speed comparison uses the chassis velocity's length, signed by +0x438. The original's +0x438 is read
  part way through its tick (it lags the row's chassis), so it is used only for the sign.
- The two S58 hunks are duplicated here until S58 lands.

## Next

Nothing for S68. When P72/P77 port the gas + brake reverse, the test can drop its lever overrides and
take them from the pad alone.
