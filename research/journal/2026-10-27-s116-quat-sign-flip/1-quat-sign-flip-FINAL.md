# 1 — quat sign flip (FINAL)

Task S116. 2026-10-27.

## What I did

- **Replayed S58's four save-state cases on HEAD** (`the_slip_cases_replay_from_the_save_state`, with
  `S58_TRACE=turn:126:133:chassis.quat[1],chassis.quat[3]`). The turn's quaternion goes from
  `(…, 0.6625397, …, −0.65109134)` at 129 to `(…, −0.64202243, …, 0.67053074)` at 130 in the original,
  and the port matches every word. `quat_hist` keeps the old sign at 130 and changes at 131, in both.
  The turn's first parting lane is now `chassis.ang_hist.y` at **173**, and the path's is `origin[1]` at
  178. S58 had them at 130 and 83.
- **Found that the flip is already ported.** It is S67's copy-back (ride.rs, step 5 of `frame_bumped`:
  `chassis.quat = math::mat4_to_quat(&out.chassis)`). S67 landed after S58 filed this task and closed it
  without saying so.
- **Raised `S58_CASES`** (crates/game-sim/tests/replay.rs). Path / slip pins went from 82/146, 96/126,
  105/130, 82/123 to 177/MAX, 123/MAX, 105/MAX, MAX/MAX. `u64::MAX` means every tick of the capture. The
  slip flags of all four cases agree on every tick, and the traverse agrees on all 112 lanes on every
  tick. The assert became `at > through || at == u64::MAX`, because `>` alone cannot ask for every tick.
- **Added `S116_FLIP`** with a check in the turn case. The capture's `ch_quat.w` changes sign between 129
  and 130, its `ch_quat_hist.w` between 130 and 131, and no `chassis.quat*` lane parts before 132. This
  way the ratchet still proves the turn crosses the flip, even if the capture gets re-recorded.
- **Filed S177**: the turn's parting at 173 (angular history y, 18.0208 in the port vs 18.0282).

## What the original does

The vehicle update's state pack snapshots the node's world rows as a quaternion, through `mat4_to_quat`
(`./kit fn 0x0013f7b0`). The stunt clocks then copy it straight into the chassis quaternion, after the
integrate (research/oracle.txt, `stunt_state_pack`). `mat4_to_quat` picks the largest of the four
candidates `0.25·(1 ± m00 ± m11 ± m22)` and makes that component positive. Through tick 129 of the turn
the largest component is `y` (|y| 0.6625 > |w| 0.6511), so `w` comes out negative. At 130 it is `w`
(0.6705 > |y| 0.6420), so `w` is positive and the other three change sign. The next tick's integrate
works from the flipped quaternion. Its `quat_deriv` therefore has the new sign, and the two-step
integrator mixes it with the previous tick's derivative, which has the old sign. That mix is the heading
jump S58 saw, and the port reproduces it.

## How it was proved

- **Save-state replay, bit-exact**: `the_slip_cases_replay_from_the_save_state` on
  context/capture/s58-slip/turn.csv. Every lane (quaternion, its history, both wheels' local velocity,
  slip flags, path) agrees through tick 172, so the flip and the tick after it are covered.
- **The ratchet bites**: with the turn's path pin set to 178 the test fails ("the path parts at 178").
- **The copy-back is the cause**: I commented out the copy-back line (a temporary probe, reverted) and
  the turn parted at tick 2 on `ang_hist.x`, with the path at 79.

## Not verified / not 1:1

- **I did not Ee-step the frame**, as the brief suggested. The replay already matches the original's
  quaternion and history words on ticks 129–131, and the copy-back is the only write the port makes
  after the integrate. A second method had nothing left to settle.
- **The turn still parts at 173** (angular history y, about 0.04%), and its path at 178. The cause is
  unknown and filed as S177.
- **The launch's and the brake's paths** still part at the landing off the jump face (124 and 106,
  pins 123/105). That was S67's finding. Their slip flags agree on every tick anyway.
- **I did not check which commit between S58 and S67 raised the cases.** Without the copy-back, the turn
  parts at tick 2, so the copy-back is necessary. I did not test whether it is sufficient on S58's own
  commit.

## Next

Done here. S177 is the turn's next parting.
