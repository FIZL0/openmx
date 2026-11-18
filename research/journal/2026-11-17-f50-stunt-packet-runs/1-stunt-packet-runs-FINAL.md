# 1 — stunt packet runs (FINAL)

Task F50. 2026-11-17.

## What I did

- **The queue half was already done.** F31b5k (2026-11-09) ported the connection's queue and its flush:
  `update::net_queue` / `queue_push`, proved by `the_connections_queue_is_the_originals`, and
  mxvatv_bike_ride.rs's `UpdateInRam` serves `Out::NetQueue` with it. Nothing left to port, so no change to
  `update.rs` or mxvatv_bike_ride.rs.
- **The stunt stubs are gone from the other six test files.** Each one hooked the stunt packet to do
  nothing. Now each lets it run and, as mxvatv_bike_ride.rs's `hooked()` does, rewrites its two RCNT0
  instructions (asserted first) to read a stand-in word of 0 in the scratch area:
  - mxvatv_bike_ground.rs, mxvatv_step.rs (`an_entry_leaves_the_record_touched`), mxvatv_biplane_replay.rs,
    mxvatv_biplane_wreck.rs, mxvatv_kiowa_replay.rs, mxvatv_car_replay/drive.rs.
  - Their doc comments now say "the stunt packet's counter read" instead of "the network send".
- `tests/atv_quad/sim_frame.rs` already had the redirect (MD105); untouched.
- opentennis: nothing to take. Its netplay is ggrs.

## What the original does

See research/oracle.txt `# F44a` (the stunt, `./kit dis 0x004e7e70 --len 0x140 -p atv`) and `# F31b5k` (the queue).
The stunt reads RCNT0 with `lui $2, 0x1000` at 0x004e7f44 and `lw $16, 0($2)` at 0x004e7f50.

## How it was proved

- `./kit check -- -p game-sim` on the six binaries: 52 tests passed with the stunt packet running.
- **No silent skips:** with the assert's expected `lui` word changed to a wrong one in all six files, the same
  run failed in all six files (six assert panics), so every file reaches the redirect on a real image.

## Not verified / not 1:1

- **RCNT0's real count is still not captured.** Ee cannot read hardware registers. In all these replays the
  stunt packet's stamp reads a stand-in 0, not the PS2's counter. Same limit as F44a.
- **What the stunt packet's queued bytes do in these six files is not compared on its own.** The files compare
  what they compared before; the stunt packet now writes the connection's queue in RAM too, and they still pass,
  but none of them asserts the queued stunt messages against a capture. State 4 and the vehicle states are
  offline, so no capture holds a sent packet to compare with.

## Next

Nothing: the task lands.
