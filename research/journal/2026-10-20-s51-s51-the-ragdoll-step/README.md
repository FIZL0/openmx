# S51 — s51 the ragdoll step

2026-10-20. Task: `./kit plan S51`.

This session ported the rider ragdoll's step into `crates/game-sim/src/ragdoll.rs`. It covers the pin
constraint's solve and its two- and one-bone helpers, the throw's clock and force, the joints' torque
springs with critical damping (the world's first force listener, which also applies gravity and the
riding pose spring), and the body actor's physics update, which runs them at the world's iterations
and rate. Each is proved bit-exact over 10 000 cases on first_recording's live rider body actor
(`crates/game-sim/tests/ragdoll.rs`). Contact (S52), and the loading, wiring and drawing (S53), are still open. The step runs with the bone
collisions and the bike box as no-ops, and with pin data that nothing loads from the file yet. Details
are in 1-s51-the-ragdoll-step-FINAL.md.
