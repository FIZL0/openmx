# 1 — tile alone edge (PART)

Task MD33. 2026-10-28.

## What I did

- **Decision: `Ground` caps the field and the edge at the tree's own samples**, rather than
  `tiles::generate` telling the tests how to write the scene. The scene's `TerrainWidth` counts
  256-sample units on a lone track (MD12) and root cells in a world's file (`Tile::samples`). For a
  one-level tree of N < 16 cells no integer `TerrainWidth` gives N × 16 samples, so "write
  `TerrainWidth=1`" only works at N = 16. The cap works whatever the scene says, and a tile's lone
  `Ground` (the one `Tiled::with_ground` builds) now agrees with `Tile::samples`.
- `Terrain::samples()` (track.rs): the last root cell present on each axis, plus one, times 16 per
  level below the root.
- `world::Ground::new`: `samples_x/z = min(scene × 256, tree)` on each axis. Where the scene's count
  stands, the area is still `Area::new(F(width), …)`, the same words as before. Where the tree is
  shorter, the unit count is `tree / 256` (exact in binary), so the area is `tree × scale`.
- The shell's `ground::Ground::new` takes the same cap for drawing. `Pages::of` gets fewer leaves
  only where the tree has none, and those leaves have no page.
- Tests:
  - `game-sim/tests/ground_cap.rs`: an 8 × 5 tile's field and edge are 128 × 80 samples. A full
    16 × 16 one-level tree under `TerrainWidth=1` keeps `Area::new`'s bits at four spacings.
  - `game-render/tests/terrain_tree_extent.rs`: all **57** disc maps' trees are exactly their
    scene's width × 256. I checked the count with a probe panic, because a pass hides skips.
  - `openmx/tests/generated_tile_alone.rs`: a 12 × 12 tile at 4 ft (764 ft) under
    `TerrainWidth=12`, ridden off all four sides through the shell. **Red before the fix**
    (0°: tick 164 off the map at z 787; by hand at 90°, y −3,831 by tick 1100 and still falling),
    green after.
- `tiled_world.rs`: `the_tile_at_the_origin_rides_as_it_does_alone` and
  `a_tile_away_from_the_origin_parts_from_itself_alone` compared the world with `md8_a` / `md8_b`
  ridden alone, and leaned on the lone tile having no edge. Both 576-ft tiles now throw at
  ≈ 426 ft, inside the stretch they compared (a: tick 121 at x 427.9). Both now stop at the lone
  ride's first wreck. The origin test still compares 120 ticks to the bit.
- **openmx-mods' tracks** (a scratch survey over every `.scn` in `~/repos/openmx-mods/mods/tracks`
  and `~/repos/openmx/mods`, 472 scenes): 459 have a tree the scene's size, so the cap does nothing
  there. That includes all four `elev_*` real-elevation tracks (glamis, hatfield, littlesahara,
  ocotillo; `TerrainWidth=8`) and every world tile from the disc. The cap bites on three imported
  maps, here and as world tiles:
  - MX vs. ATV sx07: scene 256 × 512, tree 256 × 256.
  - MX vs. ATV nat04 and Untamed sm_hs: scene 1280 × 1280, tree 1024 × 1280.

  Their edge moves in to their last ground. Before, the edge was past the ground, and a bike that
  rode out there fell.
- First try at the ride test: an 8 × 8 tile at 2 ft (254 ft). It passed both before and after the
  fix, because `off_map::MARGIN` is 150 ft: every spot on a map under 300 ft throws at once (wreck
  on tick 0, then the throw holds the bike). That map was too small to prove anything, so the test
  map is 764 ft.

Taken from opentennis: nothing; it has no terrain edge.

## What the original does

The edge test's area is `256 × TerrainWidth × TerrainWidthScale` (S73; `biplane::Area::new`), from
the scene alone. The original never looks at the tree for it. Its disc maps' trees always match their
scenes (57 of 57), so the original never meets a tree smaller than its scene.

## How it was proved

- A save-state-free static check: all 57 disc maps, `terrain_tree_extent.rs`. On every disc map the
  cap is a no-op, so the edge, the field and every ground answer are unchanged there.
- `ground_cap.rs`: `Area::new`'s bits are kept on a full tree.
- `generated_tile_alone.rs`: red before, green after, through the shell's own binary.

## Not verified / not 1:1

- **The cap is the port's own.** The original reads the edge from the scene alone. That is proved a
  no-op on the 57 MX Unleashed disc maps. On sx07, nat04 and sm_hs it moves the edge in compared with
  what MXU's rule would put there. I did not check what MX vs. ATV or Untamed themselves do past those
  maps' trees. Their exes were not read for this, since the maps are mods and MXU's rule does not
  bind them.
- The openmx-mods survey was a one-off scratch program, not a test, because the gate has no fixed
  path to openmx-mods. A mod added later with a tree smaller than its scene is not caught. It gets
  the capped edge, which is the intended behaviour.
- I did not ride sx07, nat04 or sm_hs to their new edges. Only the numbers were compared.

## Next

-
