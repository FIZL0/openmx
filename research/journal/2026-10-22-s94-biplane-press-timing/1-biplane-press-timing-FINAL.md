# 1 — biplane press timing (FINAL)

Task S94. 2026-10-22.

## What I did

- **Frame order.** `Flight::frame` used to store the pad and then step the world, so a press moved the
  controls on the tick it was made. C7's capture shows the press one tick later: at tick 23 `thr_up` is
  1 and the commands already show it, but the throttle starts to ramp only at tick 24. The original steps
  the world (preforce controls, autopilot) *before* the player's input dispatch writes the pad words. Now
  the frame searches, steps the world, and only then stores the pad, so the step reads the pad the last
  frame stored. The pad is stored only while the plane is flown. The dispatch reaches only a controlled
  actor, and an unflown plane keeps its pad zeroed. The run went from 22 to 109.
- **Full stick.** `PadFrame::stick` clamped every axis to ±127, so the human's full-down `0xff` replayed
  as `0xfe`. `Input.stick` now holds the offset modulo 256 (`0xff` wraps to -128), and `Sim::reports`
  maps it back byte for byte. A test round-trips all 256 bytes. This moved the run from 109 to 148.
  The memory note "fixture stick clamps 0xff" is now out of date.
- **C7 rows read at the wrong phase**, found once the port agreed far enough to show them:
  - Row 149 was read before the game stepped. Every body lane and the prop angle equal row 148's, and
    row 150 is two frames on. `unstepped_rows` finds such rows, and none of their lanes are checked.
    This took the run to 156.
  - Some rows (tick 157, 161, 162, …) were read after the world step but before the input step. Their
    `elev_cmd`/`ail_cmd` columns are the previous tick's: the older pad half of the row before reproduces
    them, and the row's own older half does not. `read_before_the_input_step` finds them, and only the
    `plane.pad.*` lanes are skipped on those rows. The body lanes still agree there, which is what tells
    the two phases apart.
  - I tried re-picking the pad half from the stick columns instead. That was wrong: the body parted at
    158, because the original's controls did use the older half's byte. Only the capture's pad columns
    are stale.
  - With this the run reached 261. `BIPLANE_AGREES_THROUGH` went from 22 to 261.
- **Tick 262.** `body.rate[0]` and `surfaces.elevator_command` are each 1 ulp off (the autopilot's output
  is -0.5012917 in the port and -0.5012916 in the original). This is the first tick in the PI branch where
  the pitch loop's output is unclamped; `pitch_integral` still agrees. The autopilot's own arithmetic
  (mula/madd error, mul+add output) matches and is oracle-proven, so one of its inputs is an ulp off
  within the frame. Filed as S104.
- **Consequences in other tests:**
  - `tests/flight.rs`'s lane test now expects the stored press to open the throttle on the next frame.
  - `openmx/tests/ride.rs`'s held-diagonal test pinned a fixed outcome (more than half the settled ticks
    falling). The lag moved the level plane's trim from -0.155 to -0.087, against the corner's
    elevator_command of -0.099, so the test now asserts the relation instead: the loop stays off, and
    the plane falls iff the corner's pull is weaker than the level trim.
  - `tests/platforms.rs`: the canonical inputs use random i8 sticks, -128 among them, so the seven
    vectors (HASHES, TRACE_DIGEST, POS_Y, VEL_Y, FINAL_DUMP_HEAD, FINAL_DUMP_DIGEST, FIXTURE_TEXT_DIGEST)
    were regenerated and spliced. FINAL_DUMP_LEAVES and EVERY_LEAF_HASH did not move.
- I took nothing from opentennis. Its pad path has no world-before-input split like this one.

## What the original does

Per game frame:
1. The world step runs the plane's controls from the pad words the player record stored last frame.
2. The player record's input dispatch (the plane's vtable `+0x20` input method) then writes the pad
   words at the plane's `+0x338..+0x348` for the next frame.

The stick bytes are taken whole: `0xff` is a distinct, full deflection.

## How it was proved

- A bit-exact replay of C7's capture `context/capture/acceptance/biplane.csv` (1716 ticks) over the
  original's ground (`the_biplane_acceptance_run`). Every lane agrees to the bit through tick 261, except
  the rows the two phase filters drop. Row 149 is the only unstepped row on this capture.
- The stick wrap: a unit round trip over all 256 bytes.
- `./kit check` passes, and the bike acceptance runs are unchanged (they part at tick 1 as before).

## Not verified / not 1:1

- **The frame order** is inferred from the capture (the press shows in the commands a tick before the
  controls), not traced through the original's scene update order in the decompilation.
- **The pad is stored only while the plane is flown.** This is inferred from the dispatch reaching only
  a controlled actor; no capture of an unflown plane proves it.
- **The two stale-row filters are heuristics fitted to this one capture.** They are an equal-row test,
  and an older-half-of-the-previous-row test on the pad commands. On another capture they could hide a
  real pad divergence on a row they wrongly flag.
- **The regenerated platforms vectors** were produced on x86_64 only. The cross-architecture run
  (`tests/cross/platforms.sh`) was not done.
- **The ride diagonal test** now depends on the trim. It holds for both outcomes, but the actual outcome
  of a held diagonal in the original is not compared.
- **Tick 262's ulp** in `body.rate[0]`/`elevator_command` is not explained (S104).

## Next

S104: seed the original's world step from the port's tick-261 flight, compare substep by substep, and
find the input of the pitch loop that is an ulp off.
