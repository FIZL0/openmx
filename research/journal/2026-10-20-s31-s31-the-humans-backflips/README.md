# S31 — s31 the humans backflips

2026-10-20. Task: `./kit plan S31`.

This task measured the human's backflips in recording2 on the original, then replayed them through
`air::backflip`. There were three flips:

- flip 1 landed after 1.04 turns;
- flip 2 landed after 1.00 turn;
- flip 3 wrecked mid-rotation at 0.60 turns.

The lips were open 18, 19 and 20 ticks. The human crossed the stick from −0.8 to +0.8 in 1–2 ticks,
and each flip fired on the first substep of a tick, one or two ticks after the stick passed +0.8.

`the_humans_backflips` gives the port the original's state tick by tick. It reproduces arming, firing
and the turning torque to the bit on every tick outside a wreck except four. On those four the stick
moved inside the tick, and which value each substep reads is still open. `air.rs` did not change. The
hold the torque is mapped from is a flick speed, not a time.
