# 1 — world map pictures (FINAL)

Task F28. 2026-11-16.

## What I did

- `crates/openmx/src/track_previews.rs`: a world mod with no picture of its own (no `previews/track.png`,
  no kept still) now goes to the stills job like a track, but the job draws it a map instead of running
  a shell. The map is kept as `mods/previews/<id>.map.png` and read back by the next `gather`.
- The map (`map_picture`): 512×288, north (+z) up and east (+x) right like the HUD's map, the world
  fitted in and centred. Each pixel is the ground's height at its middle on the piece that owns it
  (`Tiled::owner`, with a source that reads nothing), interpolated across the cell. Coloured green →
  tan → pale over the heights found, shaded as lit from the north-west (relief ×4). Off the ground it is
  the card's dark colour.
- Memory: the pixels are grouped by owning piece first. Then each piece's terrain file is read, sampled
  and dropped before the next. Only pieces that own a pixel are read. No GPU ground, no stand-in, no
  second world.
- Rides: the job waits while a ride loads or runs, as stills do. A ride that starts mid-map stops it
  between two tiles' reads (`Shot::Ride`), and it is drawn again after. It also takes the one-at-a-time
  lock that stills use.
- The world's layout comes from a light reader (`map_pieces` + `terrain_shape`). It reads `mod.json`,
  each tile's `.scn` and each terrain's header and tree root. `worlds::find` could not be used:
  `track_previews.rs` is also mounted by `tests/track_mods.rs`, which can't mount `worlds.rs` (it pulls in
  audio, hud, menu, ground…). `terrain_shape` copies `worlds::shape_of_file`.
- `kept()` is unchanged on purpose. A store upload of a world still shoots a real still on its maker's
  machine (P282's `bake`) instead of baking the map.

## What the original does

Nothing: mod-world card pictures are the port's own menu feature. There is no original behaviour.

## How it was proved

- Test `track_previews::tests::a_world_with_no_picture_gets_a_map_and_no_child`: two generated tiles (the
  east one a hill). The stills job gets a stand-in shell that marks every run. The test checks:
  - nothing is drawn while a `Ride` is on;
  - after the ride, a 512×288 map comes back and nothing ran the shell;
  - the file is `<id>.map.png`, not a still;
  - the area off the ground is the background colour;
  - the hilltop is lighter than the flat tile, and the hill's NW side lighter than its SE side;
  - the next gather shows the map and takes nothing.
- I looked at the picture by eye: a flat green tile, and a lit hill on the east tile.
- `a_picture_is_kept_and_a_world_never_shot` was updated: a world with none is now taken, and a kept map
  counts as a picture for worlds only.

## Not verified / not 1:1

- Not tried on a real installed world such as world_ultra, so the time and peak memory there are not
  measured. Only the 2-tile test world was run. Reason: no big world was in this slot's mods folder, and
  the brief asks for the generated test.
- The seam joins of a wrapped world are made in memory by `worlds::close_seams`, and the light reader
  leaves them out. Their strips show as background on a wrapped world's map. Accepted for a card picture.
- Worlds whose tiles come off the disc (disc_worlds, P232) do not read with the folder-only reader. They
  get a log line and no picture, as before.
- Colours and shading are my own choice. Nothing in the original sets them.

## Next

Done.
