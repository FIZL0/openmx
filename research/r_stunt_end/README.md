# When MX vs. ATV's 360 stunt ends (R47)

Proof: `crates/game-sim/tests/mxvatv_stunt_end.rs` (SLUS-21104's trick step on state04). Addresses:
`research/oracle.txt`, the R47 block after R43's. Journal: `research/journal/2026-11-18-r47-*`.

- The stunt clock that zeroes and answers 0 at the row's `Duration` (bike vt `+0x4e8`) runs only for a
  bike the network drives. Its caller, the trick step (vt `+0x4e4`), answers through it when `+0x48c` is set.
- A local bike's trick step runs the clock itself. The clock never goes back to 0. The phase is
  `min(clock / Duration, 1)`, and 1 is stored too.
- A 360 whose torque is applied (`+0x11d0`) is one of the five "kept" stunts. The clock never ends it.
- The 360 ends only when the 360 step sets the id to 255: on landing, on a touch, or when the yaw drops
  under its floor.
- Coming round only scores. The stunt id stays the 360.
- A second clip start (the second pass past 35°) runs the stunt start again. That zeroes the clocks and
  sets the restart ask (`+0x1650`), so the clip plays again from the start.
- With R1 alone held, the clock stops at the row's `+0x44` (0.72 s) and the clip is held there.
- So the port's 360 must not end at 1.5 s. It should last from the clip's first start to the 360's end.
  That is S467.
