# MD70 — ultra relief

2026-11-04. Task: `./kit plan MD70`.

world_ultra's own new ground between whole worlds (openmx-mods tools/stitch/src/ultra.rs) was the
flat blend; MD57 had given relief only to the joins inside each per-game world. ultra.rs now adds the
nearest worlds' own relief past the seam gates with build.rs's code: each world's ground over its
rectangle, smoothed and held to 0.25 ft/ft, folded and shared by inverse-square distance, eased in
under 0.1 ft/ft past the gates and dented off steep blend. The layout is unchanged; far new ground
went from 0.27 to 3.72 ft of spread (median). The rebuilt world_ultra passes `stitch check` with the
discs and openmx's world tests, and is installed. Not ridden in the shell.
