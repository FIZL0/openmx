# 1 — six piece floor (FINAL)

Task K48. 2026-11-13.

## What I did

- Nothing in the code: by the time this run started, B36 (344ca6cd9) had already fixed it. B36 changed
  `all_six_pieces_are_mx_unleasheds` and `all_six_pieces_with_the_tire_step_are_mx_unleasheds` (and
  F31b5l's `all_eight_pieces_are_mx_unleasheds`) to use `pieces_over(6000, …)`. That is twice the draws, so
  "preload: fired, degenerate" should come in around 34 times, well over the floor of 20, where 3,000
  frames gave only 17–18. K49 later split the file into `tests/mxvatv_car_frame/pieces.rs`, which is why
  the brief listed `mxvatv_car_frame.rs` as missing.
- Ran `./kit check -- -p game-sim --test mxvatv_car_frame -- all_six all_eight` on task/K48 (base
  3ab24eeb5): `TESTS PASSED: 3 tests`.
- I gave the task more draws rather than new seeds: a seed chosen to clear the floor breaks again the
  next time the generators' draw order changes (that is exactly how MD124 + MD126 broke this one).

## What the original does

Unchanged: this task only changes test coverage.

## How it was proved

The three runs pass (see above). The full gate runs in `./kit done`.

## Not verified / not 1:1

- I did not read the per-branch counts at 6,000 frames: `./kit check` prints only one line when tests
  pass. The margin over the floor (~34 against 20) is estimated from B36's figure of ~17 per 3,000
  frames, not read from this run.

## Next

none
