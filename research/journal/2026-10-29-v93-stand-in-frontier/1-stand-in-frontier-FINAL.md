# 1 — stand in frontier (FINAL)

Task V93. 2026-10-29.

## What I did

V92 decided per stand-in cell (123 ft on world_ultra) how much of the stand-in to draw. A cell's
level was the least drawn of the pieces overlapping it. So a cell shared by a shown piece and one not
shown yet kept the whole stand-in, including the part inside the shown piece. Wherever the coarse ground
there stood above the fine ground, it poked through.

Now the stand-in is cut by each piece's rectangle in the shader (`gpu.rs`, `fs_stand_in` in
`faded_ground`):

- **What the GPU holds.** The cover texture (now R32Uint) gives each cell a start and a length in a
  list of piece slots. Cells with the same pieces share one list. Per slot there are two textures, 256
  slots a row: the piece's rectangle in world feet (Rgba32Float), and how much of it is drawn in
  places of the 8x8 pattern (R8Uint).
- **Per frame.** `write_cover` writes only the level texture (one byte a piece) when a piece's draw
  changes. It used to rewrite the levels of every texel under the piece and its neighbours.
- **Which level a pixel uses.** A stand-in pixel inside a piece's rectangle uses that piece's level.
  A pixel inside none of its cell's rectangles keeps V92's rule, the least of them. That is the world's
  ragged outer edge and any gaps between pieces.
- **Filling the gap the cut leaves.** Cutting at the rectangle alone opens a gap at the edge wherever
  the coarse ground beyond the edge stands higher than the shown piece's edge. You see past the edge
  under the coarse surface. Inside a shown piece's rectangle, in a cell shared with a piece drawn less,
  the stand-in therefore still draws the piece's places, but at `frag_depth = z × 0.001` (reverse-Z):
  behind everything else. It only shows where nothing else drew, which is that gap, and it can never
  come in front of the piece. Cells with no frontier still discard as before.

This takes the brief's first option, the per-rectangle cut, plus the fill. "Drop the stand-in's height
under shown neighbours" would need a drop per cell at least as large as the coarse-minus-fine error,
which the GPU does not know, and it would dig a trench on the unshown side.

Nothing in opentennis covers this (it has no streamed terrain).

## What the original does

Nothing to match: the stand-in is ours (rendering, matched by eye per AGENT.md).

## How it was proved

- **`world_far_tiles.rs` `the_stand_in_stops_at_a_shown_pieces_edge_inside_a_cell`** (GPU, skips
  without an adapter). Two pieces meet at x = 500 ft, inside an 18 ft stand-in cell (486-504). The
  stand-in is 5 ft above them, but below them at the world's left and far edges so those edges are not
  counted. Piece 1 is shown whole and piece 2 is never added. The test draws four frames from piece
  1's side: empty, the piece alone, the stand-in alone, and both. It checks three things:
  - every pixel the piece draws alone, it draws the same with the stand-in (`poked`);
  - over a quarter of the stand-in's own pixels still draw beside the piece;
  - no pixel that either draws alone is empty with both (`open`).

  The same test against each version of the shader:

  | shader | poked | open |
  |---|---|---|
  | V92's per-cell least | 124 of 12293 | — (fails on poked) |
  | rectangle cut, no fill (`discard` instead of the far depth) | 0 | 66 (a line along x = 500) |
  | this (cut + fill behind) | 0 | 0 |

- The other `world_far_tiles` tests still pass: the fade's 4% a frame, fading out to the stand-in alone,
  and the nine-tile world's first frame.
- **world_ultra at Whole Map, by eye** (`--graphics full`, headless 1600x900, `--shot-pose` 700 ft
  over the start looking out at four headings plus one from 6000 ft up). Each pose was shot with V92's
  shader and with this one (same tree, the shader's test swapped back for the V92 build).
  - At the default reach (6000 ft) the frontier is a few pixels tall, and the two differ by a few
  pixels.
  - With `--world-reach 1500,2500` the frontier is close. There every difference sat on the frontier
  line. Looking west, V92 drew a flat coarse slab over the shown hillside, covering a fine track line.
  With this shader the line runs on to the piece's edge, and the slab left is the unshown neighbour's
  own stand-in.
  - (Shots not committed: they are pictures of the disc's maps.)
- **GL backend** (`WGPU_BACKEND=gl`, what WebGL2 runs): the new test passes, and the westward frontier
  shot differs from Vulkan's by about 11 pixels.
- **Cost.** Old and new binaries were A/B'd three times, alternating, 400 frames at 1920x1080 over the
  start (`--timings`, load 20-40):
  - `all` 40.7/32.2/38.7 ms (old) vs 36.7/16.7/35.4 ms (new); `ground` 5.8/4.5/5.9 vs 4.9/1.9/4.3 ms.
  - Too noisy to put a number on, but new is not slower in any pair. On the CPU side `write_cover`
    now writes one byte a changed piece, not every texel under it and its neighbours.

## Not verified / not 1:1

- **The stand-in shader now writes `frag_depth`, so it loses early depth rejection.** Every stand-in
  fragment, including those under shown ground, runs the cover lookup before it is discarded. The A/B
  above was on a loaded desktop and showed no slowdown, but it is noise-bound. It was not measured on
  a phone or in a browser.
- **The fill draws behind everything inside a shown piece's rectangle in a frontier cell.** So if a
  shown piece had a hole of its own inside its rectangle (cells it does not draw) in such a cell,
  the coarse ground would show through that hole instead of what is behind it. I know of no world
  piece with interior holes and did not look for one.
- **Pixels inside none of a cell's rectangles keep V92's per-cell rule.** That covers the world's
  outer edge and any gaps between pieces. So the world's outer edge is as V92 left it: where the coarse
  ground there stands higher than a shown piece's edge, the cut-off is open there as before. The fill
  only acts inside a rectangle.
- **Limits:** at most 255 pieces in one cell, and 2^24 list entries, or `set_ground_stand_in` returns
  an error. world_ultra (2212 pieces, 123 ft cells) is far under. I did not measure its list length.
- **world_ultra was only looked at in single frames,** ten poses per shader, not over a ride, and not
  through a fade at the frontier. The fade's own proof is the existing GPU test, which still passes.
- **The browser (wasm/WebGL2) was not run;** only wgpu's GL backend on the desktop.

## Next

Done.
