# F46 — database moves in rust

2026-11-05. Task: `./kit plan F46`.

The bike's collision record's two probe searches (the sweep, the part search) and its collision sync's
database half (each collidable's spatial-database move, the nearby-list gather) in Rust, in lockstep with
`posts_frame` over C15's ride-2 and flip-1, RAM alike every frame. Split four ways: F46a (the moves, here),
F46b (the gather), F46c (the sweep), F46d (the part search); the close-out (2) runs ride-2 with all four
Rust's together, as F46d's flip test already did for flip-1. Details in the numbered files; the newest
one's suffix says whose move it is (FINAL · PART · READY).
