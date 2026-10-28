# 1 — V35's body test over C8's capture

Task V81. 2026-10-28.

## What I did

- **Moved V35's per-row checks into a helper.** `v35_body_over` in `crates/openmx/tests/ride.rs`
  holds what used to be the inside of `the_body_plays_the_originals_backflip_through_recording2`.
  For every riding row it asserts that the request is the bike's words: backflip = turning flag,
  airborne = airborne, gesture = the trick id's slot. It then hands those words to the port's rider,
  runs `body_frame`, and records each tick where (want, state) differs from the original's. It
  returns the ticks checked, the original's state per tick, and those misses. The V35 test now calls
  the helper, and its assertions are the same as before.
- **New test `the_body_plays_the_originals_uncut_backflip_and_no_hander_through_c8`.** It runs the
  helper over `context/capture/c8/run1.csv` (C8's run 1) and checks:
  - all 440 rows are riding ticks, with no wreck;
  - the original's backflip state (38) covers exactly 2513..=2579;
  - state 4 (the no-hander, trick 4) covers exactly 2651..=2696;
  - every other tick is riding (30) or the airborne pose (35);
  - the port's body matches the original's want and state on all 440 ticks.

  It passes. To show it is not a silent skip, I broke one assertion on purpose (440 → 441) and it
  failed.
- **Context.** I copied C8's capture and tape from s1 into this slot's `context/capture/c8` and
  `context/recordings/c8`, and V35's `flips.csv` from s3 into `context/capture/v35`, so both tests
  run here. Before that copy, the V35 test had been skipping in s2.

Nothing from opentennis applies, because it has no tricks.

## How it was proved

The proof is C8's PINE capture from the original, through the test above. The port's body
matched the original's want and state on every tick, including the uncut 67-tick backflip and the
46-tick gesture state. Those are the two cases V35's capture could not show: V35's backflip was cut
by a Taunt, and its ride had no gesture played to its end.

## Not verified / not 1:1

- **Run 2 is not tested.** C8's `run2.csv` has phase-torn rows. At 2595 the bike is down while the
  body still shows 35. At 2650 the trick id is 4 while the request's gesture is still 24. These are
  rows read between the bike's update and the body's (memory: capture rows read at three phases),
  and they fail V35's same-row request check. Its event ticks agree with run 1 to ±1 (backflip
  2512..2579, gesture 2651..2697).
- **The landed no-hander (state 39) is not covered.** C8's tape lets go of the trick in the air, so
  the original never raised it (C8's own Not verified).
- **The body is fed the original's bike words, not the port's ride.** This proves the body's mapping
  and state machine. It does not prove the bike reaches those words on the same ticks: the port's
  ride was not driven by C8's tape.
- **The clips are V35's stand-ins.** Each state is bound to a one-second, non-looping clip, so state
  timing that depends on real clip lengths is not exercised, as in V35.
