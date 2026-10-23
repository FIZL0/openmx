# MD26 — sky crossfade

2026-10-23. Task: `./kit plan MD26`.

Under World Sky: Per Map, a combined world's skybox now crossfades across a join instead of switching
at its middle (MD9 blended only the sun, ambient and fog). The GPU sky shader has a second sky binding and
a small uniform with the second sky's share; `Looks::skies` picks the two maps with the largest shares
and the share between them. Proved by a GPU test of the mix and a test of the share across the
generated join, and looked at by eye across world_mxu_free's free06 → free10 join. Done; details in
`1-sky-crossfade-FINAL.md`.
