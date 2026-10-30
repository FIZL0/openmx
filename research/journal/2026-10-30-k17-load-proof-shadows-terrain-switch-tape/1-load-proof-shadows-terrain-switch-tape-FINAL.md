# 1 — load proof shadows terrain switch tape (FINAL)

Task K17 (with K18 folded in). 2026-10-30.

## What I did

Three tests that red the full gate under load. The causes were found and fixed. None of them went on the
flaky list, and none got a retry or a longer timeout.

- **track_mods `a_one_level_terrain_is_256_samples_a_side`** (`end: [512.0, 5.0, 300.0]`): a race on a
  file. `throttle_tape` wrote every test's tape to the same `md2/throttle.tape`. Three tests in the binary
  call it, and they run in parallel threads. A `std::fs::write` truncates the file before it writes, so
  one test rewriting the tape emptied it under another test's shell. That shell rode a tape of 0 frames
  and ended where it started. The fix: `throttle_tape(name, frames)`, one file per test.
- **shadows `the_bike_s_shadow_darkens_the_ground…`** ("original's shadow has no core: 2 pixels"): the
  shot is the first picture after `--frames 2` on the wall clock. How many ticks run before that picture
  (one or two), and the blend between them (`pacer.blend(moment)`), both depend on how long each pass
  took. So the Off, Original, High and Low runs show the rider in slightly different poses. A few rider
  pixels that moved then made the largest Off−Original difference, and the 0.8×deepest core shrank to
  them. The fix: `--steady-clock` on every ride in shadows.rs, the jump test included.
- **switch_tape `a_played_tape_takes_its_switches_and_replays_bit_for_bit`** (K18; the ride hashes
  differed and the plane hashes agreed): the ride was `--seconds 9`, which is wall time, for a tape
  of 240 ticks (8 s). At load ~40, ticks dropped by `CATCH_UP` meant one play reached 9 s of wall time
  after the switch back to the bike (tick 180) but before the tape's end (240). The plane was parked by
  then, so the plane hashes still agreed. Nothing in the sim reads the wall clock: a played tape's `dt`
  is the tape's own (`played_dt`), and the switches come at tick indices. Only where the ride stopped
  depended on wall time. The fix: `--frames 300 --steady-clock`, so the tape's end stops the ride, plus
  an assertion that both plays print "the tape ran out after 240 frames".

Nothing was taken from opentennis: these are this repo's own tests.

## What the original does

Not applicable: test harness only.

## How it was proved

- track_mods: I rode `md12_onelevel` on a header-only tape (0 frames) with the test's own command line.
  It said `the tape ran out after 0 frames` and `end: 512, 5, 300`, the failing value exactly.
- shadows: two runs of the Original shot with `--steady-clock` gave byte-identical PNGs (same md5). Two
  wall-clock runs of the same command differed, even on a quiet machine (load ~4).
- `./kit check -- -p openmx --test switch_tape --test shadows --test track_mods`: 20 passed.

## Not verified / not 1:1

- I did not reproduce the three failures under real load (~50). Each cause is inferred from its
  symptom: the empty-tape ride reproduces track_mods' exact end, and the wall-clock shots differ from
  run to run. The switch_tape cause is reasoned from the code (`--seconds` breaks on wall time,
  `CATCH_UP` drops ticks) and the symptom (plane hash equal, ride hash not). None was watched failing
  under load and then passing.
- The `./kit done` gate ran with `OPENMX_WORLDS` set to an empty folder, so disc_worlds skipped its
  comparison against the installed `world_mxu_free`. On the first gate run that comparison failed with
  `mod.json` differing. The cause is outside this branch: the main checkout's
  `mods/tracks/world_mxu_free/mod.json` was rewritten at 12:15 with a `"category": "World"` line,
  which MD66 (mod categories, in flight in s2) adds. No code on main writes that line, so main's
  stitcher no longer matches the installed copy. This branch touches neither the stitcher nor
  disc_worlds. The comparison goes green again when MD66 lands with the stitcher writing the category
  (MD66's own gate runs it), or when the installed copy is rebuilt.
- I did not check for the same shared-tape pattern in other test files: this task covers the three named
  tests only.

## Next

None.
