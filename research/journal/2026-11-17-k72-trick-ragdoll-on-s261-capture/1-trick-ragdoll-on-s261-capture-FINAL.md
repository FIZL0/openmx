# 1 — trick ragdoll on s261 capture (FINAL)

Task K72. 2026-11-17.

## What I did

- Took the brief's second way: no re-recording. S257's `a_trick_lets_the_riding_ragdoll_go…` in
  `crates/game-sim/tests/trick_ragdoll.rs` now reads S261's `context/s257/recording2-parts.txt`
  (S257's `recording2.txt` is lost, so the test skipped everywhere).
- New `on_tick` in the test:
  - keeps rows up to frame 4073 (2374 + 1700 − 1, what the poller was asked for; the gaps carried rows
    to 4105, past a backflip at 4086);
  - drops rows read off their tick. Test used is S261's: on a non-held row, each part record (`prec<i>`
    +0x60) must equal its node's world rows on the row before, bit for bit. Where not, both rows go.
  - Dropped: 2419/2420, 2749/2750, 2760/2761, 2924/2925, 2929/2930, 3235/3236, 3477/3478. These are
    S261's 7 off ticks.
- The fixed (6, 368) holds unchanged: 6 tricks, 368 posed ticks.
- The riding tick bound went from > 1000 to > 800. Each drop starts a fresh stretch with its 30-tick
  settle, so only 845 riding ticks get compared. The worst and mean bounds are unchanged.
- No port change. Nothing taken from ../opentennis (it has no trick ragdoll test).

## What the original does

Nothing new. The off-row test rests on S261's finding: each part record holds its node as the tick
before left it.

## How it was proved

- Against S261's live capture `context/s257/recording2-parts.txt`, run with
  `./kit check -- -p game-sim --test trick_ragdoll` (all 3 tests pass).
- Results:
  - 6 tricks and 368 posed ticks;
  - posed nodes within 0.0054 ft of the clip;
  - entry ticks: 5, worst 0.099 ft / 0.172 turn;
  - exits: 20 ticks, worst 0.028 ft / 0.049;
  - riding: 845 ticks, worst 0.21 ft / 0.37, mean 0.023 ft.
- The late row 2930 (S261's 2.29 ft miss) is now dropped.

## Not verified / not 1:1

- **recording2.txt was not re-recorded.** The test passes on S261's capture with its 4 gaps and 7
  off rows dropped. So about 60 rows near them are not stepped, and ~150 riding ticks are left
  uncompared because the stretches restart with a settle.
- **The off-row test can miss a row read off its tick when its nodes did not move between ticks.**
  Not seen here: every dropped row matches S261's list.
- **Held rows are not checked for being off their tick.** The held repose rewrites the records, so
  the test cannot see it there.

## Next

Nothing.
