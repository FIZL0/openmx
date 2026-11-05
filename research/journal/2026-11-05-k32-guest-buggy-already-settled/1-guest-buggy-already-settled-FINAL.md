# 1 — guest buggy already settled (FINAL)

Task K32. 2026-11-05.

## What I did

Nothing in the code: K27 had already fixed it (986eddad, 2026-11-05 02:51, after the red this task was
filed for). The red happened in K25's gate on 2026-11-04 at load ~50: `the two machines drove the buggy
apart`, 475.7 vs 474.6 ft. That is the `car: buggy N ft from where it was taken` line, which the shell
then printed from the drawn frame. On the machine whose seat did not switch, that frame coasts the car on
predicted pads. K27's `Carried`/`CarLog` (`crates/openmx/src/online.rs`) prints the leave line from the
tick it happened on, once `Netplay::settled` has passed it. So both machines' lines come from one settled
tick, which is what this task asked for. The test's other comparison, `state before tick 300`, also reads
settled ticks (`online.rs` `settled` hash). K29 then put the `--online-script` words on session ticks. That
leaves nothing in `both_agree`'s distance or state checks that reads a wall-clock moment.

This test was never in `gate.toml [flaky]`, and nothing changes there.

opentennis: nothing to take; a test robustness check only.

## What the original does

Nothing to port: the original has no online play.

## How it was proved

At HEAD 0bd68655, all through `./kit check -- -p openmx --test online_car`:
- `two_online_shells_see_the_guest` alone 4× back to back at load 33-37 on 32 cores: 4/4 green.
- then the three `two_online_shells` tests together (six shells at once) 4× at load 36-48, beside other
  slots' gates: 12/12 green.
The test this task names: 8/8.

## Not verified / not 1:1

- I did not reproduce the old red at the pre-K27 commit, so "K27 fixed it" rests on the mechanism (the
  same line, read off a predicted frame) and on 8 green runs, not on a red-then-green pair. Reason: the
  red needs a late pad at the switch-back, which load gives only sometimes, and K27's journal had the same
  gap for its sibling.
- The loads above were sampled at the start of each run, and the highest seen was 48, just under the
  failing run's ~50.
- The `vehicle: <machine>` lines are still printed from the drawn frame (K27's note). This test compares
  them only as a multiset per machine, so it does not depend on when they print.

## Next

-
