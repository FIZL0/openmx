# S15 — engine torque

2026-10-15. Task: `./kit plan S15`.

The engine, ported and proved: the power curve and the cursor the original reads every table through,
the clutch's static hold and kinetic slip and the crossing test that locks it back up, the torque that
goes through the gearbox into the driven wheel's accumulator, and the crank's own integration against
`EngineInertia` between `MinRPM` and the rev limit. `Call::EngineUpdate` and its hook are gone — a
hook replaces a function rather than wrapping it, so a ported body cannot also be recorded — which
means the two ground-step proofs now run the original's real `engine_update` and compare the engine's
revs, its torque and the wheel's torque across every substep of 2,000 frames and a 240-frame ride.
10,000 generated states for the function itself, twelve tallied paths at 100 each, 381 tests green.
The port can accelerate a bike now; it could not before. Two things are left open and both are in the
entry's last section: `+0xbc`'s setter was never found, and `MAX_RPM_POINTS` is a guess because the
`.phy` files on this disc are compressed.
