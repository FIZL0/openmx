# MD9 — world regions

2026-10-23. Task: `./kit plan MD9`.

A combined world (MD8) should look like one place. On the GPU, each tile's ground keeps its own MD4
pages and detail map. A join's ground is blended from the tiles beside it, each read mirrored past its
edge, by a weight that is 1 at a tile's edge, so the ground has no seam or colour step. Settings →
Display → World Sky (`--world-sky`, `world_sky` in settings.txt) is Global (the default: the world mod's
sky tile lights everything) or Per Map: under it, the camera's tile sets the sun, ambient and fog,
blended across a join, and the skybox switches at the join's middle. Proved by
`crates/openmx/tests/world_look.rs` (CPU blend, GPU shot of the join, the sky lines in the log,
continuous mixed light). Not done: the software renderer's world ground (MD15), a crossfaded skybox,
splat-painted tiles, and the texture-limit cases. Details in 1-world-regions-FINAL.md.
