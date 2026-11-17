# 1 — atv gather palm rows (FINAL)

Task K61. 2026-11-17.

## What I did

- **Not a load race.** R17's merge gate (06:30) ran `atv_gather` red with "every palm row was asked: 0 vs 6"
  (context/notes/merge-gate.log). The runner called it a load flake ("failed, then passed alone").
- What really changed: MD117 (slot 4, 06:07–06:26) put `mrider.phy` into the shared
  `context/mxvatv-ps2/md37a/` at 06:08:59. Before that `data_of` skipped every test for the missing file. After
  it, `the_quads_nearby_list_is_the_originals` ran for the first time since C28 (11-05) re-made the md37a
  captures. C28's `race-jump.csv` runs frames 21459–22057, so the pinned palm rows (vsync 35805, 35851–35863)
  are in no capture: the test is red every time, alone too.
- MD117's branch (a5bf3ab52) already moves the rows to C28's frames 21535–21741, but it has not merged. I applied
  the same hunk to atv_gather.rs (identical change, so the two merges agree).
- Added a check before the palm count: if a palm row is in no capture, it fails saying race-jump.csv was re-made,
  instead of the bare "0 vs 6".
- Nothing from ../opentennis applies (a test of this port's own captures).

## What the original does

Unchanged: the test still compares the original's gather (oracle on MD37a's image) with the port's, row for row.

## How it was proved

- `./kit check -- -p game-sim --test atv_gather` on the branch's file before the fix: TESTS FAILED, the same
  "0 palm rows … left: 0 right: 6" as the merge gate, alone, with no load.
- After the fix: TESTS PASSED, 6 tests (twice).
- The six new rows are each in race-jump.csv once (grep).

## Not verified / not 1:1

- Why the runner's "alone" rerun passed: not found. The gate logs are gone; on this branch the old file is red alone
  every time. A guess, unproven: the rerun built from a tree that already had MD117's hunk.
- The new "no row" message was not seen red by a mutation (I did not run a mutant). It is a plain `assert!` over
  `d.points`.
- That both palms are in the list on frames 21535–21741 comes from MD117's work; I only saw the test pass on them.

## Next

None. The lesson for the runner: a "load" verdict can be a shared `context/` file another slot added mid-gate,
which un-skips a test.
