# F31b — closed

2026-11-17. Task: `./kit plan F31b`.

F31b was SLUS-21104's whole per-frame bike sim, split into F31b1..F31b5 (journal
`../2026-11-03-f31b-mxvatv-bike-pre-process/` and the F31b5 entries after it). With F31b5 landed, C15
ride-2 and flip-1 replay bit-exact in `tests/mxvatv_bike_ride.rs` with every part of the frame in Rust
and the interpreter running no instruction; flip-1 wrecks on row 125 and is put back on row 193. This
session checked that the parts cover the brief, ran the replays, and closed F31b. Details in
1-closed-FINAL.md.
