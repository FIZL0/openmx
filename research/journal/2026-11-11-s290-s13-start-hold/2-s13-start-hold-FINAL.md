# 2 — s13 start hold, applied (FINAL)

Task S290, reopened after S304. 2026-11-12.

## What I did

S304 already did what part 1's "Next" asked of the sim: `Sim::race_frame` applies the timeline's
`StartStep` to every rider's chassis, after the watch's pushes and before the drop's release, and
its test (`the_start_timeline_holds_the_rider_until_the_warning`) shows the hold from tick 1 to the
warning. Two things were left:

- **The shell's race had no listener.** `crates/openmx/src/race_start.rs`'s `Start::race` built the
  `Race` without an activity, so in the app the countdown only counted and the gate watch was the only
  thing holding the rider. `Start` now keeps the race's `Supercross` (after the reset and the lane
  shuffle) and `Start::race` sets it as the countdown's listener. Its sx01 test checks the race
  listens, with the countdown's length the activity's start delay.
- **The held rider against the gate** (`crates/game-sim/tests/race.rs`,
  `a_held_rider_cannot_creep_until_the_warning_and_is_caught_in_it`). sx01's 1.54 s start call, the
  gate three feet ahead, the gas held from tick 1. Until the warning the chassis reads free 0,1,0 and
  the bike's x and z do not move at all. It is let go inside warn_on..warn_off. It then rolls into the
  closed gate and the watch catches it before the drop, with a 1–2 s penalty.

The two tests part 1 flagged were left as they are. They build a race with no activity, so no
timeline runs: the 30 s one tests the watch alone, and the hand-set hold in the drop test is still
what the drop releases.

Proof: `./kit check -- -p game-sim --test race -- a_held_rider timeline_holds` and
`./kit check -- -p openmx --bin openmx -- race_start` pass. A mutant `hold` (free 1,1,1) fails the
new test.

Nothing taken from opentennis: it has no races.

## Not verified / not 1:1

- **No live capture of a start** going through the hold, the warning and a gate catch. The proofs
  are S13f's oracled timeline, S304's oracled cadence, part 1's sx01 image, and this port-side test.
  The penalty after the warning is only checked against the port's own gate watch (S13b).
- **The shell run was not ridden on screen.** The race_start unit test checks the listener is set,
  but no `--race` ride was watched.
- **Which riders are items when there are several, and when there are AI riders** is not proven.
  The image has one rider, and the port holds every rider's chassis.
- **A computer rider's start message (bike_race_start) draws three or four times** (S12). This is not
  ported: the port's race riders are all players, so there is no AI rider to send it to.
- **The drop's item word writes** (+0xc = 1, +0x10 = 0) are still not ported (S13f's note).
- **The intro**: the shell counts it as seen (`intro_seen = 1`), so the timeline's intro end is the
  first tick's and no intro camera runs (the port passes `intro_camera` false).
