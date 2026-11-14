# 1 — nose first landing (FINAL)

Task S325. 2026-11-13.

## What I did

- Read the phone log (0.13.0 build 4179, crf450_factoryconnection on pro250p, world_mxu_free). Air
  Wrecks went off from the pause at ride second 33 (log line 4137) and back on at second 113 (4551).
  The only hard slowdown in that stretch is at seconds 40–41: 46.8 → 16.9 mph, with no wreck line.
  That fits a big landing ridden out with the switch off. The log has no landing lines, so which jump
  it was stays unknown.
- Desktop first. A probe test (since deleted) dropped the stand-in bike (the shipped `[Wrecks]`
  limits) and pro250p off the disc 6 ft over the flat floor. It was pitched nose-down 20–125° and
  falling at 20/50/90 ft/s, with Air Wrecks on and off. At touchdown it printed the g peak against
  the limits, the euler pitch, and which wheels touched. pro250p's limits: vertical 66.0, fore-aft
  18.33, ground pitch 70°.
  - The front wheel touches first at every nose-down angle. The fore-aft load at a front-wheel touch
    stays small (|g.z| ≤ 6.6): the landing loads the chassis' vertical lane. So a front-first
    landing can only be judged by the vertical g (Flat, reason 5) or by the ground pitch (Front,
    reason 1), never by FrontWreckSpeedLimit.
  - Vertical g ≈ the fall speed. 50 ft/s (a 40 ft fall) gives 47–52 and rides out. 90 ft/s gives
    87–89 and is reason 5. Air Wrecks makes no difference to either.
  - **The finding:** past straight down, the euler pitch folds back. 115° nose-down reads −65° in
    the air and −55° at the touch. With Air Wrecks on, the airborne clause throws such a bike on its
    first tick (reason 8, NoseDive: past 90° the euler also reads the roll as half a turn, so
    `rolled` is set). With it off, the clause is skipped, the touch reads well inside the ground's
    limit, and the bike lands on its front wheel and rides on. Between the airborne limit and 90°
    (pro250p: 85–90°), off still wrecks at the touch on the ground pitch (90° reads −79°, reason 1).
- The original, in PCSX2 copy 3. recording2's state was teleported to free09's farmyard, then
  thrown up with a forward pitch rate once airborne (S138's `research/s138_nose.py`). Recorded with
  S138's `hits.toml` through `research/s138_hits.py`; script: `research/s325/drop.sh`. The air
  control damps a poked pitch rate to nothing within a few ticks and then holds the nose there for
  the whole fall, so the rate sets the landing angle. Four drops, idle pad:

  | capture | drop | original |
  |---|---|---|
  | s325-nose22 | 22° nose down, 53 ft/s down (50 ft) | rides out |
  | s325-nose76 | 76° nose down, 53 ft/s down | reason 1 at 2615 (touchdown) |
  | s325-dive | pitched past 85° on the way up | reason 8 at 2516 (first tick, in the air) |
  | s325-flat | level, ~75 ft/s down (110 ft) | reason 5 at 2658 (touchdown) |

  A first throw at rate 1.0 (15° nose, 55 ft/s down) also rode out.
- Test: `crates/game-sim/tests/replay.rs` `high_nose_first_drops_wreck_as_the_original_does`. The
  port rides each capture from the original's save state, in free09's world with its props and the
  bike's body rays, with Air Wrecks on and off. It holds the on verdict to the capture's (tick ±1,
  reason). Off, it holds the three landings to the same verdict as on, and the dive to no wreck in
  the air. It prints the touchdown's g and pitch beside the limits.
- Test: `crates/game-sim/tests/air_wrecks.rs` `off_rides_out_a_nose_landing_past_straight_down`.
  It needs no disc. Stand-in bike, 50 ft/s down. At 115° nose-down: on is reason 8 in the air on the
  first tick; off lands and rides out. At 65°: on and off agree, reason 1 at the touch.
- So the port does not ride out what the original wrecks. The ride-out is Air Wrecks off, the
  brief's second branch. The switch's help (Settings and the pause share `AIR_WRECKS_HELP` in
  `crates/openmx/src/menu_pages.rs`) now says that off, a bike nosed down past straight down can
  come down on its front wheel and ride on, where the original throws the rider off before it
  touches. `Sense::air_wrecks`' doc (`crates/game-sim/src/state.rs`) says why: the folded euler
  pitch.
- Filed P345 (low): whether Air Wrecks off should still judge such a landing. That would be a
  port's-own rule, and it is the user's call.

## What the original does

- `bike_wreck_test` (`./kit fn 0x002522c0`, S138's notes in research/oracle.txt) reads the pitch
  and roll from the attachment walk's post-physics euler. The ground clause compares the pitch
  alone against GroundPitchDownWreckAngle (recording2's rider: 1.2217, 70°). The airborne clause
  compares pitch against 1.4835 (85°) and roll against 1.396 (80°). The original always asks the
  airborne clause, so a bike nosed past 85° never reaches the ground un-wrecked. The capture shows
  reason 8 on the first tick after the kick.
- A level drop at ~75 ft/s is reason 5 (vertical g over FreestyleDownWreckSpeedLimit 66.0). One at
  53–55 ft/s rides out, even nosed down 15–22° onto the front wheel.

## How it was proved

Four save-state captures of the original (context/capture/s138/s325-*.csv and their .p2s and
ee.bin states, git-ignored). The port rides each one from the third saved state (the dive from its
first), on the capture's idle pad, in `high_nose_first_drops_wreck_as_the_original_does`. It passes (4 drops compared):

| capture | original | port, on | port, off | touchdown (port) |
|---|---|---|---|---|
| s325-nose22 | rides out | rides out | rides out | 2616, vertical 52.4, pitch −17.7° |
| s325-nose76 | 1 at 2615 | 1 at 2615 ±1 | the same | 2614, pitch −75.9° (wrecked on the pitch before any load) |
| s325-dive | 8 at 2516 | 8 at 2516 ±1 | no wreck in the air | — |
| s325-flat | 5 at 2658 | 5 at 2658 ±1 | the same | 2657, vertical 75.4 vs 66, pitch −2.0° |

`off_rides_out_a_nose_landing_past_straight_down` (air_wrecks.rs) holds the desktop finding without
the disc.

Opentennis check: nothing shared changed (game-sim's wreck doc, the openmx help string), so there is
nothing to carry over.

## Not verified / not 1:1

- Which jump the user landed on, and at what angle and speed, is unknown: the log has no landing
  lines. That it was a nose past straight down with Air Wrecks off is inferred from the switch's
  state in the log and the 46.8 → 16.9 mph drop at second 40. It was not reproduced from the
  user's own ride.
- The original's drops are on recording2's Race 125 at free09's farmyard (flat dirt), not the
  user's crf450 on pro250p on world_mxu_free. The desktop probe used pro250p's limits off the disc
  (vertical 66, fore-aft 18.33, ground pitch 70°, the same as recording2's rider), but only on the
  flat floor.
- The drops are hand-made (a velocity and a pitch-rate poke into a standing bike), not a human's
  jump. The brief's "generated .p2m2 from a state high on a free09 hill" was replaced by S138's
  PINE throw. It needs no tape (the pad is idle), and the throw sets the nose's angle directly.
- The original has no Air Wrecks switch, so the off verdicts (a past-vertical landing riding out)
  are the port's own and have nothing to compare against. That ride-out is proved only on the
  stand-in bike over the flat floor and on the s325-dive replay.
- The dive capture's state is its first row's, so the port's first tick rides the first row's own
  pad, not the one stamped two frames back. Every row's pad is idle, so the input is the same.
- No drop between 85° and 90° in the original (off in the port wrecks there at the touch on the
  ground pitch). It was not needed: on, the airborne clause takes it, and that clause is the
  oracle-proven `wreck_test`.
- One run of each drop. The capture rows' three phases allow the port's tick ±1 (as S138's test does).

## Next

- P345: the user decides whether Air Wrecks off should still judge a landing nosed past straight
  down.
