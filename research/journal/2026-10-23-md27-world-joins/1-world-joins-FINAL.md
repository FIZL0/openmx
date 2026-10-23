# 1 — world joins (FINAL)

Task MD27. 2026-10-23.

## What I did

**What throws inside a world today.** The answer is nothing in openmx's own code, only holes in the
data:

- The edge throw is `off_map::outside` on the world's rectangle (`Tiled::area`), or a hole near the
  bike: `World::off_world` probes ±150 ft (`off_map::MARGIN`) for ground that no tile and no join owns.
  A tile's own edge is never asked.
- So inside a world the only throws are holes within 150 ft. Stitch builds from before openmx-mods C11
  had them: 9-ft strips every 2304 ft where a join's last terrain cell, which openmx never owns, met
  the next piece. openmx-mods' `out/mods/tracks/world_mxu_free` is one of those builds (10:54, before
  C11's 16:09 fix). The new `world holes:` line shows **84 holes** on it. Rebuilt with stitch at its
  HEAD, it shows **none**.
- The original places nothing that throws. Its shootoff is only the margin test. The MXU freeride
  scenes have no shootoff props, triggers or walls in the band: the band's placements are
  `TerrainScreenSpace` rut decals.
- What a player *sees* as shootoff ground is still in every world. Each map's ~675-ft plateau at
  ~190 ft lies outside its ridge (peak at d=87/88 samples, ~250 ft). Stitch's berm cut (keep
  `[17,17,751,751]`) keeps it, so two maps meet across two plateaus with a ridge on each side.
- One way the port could have thrown or walled a rider inside a world: a tile's collision shape
  standing in its cut band, on ground a join or neighbour now owns, is an invisible wall there.
  Stitch drops listed placements. Anything it missed was kept.

**Changes:**

- `game-sim/src/tiles.rs`, `Tiled::holes()`: every hole in the world, found exactly. It cuts the world
  on every piece's owned edges, as `Tile::local` draws them, and asks `hole` at each cell's middle.
  It is not a sampled grid, which would step over a 9-ft strip.
- `openmx/src/worlds.rs`, `find`: a tile's collision shapes and models whose origin stands outside its
  keep are left out. `first_owner` counts the kept shapes.
- `openmx/src/worlds.rs`, `cut_look`: the same cut for the home tile's drawn scene.
- `openmx/src/worlds.rs`, `WorldMod::holes_line`: the shell prints `world holes: none, so only the
  outer edge throws`, or the count and the first four rectangles.
- `openmx/src/main.rs`, `--trace`: prints `thrown: tick N at x, z` on the edge's wreck
  (`Wreck::CANNONBALL`), so a ride can tell a throw from a crash.

**The ridge cut** is in openmx-mods' stitch (`find_keep`), so it is filed there as **M23**. It is
committed by `agent-runner plan add` on openmx-mods' local main, 1 ahead of origin. Per that repo's
AGENTS.md it is not pushed by me.

Prototyped it in a scratch copy of stitch (`find_keep` moves the cut to the profile's highest point
within 1200 ft past the berm, if it stands >10 ft above it):

- keep comes out `[88,87,681,680]`.
- The tiles' `at` comes out at -432, which openmx refuses, so every `at` was shifted by hand.
- That leaves **39 holes** in the outer band x/z 0..432, which no join covers.
- The ridge-top join is ~420 ft wide and flat (profile at z 3000: 150 → 254 → ~255–260 for 420 ft →
  218 → 119).
- Join slope is 2.27 at the seams against 0.36 elsewhere.
- ~45 placements are dropped.

All of that is in M23. The port side needs nothing more: a keep that ends at the ridge is ridden as
any keep is.

Nothing was taken from opentennis; it has no tiled worlds.

## What the original does

Nothing to port. MX Unleashed rides one map, and its shootoff is the margin test on that map's edge (the
port's `off_map`, already ported). A world of several maps is the port's own (MD8).

## How it was proved

- `openmx/tests/world_joins.rs` (3 tests in the gate, 1 ignored):
  - **`holes_are_found_exactly_and_the_ridge_world_has_none`** (no disc). A generated two-tile world,
    laid as a ridge cut wants it, has no holes. With its join one root cell wide instead, `holes()`
    gives exactly `[567, 0, 576, 567]`, and a 20-ft grid of `hole` calls misses it.
  - **`the_ridge_world_loads_with_no_holes_and_without_the_cut_shape`**: the shell's
    `world holes: none` line on that world.
  - **`a_ride_across_the_ridge_is_thrown_only_off_the_outer_edge`**. Two generated tiles, each with an
    MXU-style edge (valley, ridge, plateau beyond). Tile `a` places the disc's
    `free06_container.col` on its plateau, outside its keep, on the join's ground and on the ride's
    line. The rides:
    - From x 200 east: on the ground over the ridge top (>3 ticks on it, never >6 ft below the
      expected ground), through the container's place, thrown on tick 248 at x 849.3 (999 − 150).
    - From x 799 west: thrown on tick 249 at x 147.8.

    **Sensitivity:** with the `scene.collision.retain` line commented out, the same test fails:
    `from x 200: never thrown; first wreck true on tick 154 at (494.9, 282.907)`, so the bike hit the
    container. The disc was present and the rides ran; checked by riding the generated mods folder by
    hand with the same flags.
- **`world_mxu_free_throws_only_off_its_outer_edge`** (ignored, needs `OPENMX_WORLDS`). Run on
  world_mxu_free built by openmx-mods stitch at HEAD (C11), 21168 x 14256 ft:
  - `world holes: none`.
  - 10 crossings from 150 ft inside one tile's keep to 150 ft inside its neighbour's, both ways over
    06↔10, 10↔07, 07↔09, 10↔11 and 11↔09. All **reached**, none thrown.
  - All four outer sides **thrown** on tick 112 at the 150-ft margin (149 / 21018 / 149 / 14106).
- **By eye**, `--shot-pose`, DISPLAY=:0, 1920x1080 shrunk to 800, not committed. From 1300 ft west of the
  free06/free10 join, 1100 ft up, yaw 70, pitch −28:
  - HEAD's berm-cut world shows a wide flat plateau between two ridge lines.
  - The ridge-cut prototype shows one broad ridge with free10's valley right behind it.
- `./kit check --all`: TESTS PASSED.

## Not verified / not 1:1

- **The shipped world_mxu_free is not cut at its ridges yet.** That is openmx-mods M23. Until it lands,
  a stitched world still meets across two plateaus. The one-ridge shot is from my scratch prototype,
  whose 39 outer-band holes and 420-ft ridge top M23 must fix.
- **The world_mxu_free rides are an ignored test.** Its world is built from the user's disc by
  openmx-mods and is not in this repo or its context, so the gate does not run it. It was run by hand
  once, on stitch HEAD's build, as above. It was not run on the prototype, which would fail on its
  39 holes.
- **openmx-mods' `out/mods/tracks/world_mxu_free` is stale** (84 holes, pre-C11). Not rebuilt; it is
  that repo's output, and M23 says to rebuild it.
- **A shape or model is cut by its origin.** One whose origin is inside the keep but whose body
  reaches over the line is kept whole. One whose origin is outside but which reaches in is cut whole.
- **Owner numbers of a tile's kept shapes** now count only the kept shapes, so they can differ from the
  tile ridden alone when something was cut. On HEAD stitch's world the shell cut no model (its drop
  list covers the berm band). Cut collision shapes are not printed, so they were not counted. On the prototype, 11 of free06's models were cut that stitch's drop list missed.
- **Holes are found at cut-cell midpoints.** An owned edge is exact in F, but a piece whose `at`,
  spacing and sample give edges that round differently in the probes' sums was not looked for.
- **Only the free06/free10 join was shot**, and only from one pose.

## Next

Nothing in openmx. openmx-mods M23, then rerun
`OPENMX_WORLDS=<mods> ./kit check -- -p openmx --test world_joins -- --ignored --nocapture world_mxu_free`
on its rebuilt world.
