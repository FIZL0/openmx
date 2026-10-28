# 1 — flat maps are joins (FINAL)

Task MD56. 2026-10-28.

## What I did

Went looking for a tile that reads, draws or streams flat, in all five installed worlds
(`~/repos/openmx/mods/tracks/world_*`, built 2026-10-27 11:09–11:14). None does. The flat ground the
user rode is the **joins** between the maps, which the radar called a map:

- **The terrain files are the maps'.** Every MX vs. ATV and Untamed tile's terrain is byte-identical to
  its standalone mod, world_ultra's tiles to the per-game worlds', and world_mxu's terrains and scenes
  to the disc's `tracks\<name>.res`. Untamed's Nat_OR (4.499999 ft) is among them.
- **The simulation reads them as alone.** For every tile of every world, a 40 x 40 grid over its keep:
  `Tiled::at` in the world against `game_sim::world::Ground` on the tile's own terrain gives the same
  heights to the bit, and no point has no ground (`every_installed_worlds_tiles_read_as_alone`).
- **They are drawn as alone.** `piece_chunks`' corners match the tile's own ground (heights to 0.05 ft;
  at most ~10 of ~225,000 off, normals the same). The V92 stand-in is within 30 ft of the ground
  almost everywhere (65 ft at most, at one nat17 corner).
- **Nothing cuts their relief.** No two tiles' keeps overlap. The keeps cut no placements. The drop lists
  only take ruts outside a ridge, camera-following skies and weather, edge water, border redwoods,
  nat_ut's border rocks and nat14's fence at 0,0. Every map's paint pages fit the GPU.
- **They stream in.** A `--world-sweep` across world_ultra at plane speed builds each piece as it comes
  within 6000 ft, with no "not drawn" or "has no ground". A sweep from world_untamed's spawn to
  nat_az, shot at its end with the stand-in on and off, differs in 0.4% of pixels.
- **They look like themselves.** Oblique per-tile shots of 67 tiles (all 23 of world_untamed, all 27 of
  world_mxvatv, 17 of world_mxu) all show their hills and track. A whole-world hillshade of
  `Tiled::at` for four worlds shows every tile's relief.

What is flat: the joins. openmx-mods' stitch writes a join as a Shepard blend of the maps' edges,
extended (`tools/stitch/src/build.rs` `blend`). Past a few hundred feet from any map that is smooth
ground. On a 100 ft grid over each world's extent, the share of ground past N feet from every map:

| world | on a map | join > 300 ft | > 1000 ft | > 3000 ft |
|---|---|---|---|---|
| world_mxu | 50% | 39% | 24% | 12% |
| world_mxu_free | 72% | 21% | 12% | 4% |
| world_mxvatv | 66% | 20% | 8% | 4% |
| world_untamed | 54% | 33% | 15% | 4% |
| world_ultra | 48% | 41% | 26% | 12% |

Since MD48 (merged 10-27 13:14, ten hours before the user's report at 23:19) that ground is painted
with the nearest maps' own paint, folded back and forth. And the radar (MD42) names "the nearest map"
anywhere on a join. Riding world_mxu at 30000,31000, on join j027_028 about 2000 ft from any map,
the shot shows an open plain in a map's colours with "Grandview" under the radar's arrow. That is a
map ridden flat, as far as the player can tell.

The fix here: `hud::OFF_MAP` (550 ft, the widest band stitch cuts off a map's edge: its 150 ft
shootoff plus a berm of up to 400). `MapNames::around_in` now names the map underfoot only within
that distance of its rectangle. Farther out the radar shows no name under the arrow. It still names
the maps each way, taken from the nearest map, so the way back to one is shown. Giving the joins
relief is stitch's job and needs the worlds rebuilt and reinstalled, so it is filed as **MD57**.

## What the original does

Nothing to compare: the original rides one map. Worlds and their joins are the port's (MD8 on).

## How it was proved

- `worlds.rs` `every_tile_of_a_world_reads_its_heights_as_alone` (in the gate): a generated world of
  three maps with hills of their own, at 9, 4.5 and 4.499999 ft a sample, each cut to a keep, plus a
  join. Every tile's simulated ground (1600 points each) is bit-equal to the map alone. Its drawn
  corners are within 0.05 ft. Shifting `Tiled::at`'s read by 4 ft reds it (1600 of 1600 points on all
  three tiles).
- `worlds.rs` `every_installed_worlds_tiles_read_as_alone` (ignored; it needs `$OPENMX_WORLDS`): the
  same check over all 77 + 27 + 27 + 23 + 5 tiles of the five installed worlds. It passed on
  2026-10-28 with `OPENMX_WORLDS=~/repos/openmx/mods`.
- `hud.rs` `a_rider_far_out_on_a_join_is_on_no_map`: within `OFF_MAP` of a map, and over a wrap
  seam's copy, it is named. 1 ft past, or 2000 ft out along x or z, nothing is named, and the
  neighbour along x still is. MD42/MD46's radar tests (ride_hud.rs, worlds.rs, hud.rs) pass unchanged.
- Shell shot, world_mxu `--trace-at 30000,31000,0`, before and after: "Grandview" under the arrow,
  then none, with Casa De Cobre / Norfolk / Ensenada / Cleveland still round the rim.

## Not verified / not 1:1

- That the joins are what the user meant is inferred, not confirmed by the user. Every tile measured
  matches its map alone. The report came after MD48 painted the joins like maps. Half of the two big
  worlds is join ground, and the radar named it after a map. If the user meant a particular map, it
  must be one where all of these pass: the height sweep, the drawn-corner check, a streamed sweep to
  it, and its oblique shot.
- 10 of world_mxu's 27 tiles were not shot one by one (the shot job was stopped once the joins were
  found). They are covered by the height and drawn-corner checks and the hillshade only.
- The cover texture (V92) counts each piece's whole terrain rectangle, not its keep. So where a
  not-yet-shown neighbour's rectangle overlaps a drawn tile, the coarse stand-in is drawn there too
  and can poke through in valleys. I saw it in no shot (0.4% of pixels on the swept nat_az shot), so I
  left it alone. Unproven either way at plane speed in the user's own window.
- 550 ft is stitch's widest edge cut, not each map's own. A map with a narrower berm keeps its name
  over a little join ground past its edge.
- The joins are still flat (MD57).

## Next

Land. MD57 gives the joins relief in openmx-mods' stitch.
