# 1 — world search compares (FINAL)

Task K74. 2026-11-18.

## What I did

- **`written_differ`** (crates/game-sim/tests/mxvatv_bike_ride.rs, beside `bytes_differ`): two machines
  marked (`Ee::mark`) while their RAMs matched. It compares only the pages either one's journal has
  seen since. When they match, it marks both again. Every `whole`th call (100 here) and the first call
  run `bytes_differ` over all 32 MB instead. A write that skips `Ee::write` (straight into `ram`) stays
  a difference until both sides write the same bytes, so that whole compare still finds it, just a few
  cases late.
  - Used by `the_fade_search_is_the_originals` and `the_sweeps_global_search_is_the_originals` (both
    compares in each case). The fade test's skipped case (`b.ram.clone_from(&a.ram)`) marks both again.
  - Not K63's `split_run`: these tests keep two machines, a and b, and call the original on one and
    the port on the other. split_run runs both on one machine and needs closures for each.
- **`GatherInRam::write_query`** (gather_in_ram.rs): an entry or place equal before and after is not
  cloned to be compared. Only an unequal one runs the old "changed beyond its flags/stamp" assert. Same
  asserts and same writes: the old code wrote only when the flags or stamp differed, and in that case
  the two are unequal.
- Sampled with eu-stack (`per-test-times-and-stacks` recipe):
  - before: write_query 28%, bytes_differ 25%, `Ee::read` (the world read) 23%;
  - after: bytes_differ is gone. write_query is the derived `Collidable ==` over all 15,417 entries
    (~25%), and `GatherInRam::read` plus `Ee::read` take ~35%.
- Nothing taken from ../opentennis: its oracle has no two-machine RAM compare.

Times of the copied binaries, one test, `--test-threads 1`, run back to back on a shared machine
(load 22-112):

| test | before (user s) | after (user s) |
|---|---|---|
| fade search | 55.2, 37.0 | 46.9, 35.9 |
| sweeps' global search | 53.2, 53.3 | 39.2, 47.1 |

## What the original does

Nothing of the original changes: this is test tooling.

## How it was proved

- Both tests pass with the new compares, and so does the whole mxvatv_bike_ride binary. The last case
  of each loop (1499) is a whole compare, so the final RAMs match over all 32 MB, as before.
- `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- The speed-up is small and the timings are noisy: the machine ran at load 22-112, and the two builds
  ran at different loads. The pairs above are the like-for-like ones. No run was quiet.
- Between whole compares, a case whose port wrote `ram` straight (skipping `Ee::write`) would be blamed
  up to 99 cases late. No helper on these two paths writes `ram` straight except the fade test's own
  `clone_from`, which marks again. `heap_in_ram::resize_checked` swaps `ram` whole, but only back to an
  earlier state whose pages the journal has already seen. I checked this by reading the code, not with
  a test.
- Not done: the rest of the time is the port reading the whole world from RAM every case
  (`GatherInRam::read`, several clones of 15,417 entries) and the compare of that world. Making it
  incremental would change how the port helpers model the world, which is more than this follow-up
  asked for. Not filed.

## Next

Nothing.
