# S467 — mxvatv 360 held to its end

2026-11-18. Task: `./kit plan S467`.

The port's MX vs. ATV 360 now keeps the rider and bike in the 360 clip from its first start until the
360 ends, not just until it comes round. A second pass past 35° restarts the clip (fade from the saved
pose, clip from 0), as the original's restart ask does. Proved against SLUS-21104's trick step, rider
layer and bike animate on state04 (`tests/mxvatv_stunt_cross.rs`). R1 hold and the bike's restart-tick
phase left for S478. Details in 1-mxvatv-360-held-to-its-end-FINAL.md.
