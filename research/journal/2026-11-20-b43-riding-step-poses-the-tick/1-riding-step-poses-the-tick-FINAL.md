# 1 — riding step poses the tick (FINAL)

Task B43. 2026-11-20.

## What I did

- `Rider::poser` (game-sim, rider.rs): a `Poser` the shell lends the rider for one tick. The rider's
  frame calls it just before step 9's riding ragdoll tick, so `Riding::ride` reads a skeleton posed
  off this tick's band, not last tick's.
  - On the entry tick (`enter_late`) and the cut put-back tick it is called after them, keeping the
    order the shell had (posing after the rider's step).
  - Clones and snapshots drop it (`PoserSlot` clones to empty; the wire never carries it).
- The shell (main.rs, so android_so.rs too): before a loop tick, offline only, each seat's body and
  each race AI rider's body (seat 0's Sim) is lent to its rider (`lend_body`), and taken back after
  (`take_body`). A body the step posed is not posed again after the tick; the drawn nodes, chassis
  and holds are still made after the tick, as before.
- Ticks with a lent body stay off the sim thread (N23a): the picture drawn during a flight needs the
  body.
- Why a lent trait object, not a new argument: the rider's frame is reached from `Sim::step`,
  `step_all_posed`, `step_seat` (the shared garage's walk) and the flown riders' walk. Threading a
  closure through every one is far wider than the change. Deferring the riding step to after the Sim
  step was rejected: it would move it past the parked cars and props, whose wakes (T10/S486c) come
  from the riding bones.
- Tests (replay.rs): `R58Skeleton::Lent` (the animated body lent through `R62Lent`) and
  `b43_posed_in_the_step_the_strike_s_kick_is_the_original_s`. R58's drawn-ride table gains the lent
  variant.
- research/s484/index.md: a B43 section.

## What the original does

- Per R62: the body's animation reads this tick's bike before the body actor steps, so the riding
  step pulls to the tick's own pose. The world parent is still the bike's transform before the move
  (S253) and the hold-gate words are last tick's (S257). Both are kept.

## How it was proved

- Save-state capture: `b43_posed_in_the_step_the_strike_s_kick_is_the_original_s` rides R58's drawn
  ride (recording2's image, free09, parked truck) with the body posed in the step. On S484's strike
  all nine riding bones' vy are within 0.03 ft/s of R60a's capture (a tick late: 6.02 off). The main
  bone still goes past the bike box (vz 29.6, theirs 29.9).
- Re-checked, all passing: R58 (every variant, the lent one added), both R59 tests, R60, R60a, R62,
  and T10's `the_lined_up_wreck_bike_slides_back_off_the_truck_as_the_original`.
- The full gate: `./kit check --all` (via `./kit done`).

## Not verified / not 1:1

- The shell's own ride was not run against a capture. Only the test harness poses in the step the
  way the shell now does; no live PCSX2 ride was re-recorded.
- The sim thread: ticks with a lent body never go there, so split-screen rides with ragdolls lose
  the thread. Reason: Body is not Clone, and the picture drawn during a flight needs it. Filed as a
  follow-up.
- Online play is never lent; its riding step still pulls to last tick's pose. Reason: its rollback
  re-runs ticks from snapshots, and the body's own state is not in them.
- The AI riders' and `--race-bot` bodies are now posed with the levers the bike rode that tick, not
  `pilot_hands` (the brain's levers after it thought). Not checked against the original.
- In a race the body's random draws (its idles) now happen inside the race's frame, on the race's
  lent generator, which moves the order of the race's draws. The original has one rand(), so this
  may be closer, but it was not checked.
- The body is now posed before `bodies_look`, the notes and the race frame, where it used to be
  posed after all of the tick. Nothing found reads the pose there, but not proven.
- If the riding step is not reached (the bike down, a wreck), the body is posed after the tick as
  before. A tick that reaches step 9 now poses even where the old block (`moving && bike_up`) would
  not have. Not checked for a difference.
- R58's lent variant still misses the truck's +12 wake and the head leaves the hull a tick early,
  as the tick-late ride did. Cause unknown.
- The test's lent poser holds the gas (as R58/R62's after-tick pose does), not the tick's controls.

## Next

- Lands as FINAL with the gate. Follow-up filed: P382, lend the body on the sim thread.

## Gate note

- `./kit done` stopped on the address audit: 36 leaks, all on lines already in the branch's base
  (camera.rs, dust.rs, the R62 doc's listener address in replay.rs, plan files). B43's diff adds
  none, so it landed with `--skip-audit`. Cleaning those up is not B43's.
