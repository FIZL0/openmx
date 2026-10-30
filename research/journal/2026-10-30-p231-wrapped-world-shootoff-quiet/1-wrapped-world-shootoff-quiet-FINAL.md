# 1 — wrapped world shootoff quiet (FINAL)

Task P231. 2026-10-30.

## What I did

- `audio::edge_distance` takes the world's wrap (`[x, z]`): a wrapped axis has seams, no edge, and
  is left out of the distance; `None` when no unwrapped axis is within `EDGE_WARNING`. Unwrapped it is
  the original's four-edge query as before (its tests now pass `[false, false]`).
- `audio::hole_distance`: a world's holes (MD8) asked the way the throw asks them. `World::off_world`
  is a yes/no at a margin (the point and the four points `m` feet away along the axes), so the
  distance is the least margin, searched every `HOLE_STEP` = 5 ft from 0 to 345, at which it reaches
  a hole. At the throw's own margin (150) the music is out and the rumble full, as at the outer edge.
- `audio::world_edge(area, land, at)`: the nearer of the two, the whole query. `main.rs` `seat_sound`
  now asks it of the chassis' centre with the ride's `Land` (it only asks on an armed bike now; an
  unarmed one's `EdgeSound::tick` ignored the edge anyway).
- Checked which of the throw's two halves the fade needed: both. The throw
  (`game-sim/src/rider.rs`) is `holed || outside_wrapped(area, wrap, bone, cofg)`; the sound asks the
  same two of the chassis' centre (the original's sound asks the chassis only, P85).
- `--trace` rides print `shootoff warning: tick N at x, z, d ft from the edge` the first tick the
  seat's sound would start the fade, so a shell ride can prove it.
- Nothing taken from opentennis: it has no map edge.

## What the original does

The original has no wrapped worlds or holes: its sound child of the bike asks the chassis' distance
to the map rectangle's nearest edge (P85). Wraps (MD29) and holes (MD8) are additions; this task
makes the warning agree with where the port's throw fires on them.

## How it was proved

- `tests/audio.rs` `a_wrapped_world_asks_no_edge_at_its_seams`: on an xz-wrapped free09-sized area,
  `None` over a grid through every seam, the corners and points past them; x-only and z-only wraps ask
  the other axis' edges alone; unwrapped corners still answer. A ride along x over the seam at 60 ft a
  tick through `EdgeSound` keeps `Mixer::music_edge` `None` and starts no rumble; the same ride on the
  unwrapped map fades (control).
- `a_hole_is_asked_by_the_margin_that_reaches_it`: a modelled hole, distances 345/200/150/0 and none.
- `tests/stitched_seams.rs` `a_ride_over_the_wrap_never_warns_of_an_edge`: the shell rides MD47's
  generated wrapped world east 900 ticks, over the wrap twice (x 1267 → 67, 1267 → 317), no warning
  line; the unwrapped copy warns on tick 0 (198.5 ft from its z edge). Needs the disc; ran here.
- `world_mxu_free_over_its_x_seam_never_warns` (ignored: the installed world from openmx-mods and the
  disc) run 2026-10-30 on `~/repos/openmx/mods`:
  ```
  z 2984: from x 17115 to 1067, over the seam true, no warning
  z 5967: from x 17115 to 952, over the seam true, no warning
  z 8950: from x 17115 to 1778, over the seam true, no warning
  ```

## Not verified / not 1:1

- The hole distance is a 5-ft search of `off_world`'s five-point margin test, not a true distance to
  the hole: a hole narrower than 5 ft between two probes along an axis, or one met only diagonally,
  is found late or not until the throw's own test reaches it. world_mxu_free and every installed world
  report no holes, so no real ride was heard against a hole; only the modelled hole of the unit test.
- The z seam of world_mxu_free was not ridden in the shell (only x, three rows); the z axis is the
  same code, covered by the unit test's grid.
- The fade on a real outer edge of an unwrapped world's axis is the original's query unchanged and
  was not re-captured; the existing P85 replay tests still pass on it.

## Next

None.
