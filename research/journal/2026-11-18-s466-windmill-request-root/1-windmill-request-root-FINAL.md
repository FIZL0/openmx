# 1 — windmill request root

Task S466. 2026-11-18.

## What I did

- Brief: in C37's remade B15 south3-bones, R24's seeds left the bones (−0.45, −2.8, +4.5) ft/s off
  from the throw's tick on. Asked: what does the riding body do differently on that tick?
- Answer: nothing in the sim. The test held the drawn bike's root and the four holds at the image's
  pose for the whole ride. The original's root moves, and on the throw's tick it snaps back near the
  chassis node.
- Added `req` to research/r14_windmill_bones.py: each row also logs the riding request's rows, frame,
  rates and hands/feet (body actor +0x488, +0x10..+0xf0).
- Recaptured the same run into context/capture/s466/south3-req.csv (not committed). Its 61 rows are
  bit-identical to b15/south3-bones.csv on every shared column (the test checks this).
- tests/replay.rs: `r24_ride_held` (r24_ride with per-tick holds handed in, as a shell does) and
  `s466_holds` (the request's frame and holds per row, carried into the rows' frame as b19_riding
  does off an image).
- `the_windmill_strikes_the_ports_bones_left_at_its_face` now also rides with the per-tick holds and
  pins the result. The old held-root gap stays pinned beside it as the contrast.
- Follow-ups: S473 (the gap left from tick 4), S474 (does the shell's drawn root snap the same way).
- Nothing taken from opentennis.

## What the original does

- The request's +0x50 frame is the bike model's "Bike" root node. It carries the drawn air lean.
- In the image (tick 0) it is rolled ~30° off the chassis rows, and stays near that on tick 1.
- On the throw's (verdict) tick it is back within a few degrees of the chassis rows.
- The riding pose spins the bones by the bike's rates about that root's third axis. About the image's
  root that is −20.48 rad/s; the original's request has −22.298. That 9% is the whole gap on the
  throw's tick and the entry.
- Recipe: put south3-2487.p2s into slot 9, then under `./kit pcsx2 with --`:
  `r14_windmill_bones.py PID 9 2439 140 4067.5 8 context/capture/s466/south3-req.csv bones ride=0 req`,
  with the copy's NominalScalar at 0.25 so no row is missed. The pause has to land on frame 2487
  (rerun if it lands late).

## How it was proved

- A save-state capture compared frame by frame: context/capture/s466/south3-req.csv, 61 rows.
- With the original's per-tick root and holds the port's bones match through the entry. Worst bone:
  0.010, 0.038, 0.053 ft on ticks 1–3; mean velocity gap under 0.6 ft/s. Before: (−0.45, −2.8, +4.5)
  ft/s and 6.07 ft by tick 23.
- The throw force the port draws is still bit-exact with the original's.

## Not verified / not 1:1

- Tick 4 on: the original's bone collisions 7 and 8 touch something (touch word 384), the port's
  touch nothing. Bone 16 is 13.5 ft/s low in y that tick; the mean is 1.4 ft/s low from then on,
  2.0 ft worst by tick 23. Not looked into: out of S466's question. Filed as S473.
- Whether the port's shell hands the riding body a root that snaps like the original's on a verdict
  tick. The test feeds the original's root, so the sim is proven; the shell's drawn root is not.
  Filed as S474.
- The skeleton is still held at the image's pose each tick (as before). Not checked against the
  original's per-tick pose.
- Only one run (south3) was checked.

## Next

- S473, then S474.
