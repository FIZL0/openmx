# 1 — wheel start state (PART)

Task S64. 2026-10-22.

## What I did

- **The save state's whole bike into the ride** (`crates/game-sim/tests/replay.rs`, `impl SaveState`):
  `seed_wheel`/`seed_wheels` read both wheels' spin-step words (spin, compression and last
  compression, spring force, load point, ground normal, contact velocity, slip flags, local velocity and
  force, angle, steer) off the rider's wheel pointers (`rider +0x7b0` front, `+0x7b4` rear) into
  `ride::Wheel`. `seed_chassis` reads the chassis integrator, including its four histories (`+0x2b8..+0x31c`)
  and the node transform. `seed_balance` reads the balance accumulators, the pitch integral and the stoppie
  latch. `seed_air` reads the air state.
- **The air state was the tick-1 kick.** Left at its default, the port starts airborne. Its first ground step
  then frees the roll axis (`chassis.ang_free.z = 1`) for a tick, while the original keeps it locked. That
  kick was the brief's `bike.vel[0]` 3.4e-5 against 0.044, and the roll rate with it. The original's words
  (bike_air_state, `./kit fn 0x251d20`) are on the rider:

  | offset | word |
  |---|---|
  | `+0x574` | on_ground |
  | `+0x578` | both_wheels |
  | `+0x57c` | airborne |
  | `+0x580` | was_airborne |
  | `+0x4f0` | hang_time |
  | `+0x430` | contact_vel |
  | `+0x600` | crossup |

  In first_recording's state they read 1, 1, 0, 0.
- **The ground query is now the original's** (`crates/game-sim/src/world.rs`). `World::ground` on a `Ground`
  was `Ground::at`'s bilinear blend. It is now `contact_search::ground` over `Ground::field`: the cell split
  into two triangles, with the corner normals blended and normalised, which is the arithmetic of the wheel's
  ground_query (`./kit fn 0x1dc960`). With seeded wheels, the bilinear blend was what parted tick 2's
  `load_point[1]` and compression. Off the field, `Ground::at` still decides that there is no ground.
- **A capture of the tire half** (`watch/s64-wheels.toml`): s8-start's fields plus the following.

  | field | offset |
  |---|---|
  | slip flags (spinning, locked, sliding) | `+0xe0`, `+0xe4`, `+0xe8` |
  | local velocity | `+0x840` |
  | local force | `+0x850` |
  | wheel angle | `+0x1d0` |
  | side force per load | `+0x234` |
  | drive torque | `+0x210` |
  | steer rate | `+0x8b0` |
  | balance command | rider `+0xa60` |
  | balance point | rider `+0xa64` |
  | the chassis' four histories | `+0x2b8..+0x31c` |

  It is recorded at a quarter speed, into `context/capture/s64-slow/s64-wheels-quarter.csv` (120 rows,
  counters 2775..2894). The reason is phase:
  - At full speed the recorder reads each row a frame late, and the row for tick 1 is never written.
    s8-start's row 0 is the save state and its row 1 is already tick 2. On one re-record the phase also
    slipped mid-run.
  - At NominalScalar 0.25 (with settle_ms 240, since 60 finds rider_type null) each row is read *before*
    its frame. So a row at counter `c` is tick `c − 2774`, and the first row is tick 1.
  - Twice in 120 rows (2818 and 2869) the recorder was still late. The sign is the next row repeating the
    state exactly. `quarter_ticks` moves such a row on by a tick and drops the last row, which has nothing
    after it to tell by.
- **The test** `the_save_state_run_starts_from_the_originals_wheels` builds the acceptance run's start
  (`acceptance_start`), seeds it as above, and replays it on an idle pad against every lane of that capture.
  It prints each lane's own first divergence and holds two ratchets:
  - `SAVE_STATE_AGREES_THROUGH = 0`: tick 0 agrees on every lane.
  - `SAVE_STATE_POSITION_AGREES_THROUGH = 21`: the bike's position (`rider.ride.origin`) agrees through
    tick 21. `x` and `z` agree on every tick of the capture; `y` parts at tick 22 by one ulp of 208.

  Before S64 the position parted at tick 1, by 0.00086.
- The chassis' speed lane is compared one tick later. The original's row carries `|ch_vel|` of its own
  velocity, the save state's row too, whereas the port measures speed at the top of the next tick
  (`chassis::measure`).

## What the original does

- The wheel's ground point is the terrain's triangle split along the cell's (+x)-(+z) diagonal. It is
  not bilinear (S8's 208.45465 vs bilinear 208.4905 under the front tire).
- The rider is locked in roll on the ground. The capture's body roll `ang_accel.z` and `ang_vel.z` are
  0 on every row, and the port's now are too (`ang_hist.z`, `ang_vel[2]` agree on every tick).

## How it was proved

A save-state capture compared tick by tick: `s64-slow/s64-wheels-quarter.csv`, 118 ticks and about 150
lanes, against the port started from the same RAM image
(`context/states/first_recording.p2m2_SaveState.eeMemory.bin`, frame 2773).

- Tick 0 agrees bit for bit on every lane.
- From tick 1 most lanes part by a few ulps: chassis `vel[0]`, the tire forces' `x`, `local_velocity`,
  `side_per_load`, the quaternion's `x`/`y`/`w`, and the angular rates.
- Lanes that agree on every tick:
  - the slip flags;
  - `local_force[1]` on both wheels;
  - `drive_torque`;
  - steer angle and rate;
  - balance command and point;
  - the engine's four outputs;
  - `origin[0]` and `origin[2]`.
- `vel[2]` agrees through tick 22, compression through tick 22, and `vel[1]` through tick 1.

## Not verified / not 1:1

- **Tick 1 is not bit-exact.** Most lanes are 1-10 ulps off. Two are further:
  - the roll quaternion `quat[2]`: −0.00022476875 against −0.00022505423, 1.3e-3 of it;
  - the pitch angular acceleration `ang_hist.x`: −0.00020550 against −0.00020278, 1.3%.

  The port's `quat[2]` moves +8.8e-8 on tick 1 against the original's +3.7e-7, then matches its rate from
  tick 2. The derivative (`quat_hist`) agrees to ulps, so the extra is in the transform delta that
  `integrate` carries into the quaternion. Things I ruled out:
  - The node matrix in RAM is `quat_to_mat4(quat)` to 1e-8, and starting without it changed nothing.
  - The balance accumulators, pitch integral and stoppie latch are seeded, and they agree.

  Not found. The test's ratchet holds the all-lane agreement at tick 0 for this reason.
- **Off the field**, the original's ground query leaves the point where it was, so a wheel past the edge
  finds ground at its own probe's end. The port keeps `Ground::at`'s "no ground" there. This is not
  proved, and the capture never leaves the field.
- **`ground_query` = `contact_search::ground`** is shown by this capture's wheel lanes (load point, normal
  and compression agree to tick 22). No Ee oracle over generated points was run.
- **The capture's phase**: the duplicate-row rule (a row equal to the next is a tick late) is inferred from
  two slips in 120 rows, not read from the recorder. A late row whose next row also happens to be late
  would be mis-ticked.
- **Still unseeded**: the trick state, input timers and held levers, `steer_command`, preload/flip state,
  and the rider's authority. On an idle pad from a standing start none of these shows in a lane, but none is
  proved either.
- **The RNG seed** is still the acceptance fixture's row 0 (`acceptance_fixture(&acc).seed`), not read out
  of RAM.
- **The acceptance verdict itself** (`ACCEPTANCE_AGREES_THROUGH`) stays at 0. Its lanes are the *drawn*
  bike (`bike_xform` is the model root, `bike_vel` is 0.8 × the chassis), which the port does not have.
  S64's test compares the chassis instead. `acceptance_verdict`'s own start is not seeded with the wheels
  or the air, because against those lanes it cannot move.
- Without the quarter-speed capture (other slots), the test falls back to s8-start.csv and checks no
  ratchet, because that capture has no tick 1 and fewer lanes.

## Next

Filed as S106. Find tick 1's roll and pitch gap: run the original's chassis step (Ee) from the save state for one frame,
substep by substep, and compare `out` (the transform `mv::balance`/`mv::roll_pitch` write) and the torque
sum against the port's.
