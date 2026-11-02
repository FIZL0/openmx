# MD57 — far join relief

2026-11-02. Task: `./kit plan MD57`.

A world's joins far from every map were flat blend. Both stitchers (openmx `crates/stitch`, openmx-mods
`tools/stitch`) now add the nearest maps' own relief past the seam gates, the way MD48 carries their
paint: each map's ground smoothed, held to 0.25 ft/ft, folded into its keep and shared by inverse-square
distance, rising from nothing at the gates under a 0.1 ft/ft cap and kept off blend too steep to hold
it. The seam rules and the 0.5 slope limit hold without widening any world. The five installed worlds
were rebuilt, checked and reinstalled; far ground went from 0.1–2 ft of spread to about 5 ft. Not
carried: jumps and ruts, and world_ultra's own new ground (MD70).
