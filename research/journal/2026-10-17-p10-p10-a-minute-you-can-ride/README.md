# P10 — p10 a minute you can ride

2026-10-17. Task: `./kit plan P10`.

A scripted minute — launch, flat out, a turn each way, a jump, a brake to a stop — ridden twice: once from
`Controls` built by hand (`crates/game-sim/tests/ride.rs`, section 7) and once through the shell's own pad
bytes out of a `./kit pad` script (`crates/openmx/tests/ride.rs` and `ride-minute.pad`), both headless
under `./kit check` and both producing the same numbers frame for frame. Every leg asserts what a rider
would notice rather than a number the parameters decide, with the three absolute numbers it does use —
top speed, sharpest turn, hardest braking — read back out of the human's recording by a capture-backed
test. It came out loud: three defects, each traced to a line, make the port unrideable today — the clutch
latches so the bike cannot pull away (P11), the steering command never reaches the hinge so it cannot
turn (P12), and a locked clutch re-drives the rear wheel at idle so the brake cannot stop it (P13). Those
three legs ship `#[ignore]`d with the defect named in the test, so `./kit check` stays green for the tasks
in flight; the other legs pass, and a deliberately over-geared bike proves they are not vacuous.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
