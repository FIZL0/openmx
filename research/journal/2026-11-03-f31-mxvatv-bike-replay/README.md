# F31 — SLUS-21104 bike replay

2026-11-03. Task: `./kit plan F31`.

C15's ride/flip captures were gone from every slot, so they were re-recorded in s3, and they match C15's
own description. The interpreter then ran SLUS-21104's own per-frame sim update off state 4. Fed only
the pad reader's words and the put-back spot from each row, it replays both captures bit-exact: the ride's
459 rows, and the flip's 465 rows through the wreck (row 125) and the put-back (row 193). This is in
crates/game-sim/tests/mxvatv_bike_ride.rs. The Rust side, a `Handling::MxVsAtv` Ride seeded from the image
and held to the same rows, is not done; it is filed as follow-ups. Details in 1-mxvatv-bike-replay.md.
