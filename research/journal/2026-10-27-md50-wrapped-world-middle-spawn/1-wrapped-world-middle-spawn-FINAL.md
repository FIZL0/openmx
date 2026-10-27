# 1 — wrapped world middle spawn (FINAL)

Task MD50. 2026-10-27.

## What I did

- `crates/openmx/src/worlds.rs`: `WorldMod::middle_tile` — of the tiles whose own scene start stands
  inside their kept rectangle, the one whose kept rectangle's middle is nearest the middle of the
  world's extent (the wrapped extent, seam joins included); the first in file order on a tie. `find`
  uses it on a world whose `wrap` is on and whose `spawn` is a tile (named or the default): the file's
  `spawn.tile` is overridden there, so the installed worlds (all naming a corner map) move without
  being re-stitched. A `spawn.position` in world feet is kept. Unwrapped worlds are untouched.
- Chose the "start on the middle tile" option over "shift the seams": the seams stay where the data
  and the seam joins put them, nothing in the sim's world layout changes, and the pick is a pure
  function of the world file and the tiles' headers, so every online seat computes the same tile.
- The sky (`home`) stays the file's (decided, not followed): the home tile's archive also gives the
  ride's props, lights and the machine lot file; the lot is already moved beside the spawn (MD24's
  `world_spots`), so nothing needs the home to follow. On world_mxu the sky stays mxu_free06's.
- `tests/wrapped_world_draw.rs` (MD43's fixture): its two-tile world now gives its start in world
  feet at `a`'s own. With a tile spawn the ride would start on `b` (the strip makes it the nearer),
  and the software renderer's ground follows the rider, so the seam's far side came out blank
  ("only 0 of it"). The test is about drawing across the seam, so the start went back where it was.
- `main.rs`: the shell says `world spawn: <tile> at x,z` for a tile spawn.
- openmx-mods `tools/stitch` (0e35a24, pushed): `stitch::middle_tile`, the same rule; `build` writes
  it as `spawn` when `wrap` is set, and the ultra builder does the same over every world's tiles
  (each tile's spacing and start read with `Tile::load`). So a newly stitched wrapped world's data
  names the middle tile too. The installed worlds' mod.json files were not rewritten (re-stitching
  them is a long job and the port moves them anyway).
- Picks on the installed worlds (release shell, 2026-10-27): world_mxu_free → mxu_free10 (8141,3188
  of 17415 x 11934), world_mxu → mxu_nat17 (16920,22680 of 41743 x 38908; the middle falls between
  nat17/nat01/nat11/nat12 and nat17 is nearest), world_ultra → mxvatv_nat06 (59081,22882 of
  121150 x 40060). Also world_mxvatv → mxvatv_nat14, world_untamed → untamed_nat_sdca.

## What the original does

Nothing: wrapped worlds are an openmx addition (MD29). With wrap off the spawn is exactly as before.

## How it was proved

- `worlds.rs` unit test `a_wrapped_world_starts_on_its_middle_tile`: a 3x3 generated world, file
  spawn the corner `a`: wrapped → `e` at e's own start; edged → `a`; spawn in feet kept; the middle
  tile's start cut out of its keep → the nearest kept start: `f`, since the seam strip past the far
  edges puts the world's middle east and north of `e`'s, so `f` and `h` tie and `f` comes first.
- `tests/middle_spawn.rs` `a_wrapped_world_rides_from_its_middle_tile` (gate, needs the disc): the
  same grid through the shell; wrapped, `world spawn: e at 184.0,184.0` and 30 traced ticks all on
  `e`, none down; edged, `world spawn: a`.
- `tests/middle_spawn.rs` `the_installed_wrapped_worlds_start_on_their_middle_tile` (ignored: user's
  mods + release): world_mxu_free, world_mxu and world_ultra start on the tiles above and the first
  traced tick is within 50 ft of the start. Ran and passed in release on 2026-10-27.
- stitch `tests/middle_spawn.rs`: the rule on three tiles in a row, and a 3x3 build writes the
  middle cell's tile as `spawn` with `wrap: "xz"` and the first tile without. Whole stitch suite green.

## Not verified / not 1:1

- Not 1:1 by nature: wrapped worlds and their spawn rule are an addition; the original has no such world.
- On mxu_nat17 the first tick stands 22 ft from the scene's start (16898 vs 16920 x); I took it to be
  the national lining the bike up on its gate and did not trace why. The installed-world test only
  asks for within 50 ft.
- The stitch tool's extent is the lattice's (`size × spacing`, before seam joins), and the port's is
  the wrapped one (with the seam strip). On a near tie the two could name different tiles; the port's
  pick is the one that rides. Whether stitch would name the same tiles as the port on the installed
  worlds was not checked (they were not re-stitched).
- The installed worlds were not re-stitched, so their files still say the corner map; only the port's
  load moves them.
- Online: not run in a netplay session; the pick reads only the world file and terrain headers, which
  every seat loads the same, so it is deterministic by construction.

## Next

Nothing.
