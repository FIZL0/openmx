# F31a — MX vs. ATV bike seed

2026-11-03. Task: `./kit plan F31a`.

State 4's SLUS-21104 bike is read into the port's types by `game_sim::handling::seed`. Those types are
SLUS-21104's own rigid body (`atv_body::Body`), not `ride::Ride`'s MX Unleashed chassis, plus its two
wheels and its engine. All 288 words the seed reads are the image's. One sim frame was run with the world
step and the body's integrate in Rust on the seeded body, and the bike's listeners still the original's
code in the interpreter. Its body is C15 ride-2 row 0's word for word, and so are the node, record, wheels
and engine. Porting the listeners is F31b. Details in 1-mxvatv-bike-seed-FINAL.md.
