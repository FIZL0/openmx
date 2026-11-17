# 1 — shovel by eye (FINAL)

Task S267. 2026-11-16.

## What I did

- Shot free10's shovel (the excavator, `shovel.psg`) in the app from three sides, after S245's node-list
  reading (this branch) and before it (the same build with the lists' use switched off in `psg.rs`, then
  put back). Release build, `--headless --frames 40 --shot-frame 30 --overlay none --size 960x540`:
  - south `--shot-pose 2992.5,300,3700,0,-25,60`
  - west `--shot-pose 2750,250,3960,90,-20,60`
  - east `--shot-pose 3250,250,3960,270,-20,60`
  - The shovel stands at (2992.5, 139.7, 3960) on free10.
  - Shots: `context/shots/s267/{before,after}-{s,w,e}.png`.
- **Before**: the boom is gone and the bucket floats on its own above the cab. 732 of the 2293 vertices
  sit outside the node they were bound to.
- **After**: whole. The boom runs from the cab to the bucket, with the dirt pile in the bucket and the
  tracks under the body.
- New `crates/game-render/tests/shovel.rs`: every shovel vertex lies in its node's bounds, as
  `tests/car.rs` checks for the cars. Red with the lists switched off (732 vertices outside), green on
  this branch. Skips without a disc.
- Nothing taken from `../opentennis`: it has no `.psg` models.

## What the original does

Nothing new. S245's journal covers the node lists (flag 0x80, one list per mesh).

## How it was proved

- I looked at the before and after shots in the app (above).
- The bounds test: each vertex inside its node's stored box. That box is the file's own data, so it is
  independent of our binding.

## Not verified / not 1:1

- No PCSX2 shot of the original's shovel to set beside ours. This task asked for before and after in the
  app. The bounds test and the whole-looking model stand in for that check.
- I looked at one moment of the shovel's clip only (frame 30 of a fresh load). I did not look at it
  through its whole ten-second dig loop. The binding is the same at every pose, and S133's toy_parts test
  proves the motion.
- From these high angles the excavator looks like it floats a little over the ground. Before and after
  look the same here, so S245 did not cause it. I did not look into it: placement is not this task.

## Next

None.
