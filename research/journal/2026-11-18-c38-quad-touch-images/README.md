# C38 — quad touch images

2026-11-18. Task: `./kit plan C38`.

C38 asked for a pair of SLUS-21104 RAM images where two quads' collision records touch: one a frame before,
one at the touch, for MD98a. MD166's ATV race state already had two AI quads rubbing; run on from it, they
part and touch again at vsync 37523 with no contact on any record at 37521. Both frames are saved
(`context/mxvatv-ps2/c38/c38-37521.*`, `c38-37523.*`) and written up in research/oracle.txt (`# C38`).
The touching quads are two AI quads, not the player's.
