# 1 — flip watch out gate (FINAL)

Task S284. 2026-11-17.

## What I did

- `crates/game-sim/src/score.rs`: the backflip watch's step moved out of `Score::frame_with` into its own
  `Score::watch_flip`, called at the same place. It now steps only when the card is not out of time
  (`Score::out`) or the watch is already past 0, as the original does.
- `research/oracle.txt`: two names for the stretch, `rider_flip_watch` and `rider_flip_watch_end`.
- `crates/game-sim/tests/score.rs`: two tests.
  - `the_flip_watch_steps_like_the_original`: the oracle (below).
  - `a_flip_after_the_time_is_out_is_not_watched`: a full flip turned after the time ran out is never
    watched and banks nothing. A flip begun before the time ran out (watch already past 0) still goes
    round and banks 10 000.
- Nothing taken from ../opentennis: it has no flip watch.

## What the original does

`./kit dis 27d2a0 27d498` (the end of the scoring section in `./kit fn 0x27c890`). The watch steps only if
all three hold:
- the bike is riding (+0x748 == 0),
- it is in the air (+0x57c != 0),
- the card's flag 0x1 (out of time, S92) is clear, or the watch (+0x1400) is above 0 (signed `blez`).

Otherwise a watch at 1 or 2 drops to 0, and one at 0 or 3 is kept. The step itself (0→1, 1→2, 2→3, the
reset) was already ported right; only the gate was missing. The "a watch at 3 scores when nothing is being
scored" part runs before this stretch (0x27d228). It has no out check, and the port's matched already.

## How it was proved

- Oracle: the stretch run in the interpreter on the human's rider in
  `first_recording.p2m2_SaveState.eeMemory.bin`, from 0x27d2a0 to the hook at 0x27d498, against
  `Score::watch_flip`. 10 000 generated cases: riding or wrecked, in the air or not, out or not, every watch
  stage, and attitudes about the edges (0, −0.79 and its neighbour bit, ±0). All bit-exact. The cases reached
  every transition, and 81 of them were a start held back by the out gate.
- Mutation check: with the gate taken out again, `a_flip_after_the_time_is_out_is_not_watched` fails.

## Not verified / not 1:1

- No capture past a freestyle event's end. The oracle runs the original's own code for the whole stretch,
  so a capture would add nothing for this gate.
- The card's listener chain (card +0x1c4) is cut to empty in the oracle. Those listeners are told (6,
  &watch) on a 0→1 start and on a reset. The port has no listeners (HUD side, as S79/S92 left them).
- The front flip watch (MD16, the port's own for MX vs. ATV) still steps with no out gate. It is not in the
  original, and the task names only the backflip.
- The watch values 4+ or negative never come up in the port (it is a `u8` set only to 0..3), so the oracle
  only feeds 0..3.

## Next

None.
