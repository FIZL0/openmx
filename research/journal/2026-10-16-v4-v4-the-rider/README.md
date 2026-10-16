# V4 — v4 the rider

2026-10-16. Task: `./kit plan V4`.

The rider is on the bike. `crates/game-render/src/rider.rs` reads the three files that name each other —
a rider's `.sub`, the `.mcf` it points at, the `.psg` that `.mcf` names — builds the skeleton out of the
`.psg`'s node tree, and bolts the body to the chassis by the four `Dummy_*` node names the two models
share. `first-draw` now draws it: 58891 triangles against V1's 56856, the difference being exactly the
rider's 2035. The gear needed no new rendering code, because a licensed rider's jersey is the same
`[Material Changes]` repaint a bike's `.sub` uses on the shared chassis.

The task's brief says the skeleton is in `bike.mcf`. It is not: `.mcf` is a motion list, and the skeleton
is the `.psg` node tree it points at. See `1-v4-the-rider-FINAL.md` for that and for the one placement in
this task that is a choice rather than a fact — the bind pose's position on the bike, which only the
motions (V5) can settle.
