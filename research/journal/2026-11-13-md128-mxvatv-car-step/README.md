# MD128 — mxvatv car step

2026-11-13. Task: `./kit plan MD128`.

MD128 ported most of what MD106b's car calls hand out. That is the map-edge test and throw, the body
reset, the spatial update, the node bounds, the collision sync, the record's gravity (empty list only)
and the subtree marking, all in `atv_car::Own`. The proof is MD106b's oracle with those calls unhooked
as well: 3,000 frames bit-exact, with 35 branches each seen at least 30 times. The respawn stays handed
out (MD148), as do the cue, rumble, look and landing sound. Along the way I fixed the test harness: the
MD127 sort-place hook had been swallowing every ray probe's stand. Details are in
1-mxvatv-car-step-FINAL.md.
