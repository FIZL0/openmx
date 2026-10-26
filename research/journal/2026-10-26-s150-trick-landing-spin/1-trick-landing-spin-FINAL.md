# 1 — trick landing spin (FINAL)

Task S150. 2026-10-26.

## What I did

- Rode crooked landings in the port first: a scratch probe in tests/ride.rs, removed before the commit,
  rode the disc's 125 off a flat-world ramp with the stick held right through the air, with and without
  the MD16 pack. About 55° crossed up wrecked (reason 3/2). About 179° crossed up (the bike yawed ~232°
  in the air) read a crossup of −1° and rode out backwards, and the ground swung the heading ~80° over
  20 ticks. That looked like the reported fling, so the question became whether the original does the
  same.
- Recorded the original doing the same thing. Recording2's rider (slot 3; `research/s150/put_state.py`
  put recording2's state there, and slot 3 has been put back since) was teleported to free09's farmyard
  at (4642.5, 135.6, 3650) with `research/s76_tele.py`. It was thrown up UP ft/s with 40 forward by
  `research/s150_spin.py` (a copy of S138's nose thrower that can also poke a yaw rate; the poke is
  damped out by the airborne auto-level within ~8 ticks, so it is used at 0). Then it was recorded with
  `research/s138_hits.py` and the stick held right (`research/s150/right.pad`). The steps are in
  `research/s150/spin.sh NAME UP 0 40 True research/s150/right.pad`, FRAMES=200, and
  `research/s150/summary.py` prints the rows. Three captures (context/capture/s138, git-ignored):
  - **s150-r50**, UP 50: touches down at 2823 with heading 164.5 against velocity heading 46, about
    118° crossed up. It wrecks with reason 3 on that row.
  - **s150-r60**, UP 60: touches down at 2801, about 148° crossed up. It rides out, and the heading swings
    round 134.5° by 2904 (yaw rate down to −2.2 rad/s). The bike ends up rolling backwards.
  - **s150-r70**, UP 70: touches down at 2855, about 169° crossed up, backwards. It rides out, and the
    heading swings −166.8° by 2945 (yaw rate at the original's 3 rad/s clamp, −2.99).
- Replayed the three in the port from their save states with the captured pad (S138's set-up: free09,
  the props, the body rays). The port already does what the original does, so no simulation change was
  needed. r50 wrecks with reason 3 at 2822 (original 2823). r60 and r70 ride out, touch down on the
  original's tick, and keep the heading within 5.06° of the original's on every tick of the swing.
  - An early replay from each capture's *first* state disagreed on r50 (the port rode it out). Those
    states' first two ticks have no pad row (the recorder polls frame−2), so the replay started with the
    stick centred and the airborne auto-level took yaw out. From the third state, as S138's replays
    start, the runs agree.
- Test: crates/game-sim/tests/replay.rs `crooked_landings_wreck_or_swing_round_as_the_original_does`
  (S150_LANDINGS, `ride_a_crooked_landing`, `HEADING_FROM_THE_ORIGINAL` = 6°). The original's wrecks
  must wreck in the port with the same reason, ±1 tick. Its ride-outs must ride out in the port, touch
  down within a tick, and keep the heading within 6° of the original's from the touchdown to the end of
  the capture. The worst stray seen is 5.06°, six ticks after r60's touchdown.
- Mid-trick landings were not captured again: P3's
  `a_scripted_pad_lands_one_trick_and_wrecks_another` (tests/trick.rs) already proves that a stunt still
  running at touchdown wrecks with the trick clause's reason, and tests/air.rs proves the clause bit-exact
  against the original. Nose and tail landings are S138's captures (nprobe3..7), tested in
  `walls_and_nose_landings_wreck_as_the_original_does`.
- Filed S153: if the user wants a backwards landing to bail, it should be an option that is off by
  default, so the original stays 1:1.

## What the original does

- **What decides wreck or ride-out** is the landing crossup clause of the wreck test (`./kit fn
  0x002522c0`). It applies on the touchdown tick after more than CrossupWreckTime of hang time, and
  fires reason 3 or 2 when the shown crossup is outside MaxCrossupAngle (45°). r50's wreck row has a
  side g of 0 (g_peak 0, 0, 0 on 2823), so reason 3 there is this clause and not the side load.
- **The crossup is an arcsine**: the asin of |heading × velocity| (flattened), signed by the cross's y,
  so it is bounded at ±90° and reads θ as 180° − θ. The capture's `crossup` column at touchdown shows
  this: r50 (118°) reads −1.05 rad (60°), r60 (148°) reads −0.54 rad (31°), and r70 (169°) reads
  −0.14 rad (8°). So any landing past 180 − 45 ≈ 135° crossed up passes as nearly straight and rides
  out.
- **What swings the bike round** is the tyres' side grip once the wheels are down. A bike running
  backwards is turned toward its velocity at up to the body yaw clamp (YawRateLimit, 3 rad/s), and in
  three seconds it is turned through 135–167° (r70 ends at heading 69, near its run-up's velocity heading of 67; r60 ends at −17, still rolling backwards with velocity heading 163). Nothing
  in the trick or wreck code does this; it is the riding contact, and the port's matches it tick by tick.
- Notes and numbers: research/oracle.txt, S150 section.

## How it was proved

Three save-state captures of the original (r50, r60, r70 above), each replayed in the port from the same
state with the captured pad, the port's first wreck (tick, reason) and heading compared with the
original's on every tick:

| capture | crossed up at touchdown | original | port | heading stray, worst |
|---|---|---|---|---|
| s150-r50 | ~118° (reads 60°) | reason 3 @2823 | reason 3 @2822 | — |
| s150-r60 | ~148° (reads 31°) | rides out, swings 134.5° | rides out, down on the same tick | 5.06° |
| s150-r70 | ~169° (reads 8°) | rides out, swings −166.8° | rides out, down on the same tick | 4.52° |

A test run with the bound at 0 failed with exactly the two strays above, so the heading comparison does
run on both ride-outs.

## Not verified / not 1:1

- **MX vs. ATV was not captured.** The brief asks for Tricks Original and MX vs. ATV. Only Tricks
  Original's (free09, Race 125) landings were captured. The port's MD16 pack runs on the flat-world probe
  rode out or wrecked as without the pack, but no MX vs. ATV original was recorded for comparison (it
  needs its own copy and states, memory note "MX vs. ATV runs in your own copy").
- **Mid-trick landings were not captured from the original for this task.** That they wreck rests on
  P3's port test and on tests/air.rs's bit-exact clause, not on a crooked trick landing in a capture.
  The scratch probe's trick buttons did not start a trick in the port either, so the port's mid-trick
  crooked landing was not exercised here.
- **The boundary itself (~135°) is inferred** from the arcsine and the 45° limit, bracketed by 118°
  (wrecks) and 148° (rides). No capture lands between them.
- **Only one direction**: all three throws yaw right with the stick held right; a left-handed landing
  (reason 2) was not captured.
- **The landings are hand-made** (a throw from a standing bike, and one pad held through the air), not
  a human's trick. Hang time, speed (36 ft/s) and ground (free09's flat farmyard) are what the throw
  gave, and there is no sweep over them.
- **Tick ±1, heading within 6°, not exact**: capture rows are read at varying phases (memory note
  "capture rows read at three phases"); the worst stray (5.06°) sits where the swing turns fastest.
- **The captured headings are rounded to whole degrees** in the probe's printout only. The test
  compares f32s.
- **The test skips** where context/capture/s138's s150 captures or their `.ee.bin` images are missing,
  and still reads as passing (memory note "a passing check hides skips"). The s2 slot has them.

## Next

Nothing in this task. S153 is the user's call.
