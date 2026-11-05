# MD61 — the wrap join as narrow as a stitched one

2026-11-04. Task: `./kit plan MD61`.

game-sim's `Tiled` now lets a tile's offset stand below 0 where the tile keeps nothing short of 0, and
stitch's grid layout starts a wrapped axis's first line on the near seam, so each pair across the wrap
gets its own gap. world_mxu_free's crossings over the seam drop from 1134 / 1125 ft (installed) to 252 ft
(a 135 ft stitched join + the 135 ft seam strip). Details in 1-wrap-join-FINAL.md.
