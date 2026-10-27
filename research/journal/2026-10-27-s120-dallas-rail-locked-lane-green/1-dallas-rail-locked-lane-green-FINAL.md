# 1 — dallas rail locked lane green (FINAL)

Task S120. 2026-10-27.

## What I did

- Nothing in code: the red was already fixed on main by S107 (merge b68f3977, 2026-10-25). This
  entry records what changed and shows the test now runs green on the data that showed the red.
- What happened: from 1cdaee5a (the batch merge that brought in S106's braking speed source), the S89
  rides' front locked lane parted one tick earlier (agreeing through 278, not 279). At the time the
  rides used race250a. S89's states, though, hold race250p's grip table, so the rides were on the
  wrong bike. S107 switched them to race250p with the levers seeded from the state and re-pinned
  `S89_LANES_AGREE_THROUGH`'s locked lanes to front 280 and rear 276
  (`crates/game-sim/tests/replay.rs:7382`). The front lane now agrees further than the old 279 pin.
  The race250a pin is moot, so I did not bisect the 279→278 step inside 1cdaee5a.
- Run on HEAD b314e1a2 in s2:
  `./kit check -- -p game-sim --test replay -- --nocapture the_dallas_rail` → `TESTS PASSED: 1`. It
  really ran and did not skip: check.log shows the along/across ride output, with the front locked lane
  parting at tick 281 and the rear at 277, both inside the pins.
- s2's S89 data is byte-identical (sha1) to s3's: `capture/s89/{along,across}.csv{,.json}` and
  `states/s89-{along,across}.eeMemory.bin`. With the same code and the same data, s3 gets the same result.

## What the original does

Unchanged by this task. The slip and lock lanes are compared against S89's PINE captures of free06's
rail rides (journal 2026-10-21-s89-dallas-rail-grip).

## How it was proved

The bit-exact replay harness: `the_dallas_rail_rides_replay_from_their_save_states` against S89's two
captures (along 600 rows, across) from their save states, with s3's data byte for byte.

## Not verified / not 1:1

- I did not run the gate inside s3 itself. Another slot's worktree is not mine to write to, and a run
  there writes its target/ and context/notes. Equivalence rests on the same commit plus sha1-identical
  data.
- I did not isolate which part of 1cdaee5a moved the old race250a lane from 279 to 278. Since S107 the
  test no longer rides race250a, so that pin cannot come back. Whether S106's braking speed source is
  right is S106's proof, not this task's.
- The locked lanes still part at front 281 and rear 277, and the chassis parts at tick 1. Those are
  S107/S123's open work (ratchets, not regressions).

## Next

-
