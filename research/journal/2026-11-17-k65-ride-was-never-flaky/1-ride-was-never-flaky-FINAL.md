# 1 — ride was never flaky (FINAL)

Task K65. 2026-11-17.

## What I did

- The two failed gates (`context/notes/gates/41f5bd11…failed`, `6f94ef37…failed`) show the same failure,
  word for word: `s154_cut_short_moves_ride_down_with_air_wrecks_off` at `tests/ride.rs:6334`, "range end
  index 327 out of range for slice of length 321". Two runs giving the same numbers is not a race.
- The test reads nothing shared. No clock, no ports, no files written. The game-sim library reads no env
  vars and opens no files. It only reads the disc.
- `run.log` and the runner's source (agent-runner 0.37.0) explain it:
  - 11:02: MD168b's batch was merged ahead in `openmx-slots/merge2`, and its gate started there.
  - 11:39: the runner restarted. That gate kept running, and no thread held merge2 any more.
  - 12:01: MD168b's turn adopted that gate and waited for it.
  - 12:02: the next merge-ahead (F49, P347, R34, MD72 …) checked out its own merge in the same merge2, while
    the gate was still running there.
  - So the gate built and tested P347's code and reported the failures as MD168b's. The retest ran in the
    `merge` worktree, on MD168b's real merge, and passed. The runner then filed K64/K65/K66 as load flakes.
- Fixed in the runner (agent-runner 027ce7f, 0.37.1): a merge built ahead waits while any gate is still
  running in merge2. Added `tests/test_ahead.py` there. It goes red with the check taken out.
- No change to `tests/ride.rs`: it is deterministic and right.
- P347 ("Air Wrecks off throws a touch with the bike upturned") really breaks this test. It ends the off
  ride earlier, so the on/off comparison slices past its end. That is P347's merge to fix. Its own merge
  gate will catch it. I did not add a plan note, because P347's branch edits the same plan line and a note
  would make its merge conflict.

## What the original does

Not touched: test-gate tooling only.

## How it was proved

- `./kit check -- -p game-sim --test ride --test roof_landing` on main 180e17ab1: 71 passed.
- The same on main + `task/MD72` merged: 71 passed (MD72 is not the cause).
- The same on main + `task/P347` merged: ride fails alone, unloaded, with the gate's exact message
  (327 vs 321). Deterministic, so not load.
- agent-runner `tests/test_ahead.py`: all checks pass. With the two new checks taken out, 3 checks fail.

## Not verified / not 1:1

- roof_landing (K66) passed on main + P347 alone. Its gate failure (Wreck 11 for 6) likely needs P347
  together with another task from that batch (R34 also changes `rider.rs`). I did not find which one,
  because it is K66's job.
- mxvatv_bike_ride's heap test (K64) was not rerun on that batch's merge. It has the same runner cause, but
  I have not shown which task changes it.
- Seven other agent-runner test files (copilot, disk, pi, plan, plansync, runner, home) fail the same way
  with and without this change. They were already failing, and I did not look into them.
- The installed runner (`uv tool`) is still 0.37.0. The fix takes effect when it is reinstalled and the run
  restarted. Restarting a live run unattended would end other slots' gates, so I did not.

## Next

- Reinstall agent-runner 0.37.1 and restart the run at a quiet moment.
- K64 and K66 have the same cause. They can probably be closed the same way once P347's batch is looked at.
