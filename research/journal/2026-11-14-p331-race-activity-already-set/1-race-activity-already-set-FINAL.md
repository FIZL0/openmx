# 1 — race activity already set (FINAL)

Task P331. 2026-11-14.

## What I did

- Nothing in code: the follow-up was already done before it was picked up. S290 (c11c53126, 2026-11-12)
  made `race_start::Start` keep the laid-out Supercross (`Start::activity`) and made `Start::race_of`
  set `Race::activity` from it.
- Checked the shell's one race build: `main.rs` builds `sim.race` with `start.race_of(&riders)`, so
  `Sim::race` runs the start timeline in play. No other place builds a `game_sim::race::Race` in the
  shell (the other `Race::new` calls in `main.rs` are the race camera's).
- Checked the note: race_start.rs's module comment already says the activity holds the rider from the
  countdown's zero to the warning window (S290, S304). The "the hold is not run" wording the brief
  asks to change is gone.
- Took nothing from opentennis: it has no start gate.

## What the original does

See S290's and S304's journals: the race activity listens to the countdown, holds the rider at its zero
and frees them in the warning window.

## How it was proved

- `race_start.rs`'s `the_sx01_prop_stands_the_gate_where_the_image_has_it` asserts that the race built
  from a laid-out start carries the activity, with the start's delay as both the activity's
  `start_delay` and the countdown's length.
- S290's `crates/game-sim/tests/race.rs` test proves a held rider does not creep and is caught by the
  gate after the warning, with the activity set as the shell sets it.
- Gate: `./kit check --all` through `./kit done`.

## Not verified / not 1:1

- The hold was not watched in the running shell on sx01 for this task. It is covered by the tests
  above, not a live ride. Not needed: no code changed here.

## Next

None.
