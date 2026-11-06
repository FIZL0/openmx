# F46b — gather in rust

2026-11-05. Task: `./kit plan F46b`.

The collision sync's gather of the bike's nearby list round the active probe now runs in Rust in the
F46 lockstep harness: state 4's quadtree, moving list and collidables read out of RAM,
`atv_world::gather` run, and its stamps, flag bytes, walk state, query pool and list written back.
ride-2 and flip-1 replay with the moves and the gather Rust's, RAM alike every frame; 300 state-4
gathers match the original's RAM. Details in the numbered files; the newest one's suffix says whose
move it is (FINAL · PART · READY).
