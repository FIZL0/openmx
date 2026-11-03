# 1 — the two partings were torn rows (FINAL)

Task S228b1. 2026-11-03.

## What I did

- Saved images at 137459, 137460, 137461, 137462, 137634, 137635 and 137637 (context/states/s228/,
  git-ignored). I used research/c10/play_at.sh on PCSX2 copy 3, with NominalScalar set to 0.05 for the run
  and put back to 0.25 after, then research/s228/rams.py. Each image holds the frame it was asked for.
  137636 was mistyped and not saved; it was not needed.
- Ran one port tick from 137459, 137460, 137461 and 137634, and compared the result field by field with
  the next image. The chassis matched exactly every time.
- Ran the original's `bike_ground_step` in the oracle from the parting images (137462 and 137637), with
  the port's levers written into +0x628..+0x674 (stick, flick, look angle and reach, clutch lever,
  reverse, brakes, gas, clutch, lean). The chassis velocity, centre and angular velocity, and both
  wheels' contact, compression, spin and normal, equal the port's after its whole tick, bit for bit.
  Neither the original nor the port adds anything after the ground step on these ticks.
- Lined the port's ticks up against the capture rows:

| row | velocity | centre |
|---|---|---|
| 137463 | the image's own state | the image's own state |
| 137464 | x from tick 137463, y and z from tick 137462 | none: 0.02 / 0.005 / 0.006 ft off tick 137462's |
| 137465 | tick 137464, exactly | tick 137464, exactly |
| 137638 | x from tick 137637, y and z from the image's | none: a few thousandths off the image's |
| 137639 | tick 137638, exactly | tick 137638, exactly |

  That is the mid-integrate read (memory: 2p torn rows are integrate's rebuild). The centre is the one
  rebuilt from the transform, and the velocity is part-written. `held` accepted a rebuilt centre only
  when the velocity read one whole state, so it stopped there.
- The non-zero `p1_ch_force`/`p1_ch_moment` on those rows is the same mid-tick read: the accumulators
  are caught while they fill. Rows that already held show them too (137630..137634), so they never
  pointed to a contact.

## The change (test only)

- `held` in crates/game-sim/tests/fly_and_collide.rs has a third, last-resort match, `torn`. It accepts
  a row whose velocity takes each axis from state j−1 or state j, and whose centre is within 0.05 ft of
  state j−1's on every axis. It is tried only after `reads` and `rebuilt` fail. My first version
  loosened `rebuilt` in place, and that let it choose a later state where the exact rule was right:
  136610, 138110, 138910 and 139310 fell to 86, 77, 68 and 79.
- `HELD`: 137410 and 137610 go to 100. No other pin moves.
- `DRAWN_TICKS` goes from (1675, 274) to (1793, 274). The two stretches add 118 riding ticks and no
  hard-turn ones. The lean figures are unchanged (worst to a side 0.713, root up 0.283, hard-turn ratio
  0.795), so no bound moves.
- research/s228/index.md is updated to match.

## How it was checked

- `cargo test --release -p game-sim --test fly_and_collide`: all 3 tests pass, and none skip.
- The ground step comparison above used two throwaway tests, which I removed.

## Not verified

- The 0.05 ft limit on `torn` comes from these two rows (the larger was 0.022 ft). The proper exact
  rule would hook `mat4_to_quat` at integrate+0x50 for the rebuilt centre, but the port does not keep
  that value.
- The put-backs (138610, 139110, 139710) and the preload (138810) are still pinned, as before.
