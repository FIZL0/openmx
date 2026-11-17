# 1 — measure the wreck check (FINAL)

Task F66. 2026-11-17. Closes the two open proofs F31b5s left.

## What I did

- Ran F31b5s's oracle and the two pure replays with a temporary panic after their tallies (so `./kit
  check` prints them), then once more with `"wreck check"` dropped from `replay_in_pure_rust`'s list.
  The test file is back as it was; no code change lands.
- **The oracle really runs.** `the_out_of_bounds_check_is_the_originals`: 12,000 cases, 0 differ.
  Path tally: inside 4,804 · not held 1,159 · out low x 2,730 · out high x 1,854 · out low z 717 ·
  out high z 736 · thrown from the middle 336.
- **The switch's saving, measured.** Interpreter instructions in the second run's frames:

  | replay | rows | switch off | switch on |
  |---|---|---|---|
  | ride-2 | 459 | 20,655 | 0 |
  | flip-1 | 465 | 20,925 | 0 |

  Off, all of it is in `the wreck hands on: OutOfBounds` (under `the tail: out WreckTest`): exactly 45
  instructions a frame, the inside path (C15 never leaves the map). The ~21k F31b5 counted is the out-of-
  bounds check and nothing else. The other census parts (the emitter's and effects' counts, all ported)
  are the same either way; the rows where the RAM parts from the handing run's (182 / 198, the known
  HUD/race words) are the same either way too.
- Times under the slots' load: ride-2 81 s / flip-1 91 s with the oracle beside them (switch on), 50 s /
  58 s alone (switch off). Not comparable runs, so no speed claim.

## How it was proved

Function oracle over 12,000 generated inputs (`crates/game-sim/tests/mxvatv_bike_ride.rs`,
`the_out_of_bounds_check_is_the_originals`), its tally shown; replay harness on C15's ride-2 and flip-1
(`ride_replays_in_pure_rust`, `flip_replays_in_pure_rust`) with the interpreter's census.

## Not verified / not 1:1

- The replays only take the check's inside path (C15 stays on the map); the throw paths are proved by the
  oracle alone, not by a capture that leaves the map.
- The switch-off runs panicked before the replays' own asserts, so their per-row wreck answers were not
  checked there; the switch-on runs are the gate's tests and pass.
- `biplane::Area::outside_mxvatv`'s `W - m < x` vs. the original's `!(x <= W - m)` (NaN only), noted
  by F31b5s, is still not changed: not this task's code.

## Next

none
