# C39 — s89 rides remade

2026-11-18. Task: `./kit plan C39`.

S89's s89-across state and both ride captures were lost, so replay.rs' S89 rail-ride test skipped
everywhere. Recorded along.csv from R32's along state and remade s89-across and across.csv. There is no
uinput pad, so the rides were played as tapes (new research/r32/ride.sh). Both rides replay bit-exact on
every tick, and every ratchet holds. The test now names free06 as its world. Details in
1-s89-rides-remade-FINAL.md.
