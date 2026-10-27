# 1 — tail landing wrecks (FINAL)

Task S144. 2026-10-26.

## What I did

- Built a tail-landing capture from S138's nose-landing one. research/s144_tail.py is
  research/s138_nose.py with a yaw rate added; a negative pitch rate puts the tail down.
  research/s144/tail.sh teleports recording2's bike to free09's farmyard (slot 3 → 4 → 8). It throws the
  bike up, sets the rates once it is airborne, and records from slot 8 with research/s138_hits.py. That
  writes to context/capture/s144 and saves a state on the first row (`--when 1`). The pads are in
  research/s144/*.pad. Each one stays idle for the first 150 ms, because the replay reads the pad two
  frames back.
- Captured 17 landings and kept 12 as cases (the rest were tuning probes): probe1–4, mid, tall,
  cross5–8, midgas and midback. The RAM images were made with `./kit state ram`.
- Added `tail_landings_wreck_as_the_original_does` and its helper `wrecks_as_the_original_does` to
  crates/game-sim/tests/replay.rs. They replay the same way as S138's
  `walls_and_nose_landings_wreck_as_the_original_does`, which was left untouched. A copy was safer for
  the merge than refactoring a test S139 and S140 may still touch. The helper reads row 0's pad for the
  first two ticks, because these states sit on the capture's first row.
- Changed nothing in ride.rs, air.rs or wreck_bike.rs: all twelve cases already agree.
- Checked the shell for anything that loosens the judgement and found nothing. Air Wrecks is on by
  default (menu.rs). The user's `tricks = mxvatv` pack lays only its 360, front flip and the
  `[Airborne Control]` unroll/unyaw gains (trick_pack::apply). It never reads the pack .phy's
  `[Wrecks]`, which are MX vs. ATV's own (freestyle down g 48 MPH against MX Unleashed's 45). The shell
  adds the bike's collision files and steps at `tick_dt`'s 30 Hz, the same as the test.
- Took nothing from opentennis: it has no bike landing.
- Filed S178 and a plan/HUMAN.md item to get the user's actual landing on tape.

## What the original does

`./kit fn 0x002522c0` (bike_wreck_test) has no tail-landing clause of its own. A tail-first touchdown is
judged by these:

- the down g peak (+0x554) against FreestyleDown 66.0 / RacingDown 58.67 → reason 5. The rear shock
  bottoming out is just part of that impulse: tall's g.y was 72.46 with the rear compressed to 1.00.
- the post-physics pitch (S138's step 4b) past GroundPitchUpWreckAngle 70° with a wheel down → reason 11.
  probe4 touched down at 69.5° and pitched on to 71.37° three ticks later.
- the side g and the airborne pitch/roll limits. cross6 landed crossed up and leaned, front-first:
  reason 2. midback looped over in the air: reason 11.
- the crossup clause needs the pose's crossup past 0.99 or below 0.01 after a 0.75 s flight. It never
  fired, even at 35° crossed up and 43° leaned (cross7 rides out).

In every capture, a rear-first touchdown put no fore-aft or sideways g on the chassis (g.x = g.z = 0), so
the back and front limits never come into it. The numbers are in research/oracle.txt under S144.

## How it was proved

Each case was a save-state capture in the original (PINE recording, S67's whole-bike spec plus the wreck
reason), replayed in the port from the same state with the captured pad on free09's world with its props
and the bike's body rays. The test compares the first wreck's reason and tick (±1, the row phase), or
"none" on both sides. 12 of 12 agree:

| capture | landing | original | port |
|---|---|---|---|
| probe1 | 8° tail, 26/s down | rides out | rides out |
| probe2 | 15° tail, 32/s | rides out | rides out |
| probe3 | flat, 47/s | rides out | rides out |
| mid | 52° tail, 49/s, rear 0.93 | rides out | rides out |
| probe4 | 70° tail, 52/s, rear bottomed | reason 11 @2837 | reason 11 @2836 |
| tall | 52° tail, 73/s (g.y 72.5) | reason 5 @2883 | reason 5 @2882 |
| cross5 | 34° tail, 41/s, crossed 13° | rides out | rides out |
| cross7 | 25° tail, 40/s, crossed 35°, leaned 43° | rides out | rides out |
| cross8 | 35° tail, 64/s, crossed 20° (g.y 62.1) | rides out | rides out |
| cross6 | crossed 49°, leaned 50°, front first | reason 2 @2817 | reason 2 @2816 |
| midgas | 55° tail, 53/s, gas held | rides out | rides out |
| midback | gas + stick back, loops over | reason 11 @2766 | reason 11 @2766 |

The per-tick print matches, not just the verdicts. At probe4's touchdown the original's row 2834 and the
port's row 2834 both read g (0, 48.82, 0), vel (1.30, 0.66, 25.68) and pitch 69.502. At probe3's touchdown
both read g (−0.19, 46.52, −0.23).

## Not verified / not 1:1

- **The user's own landing was not reproduced.** None of the twelve landings is more generous in the port
  than in the original, so whatever the user saw is outside them. Candidates not captured: a sloped
  landing ramp, a rider other than recording2's (another bike set's limits), a race (the 58.67 racing limit
  and its flag were never set here), and free06. S178 waits on a tape of the user's ride.
- **The shell was not run.** That the shell judges landings as `Sim::frame` does in the test rests on
  reading main.rs (same rider build, bike col, `tick_dt` at 30 Hz), not on a replay through the shell.
- **The landings are thrown by hand.** Velocity and pitch/yaw rates were poked into a standing bike, then
  the bike was left idle or the pad held. These are not a human's jumps. All of them landed on free09's
  flat farmyard: no slope, no jump face.
- **The pad for the first two ticks is row 0's**, not the pad the original polled two frames before the
  state. It is idle in every case: the pad scripts wait 150 ms before they press anything.
- **Ticks agree within ±1, not exactly**, because rows are read at varying phases (as in S138).
- **No case reached the crossup clause (reason 2/3 via 0.99)** or the back/front g limits on a landing,
  so the port's version of those clauses is not exercised by a landing here.
- **No 50cc bike and no no-hander or trick landing** (those are S138/S10's), and no racing flag.
- The capture states and CSVs are in this slot's context only (context/capture/s144). The test skips
  with a line where they are missing, and a skipped run still counts as a pass.

- **The gate ran with this slot's mods/tricks parked** (target/tmp/parked-mods-tricks, put back afterwards).
  The folder is git-ignored. On this branch's base, whip_tricks.rs's 360 is the stale one-push test that
  V69 (7a8d9c6f, on main, not in this base) rewrote, so it reds whenever the pack is present. The pack
  tests skipped in this gate run. one_fps and ecosystem's draw budget reddened once each under a load
  average of about 30, and passed or are timing-only.

## Next

S178: get the user's tape (plan/HUMAN.md), find the landing, put it to the original and replay both.
