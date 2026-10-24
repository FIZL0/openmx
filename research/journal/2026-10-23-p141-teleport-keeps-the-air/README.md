# P141 — teleport keeps the air

2026-10-23. Task: `./kit plan P141`.

A marker's Up onto another player used to drop you on the ground under them. Now it puts you where their
machine is that tick, in the air too. A bike sent to a flying plane appears 20 ft under it at the plane's
velocity and falls as it would off a jump. A plane sent to a flying plane flies on 40 ft off that plane's
right wing, as that plane flies, keeping its own throttle. A plane sent to a moving or jumping rider appears
20 ft over them at their velocity and stalls. A plane sent to a rider at rest lands beside them as it always
did. All of it works offline and online. `crates/openmx/tests/teleport_air.rs` proves it on the disc's
biplane; details in 1-FINAL.
