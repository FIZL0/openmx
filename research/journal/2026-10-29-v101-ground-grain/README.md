# V101 — ground grain

2026-10-29. Task: `./kit plan V101`.

The ground's finest grain against the original's, and free06's bowl walls. The original's 0–1 px
grain turned out to be its frame, not its ground: a 16-bit, 4x4-dithered frame buffer, which the port
does not copy. The ground's own 1–4 px grain was short because V97's mip floor scaled both screen
gradients by one factor, which blurred ground seen at a grazing angle along its depth about four times
past the floor. Now the floor is added to the footprint's two axes (covariances add), with the paint
target's own 512x832 texel folded in. On free09 the 1–2 and 2–4 px bands went from 0.28/0.73 to
0.59/1.14, against the original's 0.90/1.13. The walls: the same floor flattens them. Read with no
floor they carry the original's grain, but the floor is right on the mid ground (V97's match), and
the rest of the gap is the walls' own content. Details in 1-ground-grain-FINAL.md.
