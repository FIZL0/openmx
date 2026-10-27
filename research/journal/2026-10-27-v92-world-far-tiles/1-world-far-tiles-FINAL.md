# 1 — world far tiles (FINAL)

Task V92. 2026-10-27.

## What I did

At Whole Map (no fog), a world's horizon was empty past the pieces within `Reach::DESKTOP`'s 6000 ft.
Each piece that streamed in appeared whole in one frame, and so did its props.

- **A stand-in for the whole world, resident from the load** (`worlds::stand_in`,
  `Renderer::set_ground_stand_in`).
  - It is a single coarse heightfield over every piece's rectangle. Each corner is the height and
    normal of the nearest sample on the piece that owns the place (`Tiled::owner`).
  - Its cell is a sixteenth of the finest piece's samples or coarser, capped at 512² cells, and the
    world's width divides into whole cells.
  - It is held as one ground piece (`gpu::STAND_IN`) with its own quadtree. It uses the world's paint
    shader, so its colours are the pieces' colours, coarser.
  - On world_ultra it is 948 × 277 cells of 123 ft in 270 chunks, about 6.6 MB of corners. It builds
    in 0.56-0.92 s at load, reading every owning piece's .trn on every core.
  - Why one stand-in rather than one per tile: a stand-in chunk per piece would have been 2212 pieces ×
    a 65² chunk × 24 B, about 220 MB. One grid over the whole world is a few MB, as the brief asked.
- **Where the stand-in draws: a cover texture** (R8Uint, a texel per stand-in cell).
  - A texel holds how many of an 8×8 Bayer pattern's 64 places the pieces over it draw. It takes the
    least over every piece whose rectangle overlaps it, so a texel with any piece not wholly shown
    keeps the stand-in.
  - `fs_stand_in` discards the places the pieces draw.
  - It is rewritten only for the pieces whose draw changed (`GpuGround::write_cover`), by overlap.
- **A piece streamed in comes in unseen, then fades in over `GROUND_FADE` = 1 s.**
  - Its draws use `fs_fade`. The instance index carries the fade step (1..63), and a step draws one
    more sixty-fourth of the pattern's places. The stand-in draws exactly the complement, so each pixel
    is drawn once.
  - A whole piece draws with instance 0 through the old `fs` pipeline, so its early-Z is unchanged.
  - A piece let go fades out the same way and is dropped when it reaches 0. Re-adding a piece that is
    fading out restarts it cleanly.
  - `GroundStream` says `Change::Show(i)` when a piece's last part has gone up. `main.rs` calls
    `show_ground_piece(i, 1.0)` for it, and `show_ground_piece(i, 0)` for pieces built before the ride.
  - Fades advance on the picture's clock (ticks + blend), so headless runs are deterministic.
- **Props and plants fade out by distance** (`Renderer::set_prop_fade`).
  - Between the dressing's reach minus 1500 ft (`PROP_FADE_BAND`) and the reach itself, a prop's
    screen-pattern `cover` goes from all to none. This is the same dither the plants already use
    (V55).
  - So dressing that streams in at 6000 ft arrives invisible and comes up as the rider nears.
  - It is set only on world rides, and it does nothing under `Graphics::Original`.
- **Stream order by distance and heading** (`GroundStream::ahead_first`).
  - A piece's sort key is its gap × (1 − 0.5·cos), where cos is between the rider's heading (taken
    every 50 ft, and dropped after a jump of 2000 ft) and the direction to the piece.
  - This applies to both the builder's queue and the per-frame hand-over. The hand-over keeps MD28's
    256-chunk budget a frame.
- `--world-stand-in on|off` (default on) switches the stand-in and the prop fade, for comparison.

## What the original does

Nothing to match. The original has no worlds of tiles; this is the port's own streaming (MD8/MD17).

## How it was proved

`crates/openmx/tests/world_far_tiles.rs`:

- **`a_worlds_every_tile_has_ground_from_the_first_frame`**
  - Setup: MD17's generated nine-tile world, at `--graphics full`, with a 200/300 ft reach that builds
    only 1 of its 13 pieces before the ride. The first frame is shot straight down over the whole
    world.
  - With the stand-in, each of the nine tiles' rectangles (their middle 80%) is 100% ground.
  - With `--world-stand-in off`, eight of them are 0% ground and only t00 has any.
- **`a_piece_fades_in_over_its_stand_in_a_little_a_frame`**
  - Setup: `Renderer::headless`, a stand-in over two pieces, and piece 1 lit differently from it.
  - Adding the piece changes nothing (0 pixels).
  - Shown over 1 s at 60 frames a second, no frame changes more than `MOST_A_FRAME` = 4% of the
    piece's 6014 pixels. The worst frame changed 192 (3.2%), a typical frame 82-101 (about 1/64).
  - The fade ends pixel-for-pixel on the piece shown at once.
  - Fading out is just as gentle, ends on the stand-in alone, and drops the piece.
- **`a_piece_shown_at_once_changes_every_pixel_in_one_frame`**: the swap the fade replaces changes
  every one of those pixels in one frame.
- **`world_ultras_gas_ride_streams_inside_a_frame`** (`#[ignore]`: it needs the user's mods, and wall
  clock)
  - The ride: 30 s on the gas on world_ultra with `--timings`, headless at 1920x1080, as MD28 measured
    it.
  - Result: no frame's `stream` stage reached 16.7 ms. Its peak was 6.04 ms, and the session averaged
    0.28 ms of stream a frame. The test passed.
- **Overhead shots of world_ultra by hand**, from 90 000 ft up, first frame:
  - with the stand-in, the whole world's ground shows;
  - without it, only the ~81 pieces near the start show.
  - (Not committed: they are pictures of the disc's maps.)
- **Back-to-back 30 s rides, on and off**, at load 45-66:
  - Session stream: 0.61 vs 0.72 ms a frame.
  - Frames with over 5 ms of stream: 10 vs 10.
  - The first frame's `ground` stage: 45 ms vs 19 ms. The stand-in's cover is written once there for
    all 2212 pieces. Before I made `write_cover` work by overlap it was 238 ms.
- The existing suites `world_streaming`, `wrapped_world_draw` and `world_texture` pass. The full gate
  (`./kit check --all`) passed 2574 tests. Three netplay lobby tests passed only on the gate's lone
  rerun (its known load flakes).

## Not verified / not 1:1

- **"No frame over 16.7 ms from streaming" was measured on a busy machine.** Load average was 45-66
  from other slots, and every frame of both rides, stand-in on or off, was over 16.7 ms in all (sim
  alone went to 175-455 ms on some frames). Only the `stream` stage's own time was checked (peak 6 ms
  and 14 ms on the two loaded runs, never over 16.7). A quiet machine should run the ignored test
  again.
- **world_ultra was ridden as a scratch copy with its `"wrap"` removed.** This branch's code refuses
  the mod folder's current world_ultra ("116131.5 feet wide … cannot wrap"). Main fixed that in
  edea158e, which this branch predates, and I did not merge main. So the stand-in on a wrapped world's
  copies, and its seam, were not seen on world_ultra. The copies are drawn by moving the camera, so the
  cover lookup is in world coordinates and needs no folding; that is by reading, not by a shot.
- **There is a coarse overlap strip at the load frontier.** A cover texel (123 ft on world_ultra)
  shared by a shown piece and one not yet shown keeps the stand-in. The coarse surface then draws
  inside that edge of the shown piece and depth-fights it, wherever the coarse ground is above the fine
  one. It is at least 6000 ft from the rider. I did not look at it in a ride.
- **The props' fade band is not V66's.** V66 (prop LODs and their fade band) is not done. This is a
  distance dither of its own (`PROP_FADE_BAND` = 1500 ft before the dressing reach). No test covers it,
  and I only looked at it in one world_ultra first frame.
- **A cross-fade, not a geomorph.** The stand-in and the piece are dithered, not blended in height, so
  where they differ in height the fade shows two surfaces stippled for its second.
- **The heading order is not measured.** Nothing shows that the tile ahead now lands first in a ride.
  Its sort key is new, and the ordering itself has no test.
- **The stand-in costs about 0.9 s at load on world_ultra**, reading every owning piece's terrain (a
  cache would avoid it). It costs 26 ms more on the first frame for the first cover write.
- **`shade()` runs before the discard** in `fs_fade` and `fs_stand_in`, as naga's uniformity analysis
  wants derivatives before a discard. So the discarded pixels' fragment work is not saved. Not
  measured.
- **Software renderer:** no stand-in and no fades. It draws its own ground (MD15), and none of this
  reaches it.

## Next

Nothing for this task. Follow-ups: V93 (the stand-in at the load frontier) and V94 (re-time on a quiet
machine with the wrapped world_ultra, and check the heading order).
