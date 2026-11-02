# P253 — mxu worlds

2026-11-01. Task: `./kit plan P253`.

The user found that world_mxu ("every freeride and national") had no freeride maps, only nationals. The
freerides never dropped out. All 27 tiles were built, placed, drawn and named on the radar, in openmx-mods'
installed world and in the one built off the disc alike. But the stitcher lays tiles out by edge height,
and the freerides' rings stand ~100 ft above every national's, so the five came last: one row along the
world's far edge. MD50 starts a wrapped world on its middle tile, nat01, 12,000 ft from the nearest town.
A ride saw nothing but nationals. Now a world spec can name `middle` tiles, which the stitcher lays in the
grid cells nearest its middle. world_mxu ("MX Unleashed: WORLD") names its five freerides, so the ride
starts on free07 with the other four around it. A new world_mxu_nat holds the 22 nationals. The pick
lists WORLD, then the freeride maps, then every national.
