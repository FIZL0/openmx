# S53 — the ragdoll in the ride

2026-10-20. Task: `./kit plan S53`.

This task is blocked on its two prerequisites. S51 (the step) and S52 (the contact and the settle
test) were not merged, and S52 was editing the same files, so the ragdoll cannot run in the ride or
be drawn yet. The one piece that stands alone is done: the wreck sound's random draw, in
`crates/game-sim/src/wreck_sound.rs`. It is proved bit-exact by two 10 000-case oracles on the live
emitter, but it is not yet wired into the ride. Details, the unproven parts, and the order to finish
in are in 1-s53-the-ragdoll-in-the-ride-PART.md.
