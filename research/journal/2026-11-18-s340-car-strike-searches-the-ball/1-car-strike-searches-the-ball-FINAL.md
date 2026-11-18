# 1 — car strike searches the ball (FINAL)

Task S340. 2026-11-18.

## What I did

- Found why C34's ball drive left the original at fc 776. It was neither half the brief named. The car's
  push and the ball's step are both right. The struck ball was missing its own ground entry.
- In the original, the car's strike wakes the asleep ball. The ball is filed and searched in the same
  pass, so its list holds the car's mirror *and* its ground before it resolves. The port's car strike
  (`PropSet::strike`) did not search the struck ball. So the ball resolved against the car alone, ~2 % fast.
- Fix: `Machine::step_props_in` (car.rs). It runs the car's props step in the props' collide pass and
  strikes through `PropSet::strike_in` with `Strike::by` set, the B34 path the plane already uses. Then the
  freshly woken prop searches at once (`search_late`). `step_props` keeps the old path (no pass at hand).
- The shell's `CarRide::step_traffic` (openmx vehicle.rs) and car_drive.rs's replay now use it.
- car_drive.rs `the_ball_strike_runs_on_its_own` is now exact through fc 780 (was 775). `BALL_STRUCK` is 781.
- Saved states inside the strike to compare frame by frame: research/s340/saves.py and capture.sh replay
  c34/ball.p2s with the tape and save a state at each listed frame_counter. The states (s2's context)
  are context/states/s340-ball-{772,774..776,778..781,784,785}.ee.bin, and each holds row fc exactly.
- Ran the original's node update in the interpreter from those states, logging each record's resolve list.
  That is how I saw the missing ground entry at 776 and the order at 781. The exploration test was deleted.
  Its findings are in research/oracle.txt # S340.

## What the original does

- 775 -> 776: the car (4 substeps) strikes ball 2. Ball 2's list: the car's mirror, then its own terrain
  entry. Then it resolves (2 substeps) with both.
- 780 -> 781: ball 2's rays meet balls 5 and 0. The original walks them in ball 2's kept nearby list's
  order (record +0x10c, a tree gather): ball 5 at place 3, ball 0 at place 4. The port walks the other props
  in collection order (prop_props.rs strikes_of): ball 0 first. The lists then differ in order, and the
  sequential resolve moves the bodies differently.

## How it was proved

- A bit-exact replay fixture: tests/car_drive.rs `the_ball_strike_runs_on_its_own`. C34's 219 rows: the car
  and all six balls (position, velocity, quat, rates, enabled) bit for bit through fc 780, and no ball wakes
  on another frame for the whole drive.
- The interpreter's node update from s340-ball-775 and -780, compared by eye with the port's lists (not kept
  as a test).

## Not verified / not 1:1

- Ball-on-ball order from fc 781 on: not 1:1. The port walks the other props in collection order, and the
  original in its kept tree gather's order. Already filed as S385. I added this drive to it as a note.
  Doing it right means putting the knockable props into each prop's own kept gather, across five search
  paths, with the S242/S258 proofs at risk. That is S385's work, not this one's.
- Everything past 781 (ball 0 off at 785, the car at 804): not looked at. These may only follow from 781.
- `Machine::step_props` (no props pass) still strikes without the late search. Only car_props.rs uses it.
  A ball it wakes resolves without its ground on the strike frame, as before.
- Late probes' mirrors: `strike_in` hands the struck prop's mirrors the riding bike's frame (B29), not the
  car's. The car's hull is met afterward in the props' step, which is how this drive comes out exact. No
  capture shows a late probe's mirror landing on the car itself.

## Next

- S385 (filed): take strikes_of's order from the striker's kept tree gather. Then raise `BALL_STRUCK` to
  the whole drive.
