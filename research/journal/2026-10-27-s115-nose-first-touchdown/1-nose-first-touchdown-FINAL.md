# 1 — nose first touchdown (FINAL)

Task S115. 2026-10-27.

## What I did

- Played animandpump back on the original again (PCSX2 copy 1) with S67's `capture/play.sh` and
  `landings.toml` (`SAVE=3457:1,3458:2`, FIRST 1939, 1530 rows). The capture is
  `context/capture/s115/landings.csv` (frames 1939..3485, 4 torn rows). Beside it are the states
  `landings-3457.p2s` and `landings-3458.p2s`, saved on the frames asked for, and their RAM images
  (`./kit state ram … --out landings-<frame>.ee.bin`). S67's own `context/capture/s67/` came from s3's
  context.
- Added the two states to `each_captured_landing_rides_like_the_original` as `S115_NOSE`. Its first
  run turned the task around: **the gap S67 saw on S67's own 3452 state was already gone on main.**
  The state's worst gap from the touchdown on was 0.05°, cofg 0.004, vel 0.050, against S67's 2.69°,
  0.397, 2.356. The port goes into first gear at 3464, on the same tick as the original. Some sim
  change between S67 and now closed it; see "Not verified".
- Wrote `the_nose_first_touchdown_tick_is_the_originals` (tests/replay.rs). From each S115 image, the
  original's whole `bike_vehicle_update` runs one tick in the oracle, on the dt of the pad that tick
  read. The port, seeded from the same image, runs its one tick beside it. Bit for bit on the chassis
  quaternion, centre, velocity and spin, the node's three rows and origin, and both wheels'
  compression and spring force. On both ticks everything agreed except the centre's z and the
  origin's z, each one ulp off (`0x457e2f4d` against `0x457e2f4c`).
- Found the ulp. I hooked `integrate_ab2` and `chassis_integrate` in the oracle and ran the port's
  `chassis::advance` on the original's own inputs: all four integrator calls matched. The input that
  differed was the centre handed to the position call, which is the integrator's PointCofg answer.
  The original asks the transform's point call for it, and that call adds the origin into the
  accumulator *first*: `vaddax` of row 3 with `vf0.x`, then `vmaddax`/`vmadday`/`vmaddz` of rows 0, 1
  and 2. The port answered with `math::transform_point`, which is `vu::transform`: x·row0 + y·row1 +
  z·row2 + 1·row3, so the origin goes in last and the sum rounds differently. Fixed in `ride.rs`: a
  new `orientation_point` (the origin-first chain, `xyz` written, `w` the point's) answers
  `Call::PointCofg`. Both ticks now agree on every lane, to the bit.
- Removed `S67_NOSE_FIRST_BOUND`. Every landing is held to `S67_BOUND`.

Worst gaps from the touchdown on, after the fix (`S67_BOUND` = 0.5°, 0.05, 0.3, 0.01, 0.05, 0.01):

| state | angle ° | cofg | vel | comp | drawn | aim |
|---|---|---|---|---|---|---|
| landings 2143 | 0.01 | 0.002 | 0.004 | 0.001 | 0.0021 | 0.0002 |
| landings 2745 | 0.03 | 0.013 | 0.121 | 0.001 | 0.0132 | 0.0007 |
| landings 3386 | 0.01 | 0.002 | 0.004 | 0.001 | 0.0019 | 0.0001 |
| landings 3452 (nose-first) | 0.04 | 0.003 | 0.043 | 0.002 | 0.0024 | 0.0009 |
| landings 3492 | 0.00 | 0.002 | 0.003 | 0.000 | 0.0024 | 0.0001 |
| run2 2750 / 2751 | 0.02 | 0.005 | 0.033 | 0.001 | 0.0049 | 0.0003 |
| s115 3457 | 0.04 | 0.003 | 0.052 | 0.004 | 0.0025 | 0.0009 |
| s115 3458 | 0.04 | 0.003 | 0.047 | 0.003 | 0.0034 | 0.0009 |

Re-pinned, all from the one ulp: `HUNDRED_TICKS` in lib.rs, tests/hash.rs and tests/platforms.rs.
In platforms.rs, `HASHES`, `TRACE_DIGEST`, `VEL_Y`, `FINAL_DUMP_HEAD`/`DIGEST`,
`FIXTURE_TEXT_DIGEST`, `RIDERS_HASHES`, `RIDERS_DIGEST` and `RIDERS_WIRE_DIGEST`, each spliced from the
generators. In tests/ride.rs, MD25's two folds. These are the same pins S67's fixes moved.
`RIDERS_WIRE_LEN` and the state's shape did not move.

Nothing from opentennis: it has no bike chassis.

## What the original does

- `./kit dis 0x001b3060` (orientation_transform_point): first the rebuild check `./kit fn 0x001b1750`,
  then `vaddax.xyz ACC, row3, vf0x`, `vmaddax.xyz ACC, row0, p.x`, `vmadday.xyz ACC, row1, p.y`, and
  `vmaddz.xyz out, row2, p.z`. Only `xyz` is written, so `w` is the point's.
- `./kit fn 0x0015c540` (chassis_integrate) calls it first, with the transform and the chassis'
  `+0x190` (the centre of gravity in the body frame), and stores the result to `+0x60`. The position
  call to `integrate_ab2` (`./kit fn 0x0013e460`) then integrates `+0x60`.
- The oracle's note on `orientation_transform_point` says "without its origin". That is wrong, at
  least for this call: the `vaddax` of row 3 puts the origin in. I did not change the note, because
  it belongs to S19/V5's block. This journal is the correction.

## How it was proved

- **The touchdown tick:** `the_nose_first_touchdown_tick_is_the_originals`, the original's
  `bike_vehicle_update` on 2 RAM images (3457, the last airborne tick before, and 3458, the
  touchdown) against the port's tick from the same images. 16 lanes, bit for bit.
- **The landings:** `each_captured_landing_rides_like_the_original`, now 9 landings, all within
  `S67_BOUND`.
- **The rest:** game-sim's whole suite (770 tests) with the re-pinned vectors, including the wheel
  and chassis oracles and the other S67/S107 save-state runs, all unchanged in what they hold.

## Not verified / not 1:1

- **What closed S67's 2.69° gap is not known.** Between S67 (285d7f91) and this branch, the 3452
  state's worst gap fell from 2.69° to 0.05° with no work aimed at it. I did not bisect it, because
  each step needs a scratch worktree and a full game-sim test build. The candidates that touch the
  path are S107 (the general inverse inertia) and S114 (the wheel ray probe, which replaced
  belly_reach). S115's own fix is only the last ulp.
- **One tick, not a run, is bit for bit.** The oracle runs the vehicle update once per image, on the
  controls already in RAM. Running it on further needs each tick's control state written in, and
  that was not built. Past the touchdown the landings are still held to a bound against capture rows
  read at three phases of the frame (S67's reasons).
- **The fix covers the integrator's PointCofg only.** `ride.rs` still uses `math::transform_point`
  for the centre where the port places or re-classes a bike (`place_on`, the build swap's re-centre).
  It is also used for the probes' end points and the wreck bike's start. Which point call the original
  makes in those places was not checked. The integrator recomputes the centre on its next substep, so
  the first two cannot carry an ulp past one integrate.
- **The 3458 tick is not a bottom-out.** The front lands at comp 0.166 with the rear in the air, so
  the brief's "front-only bottom-out path" is not exercised by the 3458 tick. S67's tests/wheel.rs
  oracle covers the bottom-out push itself.
- **The other slots' copies of the pinned constants.** A branch that moves the same vectors will
  conflict with these literals. The merge must regenerate them, not pick a side.

## Next

none — the task is done.
