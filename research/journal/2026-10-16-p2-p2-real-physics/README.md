# P2 — p2 real physics

2026-10-16. Task: `./kit plan P2`.

The shell's invented physics is deleted and the ride is a `game-sim` state: one bike value driven by
`move.rs`, `wheel.rs`, `chassis.rs` and `air.rs` on the parameters S18 reads off the disc, sweeping
S4's collision against all 353 of the scene's collision placements, with S7's follow camera on the
disc's presets behind the view. The wiring is proved by comparing the frame's own call list against
`move::step`'s proved one, by replaying a freeride fixture to the same per-tick hash twice, and by
throwing a rider at a wall; the acceptance ride is 60 s and 3583 frames with no panic. On the way it
turned out the ported engine made no torque at all — the engine's constructor writes two words that
are not `.phy` keys, so a bike built from the file alone has a dead motor. Fixed, and written into
`research/oracle.txt`. The ride is still soft, and S21's 25 dropped `.phy` keys are why.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
