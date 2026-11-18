# MD214 — kiowa put back ground snap

2026-11-18. Task: `./kit plan MD214`.

MX vs. ATV's Kiowa put-back now snaps its spawn to the ground as SLUS-21104 does: the ground query
under the spawn, the place lifted 0.3 along the normal, the up that normal. Oracled bit-exact over
10,000 spawns on Desert Oasis's terrain. `Wreckage::mxvatv` is now on for the MX vs. ATV helicopter in
the game, its spawn the model's origin where it was taken. Details in `1-…-FINAL.md`.
