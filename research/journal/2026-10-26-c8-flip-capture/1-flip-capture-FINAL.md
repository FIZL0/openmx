# 1 — flip capture (FINAL)

Task C8. 2026-10-26.

## What I did

- **Made the tape from recording2 rather than from nothing.** recording2 is a free09 ride with a
  human backflip, flip 1, that fires at tick 2506. V35 found a Taunt cutting that backflip's clip
  short at 2544. The same ride also has a cancan on its next jump, at 2652. So I patched the human's
  tape, using `capture/make_tape.py`:
  1. The Taunt is gone. Circle is lifted over fields 335..349, and the stick is centred there.
  2. The cancan is now a no-hander. Over fields 549..600, Circle with the stick full left becomes
     Triangle (trick button 2) with the stick full up for the first 8 fields. That gesture is slot 4,
     `nohandedlander`.
  3. The tape ends at field 860 (about tick 2807), and its header count is rewritten to match.

  Up to field 335 the tape is the human's, so the run-up and the flip's firing are the original's own.
  A hand-generated tape would also have hit S100's two-fields-a-tick problem. recording2 is two
  fields a tick by itself, so that problem does not arise here.
- **Files.** The tape is `context/recordings/c8/c8.p2m2`, with recording2's own state as
  `c8.p2m2_SaveState.p2s` and the source `recording2.p2m2` beside them. It sits in a subfolder so that
  replay.rs does not pick it up (see the memory). All of it is git-ignored and in slot s1.
- **The capture.**
  - The spec is `capture/c8.toml`. It is V35's `flips.toml` plus several fields from the acceptance
    spec: dt, `ctl.held`, bike_vel, bike_xform, on_ground, the trick group/dir/safe/elapsed and S31's
    stick. It also adds the rider's no-hander word `+0xa44`.
  - `capture/play.sh` is V35's script (that is, S109's) pointed at this spec.
  - Run 1 is `context/capture/c8/run1.csv`: frame_counter 2374..2813, 440 rows, 0 torn.
  - Run 2 is `run2.csv` (2374..2816, 440 rows, 0 torn), filmed as `run2.mp4` (17.9 s).
  - The two failed tries are kept beside them as `try1-taunt-lifted.csv` and `try2-held-to-700.csv`.
- **A film strip**, `context/shots/c8/strip.jpg` (taken with `capture/frames.sh`, at 1, 3, …, 17 s).
  It shows the original's HUD calling "Backflip 10000" at about 7 s and "No Hander 3830" at about 11 s.
- Filed **V77**: run V35's body checks over this capture.

Nothing from opentennis applies; it has no tricks.

## What the original does (this ride, run 1; ±1 tick between runs is the row phase)

| Tick | What happens |
|---|---|
| 2506 | backflip fires (`flip_active`), the same tick as the human's flip 1 (S31) |
| 2511 | airborne, state 35 |
| 2512/2513 | turning, request backflip up, state 38 `backflip` |
| 2579 | last tick turning; state 38 runs **uncut for 67–68 ticks** (V35's run: 31, cut at 2544) |
| 2580 | turning over (the right way up), state 35 |
| 2595/2596 | lands, state 30, no wreck |
| 2650/2651 | the gesture: group 2, dir up, trick id 4, state 4, request gesture 4 |
| 2673 | the trick is safe |
| 2696/2697 | the trick ends, state 35 |
| 2718 | lands, state 30, no wreck; it rides on to the capture's end |

- **The tries, and what they teach.**
  - **Try 1: Circle lifted, stick left as it was.** The flip under-rotated, landed still turning at
    2594 and wrecked with reason 1. The cause is the port's `air::backflip`. The flip torque moves
    only while the trick group is 0. A stick pushed against the flip bleeds the torque at
    `rate_decrease`. The human's forward push under the Taunt therefore did nothing, and without the
    Taunt it stopped the flip. Centring the stick holds the torque still, as the Taunt did. With that
    change the flip is the human's tick for tick (rotation over at 2580 in both).
  - **Try 2: Triangle held to field 700, past the landing.** The no-hander was held on the ground
    (`+0xa44` = 1 and request `+0x100` = 1 at 2718/2719), but it landed rolled and nose-high
    (up.y 0.66, vy −35) and wrecked with reason 5 (Flat). A held trick halves the stick, so the
    human's late-air corrections came out weaker.
  - **Final tape:** Triangle is released at field 600, where the human let go of Circle.

## How it was proved

- **A PINE recording.** The rows above come from run 1.
- **Repeat run.** Run 2 has the same events on the same ticks, ±1 row. 367 of run 2's 440 bike
  positions are bit-equal to one of run 1's rows. The other 73 lie between two of run 1's
  consecutive positions: they were read between the tick's substeps, so they show the same ride at a
  different phase.
- **By eye.** The film's HUD names both moves ("Backflip", "No Hander") and shows clean landings.

## Not verified / not 1:1

- **The no-hander is not landed no-handed.** The final tape lets go in the air, so the ground
  no-hander (`+0xa44`, request `+0x100`, body state 39 `nohandedlander`) is never raised in run 1/2.
  It was raised for one tick in try 2, which wrecked (reason 5). A held landing needs different
  air steering, and I did not search for it.
- **The ride is the human's only up to field 335.** After that it is this tape's ride. It is the
  original's own, but no human rode it. Also, centring the stick under the Taunt matches the human's
  flip only because the bike gave the stick no say while turning. That reading comes from the port's
  `air::backflip`. On the original it is confirmed only by the outcome: the rotation ends on the same
  tick (2580) and lands on the same tick.
- **The field↔tick mapping is approximate.** I took it from two trick presses and S31's stick
  crossing (field ≈ 2 × (tick − 2377) for a press). The tape's edits are placed on whole human
  presses, so nothing depends on it being exact.
- **73 rows per run are read between substeps** (see above). A consumer has to compare rows at
  every phase (memory: capture rows read at three phases).
- **The film's tick offset was not measured.** The 7 s and 11 s are read off the HUD, not computed
  from frame_counter.
- **No port test uses the capture yet.** That is V77. This task was the capture.

## Next

Done. V77 adds this capture's ticks to V35's body test.
