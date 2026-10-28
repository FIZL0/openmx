# S147 — front 2889

2026-10-28. Task: `./kit plan S147`.

The s62 front wreck parted from its capture at 2889 because of the capture, not the game and not a
collidable. The recorder wakes on the frame counter, which the main loop steps before it simulates, so
row 2889 was read in its tick before anything moved. Its bones, request and race clock are row 2888's,
word for word; only the generator has moved on. The tick ran all the same, and row 2890 holds two steps.
The first of those steps ran on a request that no row carries. Front has three such rows (2889, 2892,
2894) and the other s62 captures none. The replay now runs a torn row and the row after it uncompared,
then lays in the next row's bones. With that, the original's own update holds front's capture from
2603 to its last row, 3180 (it held to 2888), and the port replays it bit for bit. A new test shows the
cause.

Details: 1-front-2889-FINAL.md.
