# 1 — covered by F21

Task F19. 2026-11-17.

## What I did

- Nothing new in code. F21 (2b0ca1d30) did this task's work: a prop's rays now strike the
  bodies of its own collection, not only other collections'.
- F21's journal (2026-11-17-f21-pins-strike-pins) says so: "F19 in plan/TODO.md is the same work".
- Checked on this branch: `./kit check -- -p game-sim --test prop_props` passes (5 tests). That
  includes F21's `a_pin_strikes_a_pin_of_its_own_collection` and `a_pin_on_a_pin_runs_on`. Their
  states (context/f21, context/states/f21-lean-209..214) are in the shared context, so they ran.
- Nothing taken from ../opentennis: there was no code to write.

## Not verified / not 1:1

- Nothing beyond F21's own list (stub wake, not a bike knock; one captured pair, free11 pins;
  no jointed sibling captured). See F21's journal for the reasons. No new proof was made here.
