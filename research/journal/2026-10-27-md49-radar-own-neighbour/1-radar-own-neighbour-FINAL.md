# 1 — radar own neighbour (FINAL)

Task MD49. 2026-10-27.

## What I did

- `crates/openmx/src/hud.rs` `MapNames::toward_in`: a map is never its own neighbour in any copy —
  the skip was `i == from && off == by` (only the copy the rider is on), now `i == from`. On
  world_mxu_free (3 by 2 tiles, wrapping xz, no tile north of Dallas = mxu_free06) the search looked
  past Dallas's north edge, found nothing but Dallas's own copy one world-height over at across-offset
  0, and ranked it first; now the nearest other map that way is named (ranked as before: across-offset,
  then distance past the edge), or none if there is none. Doc comment updated (it said "one map wide,
  the map itself").
- Applies to every wrapped world; a world that does not wrap only ever has the `[0, 0]` copy, so it is
  unchanged.
- Test `hud::tests::no_map_is_its_own_neighbour_on_a_wrapped_world`: world_mxu_free's layout (tile
  positions and keep rectangles from its mod.json, 5472-ft tiles kept 704..5448 ft), nine rider spots
  on each map, no way names the map itself, and Dallas has a name north and south. It fails on the old
  code (`Dallas ... names itself: [Some("Dallas"), Some("E"), Some("Dallas"), Some("C")]`) and passes
  now. MD43's `the_labels_name_the_map_across_the_seam` still passes.
- Note: hud.rs's unit tests are in the openmx **binary**, not the lib (web.rs): `./kit check -- -p
  openmx --lib hud` runs zero tests and still says PASSED; use `--bins`.

Nothing taken from opentennis: it has no radar or wrapped world.

## What the original does

Nothing: the original rides one map and its radar names no maps (MD42). This is the port's own.

## How it was proved

Unit test above, red before / green after. Port-only feature, no original to compare against.

## Not verified / not 1:1

- Which other map is named north/south of Dallas in the shipped world: the test checks it is some
  other map, not which one. With the keep margins (704 ft on the low side, 24 ft on the high side) the
  copy of mxu_free09 over the x seam can come nearer across than mxu_free11 for a rider in Dallas's
  middle; that is "the nearest other map" by the existing ranking, not checked in the game window.
- The test's wrap sides (3 × 5472, 2 × 5472 ft) are assumed from the tile grid, not read from the
  installed world's loaded `Wrap`; not run in the shell on world_mxu_free (no display check of the
  radar labels).

## Next

none
