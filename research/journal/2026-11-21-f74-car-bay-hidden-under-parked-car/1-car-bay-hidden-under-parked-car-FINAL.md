# 1 — car bay hidden under parked car (FINAL)

Task F74. 2026-11-21.

## What I did

- The shell (main.rs, the GPU actor list) no longer draws a car's see-through bay where a car of
  the same kind that the original parks in play came to rest at the load. "Where" means within
  10 ft across (x, z) of the parked car's load rest spot (`InPlay::at`), not its spot now. So a
  truck a bike pushes away still hides its bay; the bay does not come back.
- Only cars the Sim holds right now count (`InPlay::handed`). A car a player drives leaves the Sim,
  and its bay shows again, as before F72.
- Hiding by kind alone was wrong: free07's `[Baja1]` bay is at 4020, 2357, but its `[Machine1]` baja
  rests at 3902.5, 2438 (140 ft away). That bay is still drawn. free09's Monster bay is 0.05 ft from
  the truck and free11's Buggy bay 1.06 ft from the buggy, so both are hidden.
- car_picture.rs reads the `picture parked:` line now. It checks that no bay is drawn within 10 ft of
  the parked car, that free07 alone still draws its bay, and that a car is taken at its bay (exact
  where the bay is drawn, within 10 ft where it is hidden).
- world_parked_car.rs compares each car's `picture parked:` spot on its own track, moved by the
  tile, with the world's. The tolerance is 0.25 ft, because each car settles on its own (P378): up
  to 0.16 ft apart across, 0.05 ft up. It also checks that no bay is drawn inside a parked car in the
  world.
- The software renderer draws no bays, so it needed no change.
- Nothing taken from ../opentennis (it has no parked machines).

## What the original does

The original draws no bays. They are the port's switch markers (P103). Its `[<Type><n>]` sections
are spawned only for a switch, and `[Machine<n>]` is the car it parks in play (`vehicle::in_play`'s
doc). So this is presentation only: it touches no simulation, RNG or timing.

## How it was proved

- `./kit check -- -p openmx --test car_picture --test world_parked_car`: 3 pass. Both ran; neither
  skipped (run again with `--nocapture`: no "skipped").
- Headless `--trace-picture` on free09, free07 and free11: free09 and free11 print only
  `picture parked:`; free07 prints its bay at 4020.00 136.08 2357.00 and the baja at 3902.47.
- Looked by eye at target/tmp/s245-car-picture/free09-parked-behind.png: one solid truck, no
  see-through copy around it.

## Not verified / not 1:1

- The pushed case is not tested. No test pushes a parked car more than 0.09 ft (split_contact's
  truck test). The bay stays hidden by design, because the rule reads the load rest spot, not the
  car's spot now.
- The 10 ft limit is my pick (about one car length). It sits between the 1.06 ft (buggy) and the
  140 ft (baja) seen on the disc's tracks. A mod track whose bay is 10–20 ft off its parked car would
  draw the bay beside the car.
- Taking a car still starts it at its bay, not where the parked car stands. On free09 that is 0.05 ft
  off, on free11 1.06 ft off, and on free07 140 ft off, while the parked baja stays where it is. Not
  checked against the original's switch in PCSX2. Left as it was; outside F74.
- Not looked at in a window or in split screen (only seat 0's in-play cars are read, as F72 draws
  them).

## Next

Nothing.
