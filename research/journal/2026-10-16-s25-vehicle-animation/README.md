# S25 — vehicle animation

2026-10-16. Task: `./kit plan S25`.

The vehicle's own animation state machine — a sixty-entry jump table on the actor's animation state,
its own clip table, and the arms that reach the two things nothing else in the image reaches: the
sixth entry point of S20's animation caller and the four-clip blend — is ported into
`crates/game-sim/src/vehicle_anim.rs`. It came out bit-exact: three oracles in
`crates/game-sim/tests/vehicle_anim.rs` prove the jump table one state at a time over 24,000 frames
(including both writers of mode four, each hiding in a branch delay slot), the clip table over 2,000
vehicles, and the sixth entry point over 6,000 frames. What is left unproven is written down in the
numbered file: the out-of-range reads the frame itself can never make, and the renderer's half of the
sixth entry point.
