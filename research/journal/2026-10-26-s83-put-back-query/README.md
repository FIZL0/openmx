# S83 — put back query

2026-10-26. Task: `./kit plan S83`.

The put-back's clearance and overhead probes now ask the original's query on a real track: the bike's
own search list, gathered over the props and plants only when the bundle has moved ten feet, each
flagged mesh swept in the prop's own frame, plants refusing within 20 ft, and (for a race the port
does not have yet) checkpoints and the static-only gather. `game_sim::back_search`, kept per ride in
`Ride::back_search`. Proved bit for bit against the original's `put_back_solid` on S66's bike save
state (which is free09's world, not free06's as named) over 12 000 generated bundles, plus 10 000 with
made-up checkpoints. What the port's world lacks (toys, bikes, hierarchies) is left out of the proof.
Details in `1-put-back-query-FINAL.md`.
