# 1 — world joins (PART)

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

**The ridge cut** is in openmx-mods' stitch (`find_keep`), so it was filed there as **M23** (landed 10-23 as 4e86b00). It was
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
- **`world_mxu_free_throws_only_off_its_outer_edge`** (ignored, needs `OPENMX_WORLDS`). Since
  2026-10-24 it asserts that every crossing reaches the next tile (no wreck, no stop short) and that all
  four outer sides throw. It was not in 10-23's version, which asked only "not thrown inside" and at
  least one side.
  - On 10-23 it ran on world_mxu_free built by stitch at C11 (berm cut): `world holes: none`, all 10
    crossings reached, all four sides thrown on tick 112.
  - On 10-24 it ran on the **ridge-cut** world, built by openmx-mods M23 (4e86b00; keep
    `[88,87,681,680]`, tiles at 0/5472/10944, joins 135 ft wide). The world is in openmx-mods-slots/s1's
    `out/mods`. The test passed:
    - `world holes: none`.
    - 10 crossings over 06↔10, 10↔07, 07↔09, 10↔11 and 11↔09, both ways, all **reached**, none
      thrown. For example, `mxu_free10 -> mxu_free06` reached in 366 ticks.
    - Off the sides, **thrown**:
      - west on tick 112 at x 150;
      - east on tick 323 at x 17131;
      - north on tick 111 at z 150;
      - south on tick 220 at z 11651.

      Each is 150 ft inside the world's 17280 x ~11800 ft edge.

    The numbers were read with a temporary panic, removed before the commit.
- **By eye**, `--shot-pose 4900,1100,3000,70,-28,60`, DISPLAY=:0, 1920x1080 shrunk to 800, not
  committed. The camera is 1300 ft west of the free06/free10 join.
  - On 10-24, on M23's ridge-cut world, it shows **one ridge** with free10's valley and its course right
    behind it. The `world holes: none` line was printed.
  - The same pose on openmx-mods' main `out/mods` (the stale pre-C11 berm-cut build, `world holes: 84
    inside`) shows a wide flat plateau between two ridge lines.
- `./kit check --all`: on 10-23 TESTS PASSED. On 10-24, rebased onto main bb13cfa6, the gate was red
  on two tests, neither MD27's:
  - `game-render --test whip_tricks` read a leftover `mods/tricks/mxvatv_tricks` from another task in
    this slot (10-23 19:36). Moved out of the worktree, it passes.
  - `game-sim --test replay` `the_dallas_rail_rides_replay_from_their_save_states`: `along:
    rider.ride.front.slip.locked agrees through tick 278, was 279`. It fails the same way on bare
    bb13cfa6, in a scratch worktree with s3's context linked in. Without S89's capture it skips and
    reads green, which is why other slots do not see it. Main is already red on P148's S119 (S68 and
    S58, since 1cdaee5a). Filed as a new S task beside it.

## Not verified / not 1:1

- **The world_mxu_free rides are an ignored test.** Its world is built from the user's disc by
  openmx-mods and is not in this repo or its context, so the gate does not run it. It was run by hand
  on M23's ridge-cut build, as above.
- **openmx-mods' main checkout `out/mods/tracks/world_mxu_free` is still stale** (84 holes, pre-C11,
  berm cut). M23 rebuilt the worlds only in its slot's `out/mods`. Not rebuilt here: it is that repo's
  output.
- **The ridge join is 135 ft of stitch's own ground between the two peaks.** Its shape is
  openmx-mods M23's, which reports dips of at most 6.7 ft. The port rides whatever ground the join
  holds, and nothing in openmx changes it.
- **A shape or model is cut by its origin.** One whose origin is inside the keep but whose body
  reaches over the line is kept whole. One whose origin is outside but which reaches in is cut whole.
- **Owner numbers of a tile's kept shapes** now count only the kept shapes, so they can differ from the
  tile ridden alone when something was cut. On HEAD stitch's world the shell cut no model (its drop
  list covers the berm band). Cut collision shapes are not printed, so they were not counted. On 10-23's prototype, 11 of free06's models were cut that stitch's drop list missed. On M23's ridge-cut build, none were (its drop list covers the ring past the ridge).
- **Holes are found at cut-cell midpoints.** An owned edge is exact in F, but a piece whose `at`,
  spacing and sample give edges that round differently in the probes' sums was not looked for.
- **Only the free06/free10 join was shot**, and only from one pose.

## Next

MD27's work is complete: openmx-mods M23 landed the ridge cut, and this side was run against it.

It is not ticked. `./kit done` gates on the full suite, and main is red (S119 and the new Dallas S
task). The work is committed on task/MD27 and pushed by hand. Once those land, rerun
`./kit done MD27 "<msg>"` to tick.
