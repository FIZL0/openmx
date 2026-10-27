# 2 — the idle sweep again, on a newer main (READY)

Task S153, second pass. 2026-10-27, on main e9fc4d5a.

The task came back open with its block mark gone and S162 not done. No new word from the user was in
the plan. Since the first pass, these changes have landed in the wreck and contact paths:
- P147: the rider's head and body against the ground.
- S145: movers through the body contacts, and the toys' parts move.
- S154, S157, S118 and MD28.

So I reran the same 2312-drop idle sweep (`sweep/jobs.sh`, release build, race125p, 450 idle ticks) to
see whether any of them made an idle roll-back bail.

## What I did

- **The sweep now finds 14 wrecks, up from 9.** They are listed in `sweep/wrecks-2026-10-27.txt`.
  free06 2500,3100 h90 (the slow prop graze) no longer wrecks. Six spots are new. To find each new one's
  cause, I read `--trace` and `--trace-air` around the wreck tick and added temporary `eprintln!`s, now
  reverted:
  - one on the head-ray hit in `rider.rs` and `ride::hits`;
  - one on the struck prop's owner in `world.rs`'s `solid`;
  - one on movers near the bike in `mover_contact::struck`.
- **free06 2500,4000, h270 at tick 192 and h90 at tick 256: Scenery(6).** Each bike rolls back smoothly
  at −10 to −15 ft/s, all contacts down on flat ground (y 135.4), with no g spike. The head ray
  (`air::HEAD`) meets prop 361 at y 141.5, 6 ft above the ground. Prop 361 is a **moving** prop (its
  `Prop::moving` is set), which is a traffic car. The bike has rolled back onto a road, and a car drives
  through the rider's head. That is a traffic hit, not a bail.
- **free09 2800,4300 h270 at tick 232: Front(1).** The bike sits at 0.5 ft/s. At tick 230 it gets a
  kick: 9.8 g sideways, speed to 9.3 ft/s, one wheel off. No traffic car is within 15 ft of its radius.
  The first pass had this spot wrecking at h90 on tick 231. h0 and h180 do not wreck. A kick at a fixed
  time, at one spot, that depends on how the bike is turned looks like a toy's moving collision (S145)
  sweeping through it. I did not prove which toy it is.
- **free09 3100,4000 h90 at tick 342: Back(4).** The bike rolls forwards at 6 ft/s and gets a sudden kick
  at tick 342 (26 g along). It is an impact, of the same kind.
- **free06 4000,3100 h0 and 4300,4000 h0: Right(3) and Left(2).** Both bikes are airborne (no contacts)
  for many ticks, with vy down to −48 ft/s: they rolled off a ledge and landed on their side.
- **free06 3700,3700 h180 at tick 250: Front(1).** The bike rolls back down a long hill to −67 ft/s
  (`min` in the sweep's line). At tick 241, with one wheel off, the yaw and pitch rates jump. Heading
  wraps through 180° and pitch runs to −64° in eight ticks, which is a backward impact with a bump at
  speed.
- **free06 2500,3700 h270 at tick 187: Tail(11).** It is airborne off the same bump that S162 lists at
  2500,3700 h90.

## What this says

On this main, an idle bike rolling back on open ground still never bails. The first pass's capture shows
the original does the same: `the_captured_idle_roll_back_never_wrecks` covers it, and it is still in
`tests/replay.rs`. Every idle wreck the port has is an impact:
- traffic or a toy hitting a parked bike;
- a fall off a ledge;
- a fast backward roll into a bump.

The traffic case is new, and it is the likeliest match for the user's report. A bike left idle near a
road rolls back into the lane, and the rider's head gets hit (`Wreck::Scenery`, the head-ray clause),
with no visible reason on the bike. Whether the original's traffic wrecks a parked rider the same way
belongs to S151/S152: are the cars movers or scenery to the head ray? I filed nothing new, because those
two already cover it.

## How it was proved

Port side only: the sweep, plus traced and instrumented reruns of each new spot, on e9fc4d5a. No new
capture of the original was made.

## Not verified / not 1:1

- **I did not reproduce the user's wreck.** I still do not know their spot, their seat count, or whether
  anything hit them. Nothing in the plan says more than the first pass had.
- **I did not check the traffic hit at free06 2500,4000 against the original.** Whether a car driving
  through a parked rider's head wrecks them in the original (and as Scenery) needs a free06 state on that
  road with traffic passing. It is S151/S152's question.
- **I did not identify the kicks on free09 (2800,4300 and 3100,4000).** I am inferring a toy's
  collision from the timing and the heading dependence. I did not name the toy or compare it with the
  original.
- **The ledge falls and the 3700,3700 backward impact were not compared with the original.** This is
  the same gap as S162's two spots.
- **Only race125p was swept, with no other riders.** I did not try split screen with 8 bikes (the
  user's 10-23 settings) or AI riders, so a Joust from a passing rider was not looked for.

## Next

Same as the first pass. The user says where it happened, ideally with a `--record` tape or a `.p2m2`,
and whether a car went by. If it was a road, it is the traffic case above, and S151/S152 decide it.
