# 1 — world streaming (FINAL)

Task MD17. 2026-10-23.

## What I did

**Measured the split first.** I ran a release build headless with `--frames 2` and temporary `Instant` prints, which are gone now.
The world was world_mxu_free, with 55 pieces: tiles and joins. Its old first frame took 61 s:

| step | seconds |
|---|---|
| read (Files::load_terrain, all pieces) | 0.01 |
| parse | 0.00 |
| Ground::new | 0.00 |
| **Ground::chunks** | **60.19** |
| owner cut (Tiled::owner per cell) | 0.46 |
| upload | the rest of ~0.5 |

**The hot step was a wrong sample count.** `Ground::new` sizes a ground as `layout.width × 256`, which is the 2-level case.
A join has only 1 level, so it was meshed at 4096² samples instead of 256². Each tile took 0.21 s; each join took 0.3–1.6 s.

- `worlds::piece_chunks` now sets `samples_x/z` from `Tile::samples()`. That one fix brings the full 55-piece build to about 0.7 s.
- The owner cut skips `Tiled::owner` for chunks that no earlier piece can overlap.
  For those chunks, the cut is just "inside keep". The full check is still made where an earlier piece's rect, plus one spacing, overlaps.

**Streaming.** I added `worlds::GroundStream`:

- **Start.** Before the ride, it builds in parallel (scoped threads) only the pieces whose gap to the rider is within `Reach::load`.
  The parked plane counts as a second near point.
- **Background.** A worker thread builds the rest, nearest first, re-sorted on every `update(near)`.
- **Drop.** Pieces past `Reach::drop` are dropped, and a late build that lands past drop is thrown away.
- **Wait.** If the rider is *over* a piece that is still queued, `update` blocks for it and logs `world ground: waited N ms for X`.
  The tests prove this never happens on a ride.

New flags:

- `--world-ground all|near` (default near);
- `--world-reach LOAD,DROP` (default `Reach::DESKTOP`, 6000/9000).

**GPU** (`game-render/src/gpu.rs`):

- **Paged vertex buffers.** The ground now lives in vertex *pages*, each at most the device's max buffer size.
  This fixes world_mxu's "Buffer size 481738752 > max 268435456". Each page grows by doubling.
- **Per-piece API.** I added `set_ground_streamed`, `add_ground_piece(piece, chunks)`, `drop_ground_piece`, `ground_chunks` and `ground_piece_hash`.
  `set_ground_painted` is now a thin wrapper over them, so race tracks are unchanged.
- **Regrid.** It re-lays the stitching grid as one column band per piece.
- **Draws.** One draw call per page run.
- **Lazy hole patterns.** The 16 stitched index patterns of a chunk with holes are made on first use.
  This halved resident memory: world_mxu all-at-once went from 6128 MB to 1896 MB.
- **Cost.** `add_ground_piece` takes 1.4–2.1 ms for an 8-chunk join piece, measured on world_mxu with a temporary print.

**Results** (release, headless, offscreen RADV, `--frames 2`; wall time and max RSS from a Python `resource` wrapper):

| world | mode | pieces before the ride | ground ready | wall | max RSS |
|---|---|---|---|---|---|
| world_mxu_free | before MD17 | 55/55 | 61.1 s | 61 s | — |
| world_mxu_free | near | 15/55 | 0.43 s build, 0.47 s with upload | 0.8 s | 339 MB |
| world_mxu_free | all | 55/55 | 0.7 s | 1.1 s | ~630 MB |
| world_mxu | before MD17 | — | never (wgpu buffer limit) | >600 s | — |
| world_mxu | near | 128/653 | 1.8 s build, 1.9 s with upload | 2.2 s | 816 MB |
| world_mxu | all | 653/653 | 12.5 s | 18 s | 1896 MB |

I also flew a 60 s biplane gas tape on world_mxu: 6 pieces were built mid-ride, peak RSS was 815 MB, and there were no waits.

## What the original does

There is nothing in MX Unleashed to match: its tracks are single grounds.

MX vs. ATV streams its own terrain differently. I did not decompile it for this task, and I did not consult opentennis either.
The design (load/drop reach with a margin, nearest first) follows `game_sim::tiles::Reach` and `Tiled::stream`, which already existed for the sim.

## How it was proved

`crates/openmx/tests/world_streaming.rs` has three tests. They use a generated world of 9 tiles (3×3, 720 ft pitch) and 4 joins (v0, v1, h0, h1), 13 pieces in all.
They ride the real shell headless on the GPU renderer with `--world-reach 200,300` and a 540-tick gas tape from t00's start line along +x:

1. **`only_the_pieces_in_reach_of_the_start_are_built_before_the_ride`.** The set built before the ride is exactly the pieces whose gap to the start is within 200 ft. This is counted from the log, not timed.
2. **`riding_along_the_row_builds_ahead_and_drops_behind`**:
   - there is no "waited" line, so every piece was built before the rider was over it;
   - a piece was built if and only if it comes within reach of the ridden line;
   - t00 and v0 are dropped;
   - nothing within drop of the end is dropped;
   - the rider ends more than DROP past v0.
3. **`the_streamed_ground_under_the_rider_is_the_whole_builds`.** At the end of the ride, the hash of the piece's chunks under the rider equals the hash from a `--world-ground all` run.
   The piece was streamed in mid-ride, not before it. The all-run built all 13 pieces, and the end positions are equal.

The tests skip without the disc or without a "renderer: gpu" line.

## Not verified / not 1:1

- **world_ultra still does not load.** openmx-mods' stitch writes `untamed_nat_or` at x = 215.99995, and the loader refuses a fraction of a foot. This happens before MD17's code runs, and it is a data problem in the other repo. So a bounded RSS on the largest world is **not shown**.
- **Bounded memory is not proved across a whole real map.**
  - On world_mxu, my biplane tape built 6 pieces and dropped none: the plane barely left the spawn.
  - The drop path is proved only on the generated world.
  - RSS held at 815 MB for 60 s, but no long crossing was ridden.
- **The "same chunks" check compares hashes of the CPU chunk data handed to the GPU, not a GPU readback.** The hash covers vertices, cells, at, missing, centre and radius.
- **The fog span in streamed mode is the world area's diagonal**, not the 3D bounds of the built chunks. The old code used the bounds, which are unknown before every piece is built.
- **The terrain files are read twice**, once for the sim's `Tiled` and once for the ground builder. It was measured at 0.01 s, so it is not worth sharing.
- **Mid-ride hitches** were not measured under `openmx --timings`. Only `add_ground_piece` itself was timed (≤ 2.1 ms per 8-chunk piece). The regrid cost on world_mxu's 653 pieces is not measured separately.
- **`--world-ground all` on world_mxu**: the first frame takes about 5 s ("ridden for 5.1s over 2 frames"), on top of the 12.5 s build. I did not dig into it.
- **The software renderer is not streamed.** It still gets the whole ground before the ride.
- **The parked plane** in world_mxu sits at free06's machine.txt coordinates (2304, 2281), which the home offset does not shift. I only added it as a near point; where it parks is unchanged.
- **The tests ride from the tile's start line, not with `--trace-at`.** A `--trace-at` start in the main loop wrecks the bike at once (reason 13).
- **Reach::WEB** is not exercised by any test.

## Next

None for MD17. Follow-ups are filed in the plan:

- the openmx-mods stitch offset;
- the plane's parking spot in worlds;
- a `--timings` look at mid-ride ground adds on world_mxu.
