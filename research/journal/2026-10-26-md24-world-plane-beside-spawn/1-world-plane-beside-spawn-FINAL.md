# 1 — world plane beside spawn (FINAL)

Task MD24. 2026-10-26.

## What I did

- The bug: a world (a world mod such as `world_ultra`, or the spliced `free_all`) reads its home tile's
  `tracks\free06\machine.txt` from the disc's machine archive, whose coordinates are free06's own feet.
  The lot parked them unmoved, so on `world_ultra` (home tile at 91008, 18720) the plane stood at
  2304, 135, 2281 — tiles away from the spawn at 93892, 338, 21470. The lot's ground probe had the same
  fault: it was the home tile's terrain alone, in its own feet.
- `vehicle::world_spots` (new): the file's spots for every machine, moved as one by the step that stands
  `[Biplane1]` one `Machine::spacing` (40 ft) to the right of the world's spawn (across its heading,
  flattened, the same side `park` steps a copy to). Every other spot keeps its place relative to it, its
  facing, and its clearance over its own ground — measured, before the move, at the spot plus the home
  tile's offset, which is where the home tile's ground actually lies in the world.
- `Lot` now keeps the spots it parks around (`Lot::around`) instead of the file's bytes, so a world's
  moved spots survive `resize`; `Lot::new` is unchanged for a track. `park` is left (the docs name its
  rule) but unused, so it carries an `allow(dead_code)`.
- `main.rs`: `line_start` is computed before the lot (moved up, unchanged); the lot and the online
  seats' bays go through `world_spots` in a world; both probe the ground through `lot_ground`, which
  asks the world's `Tiled` when there is one.
- 40 ft is past the switch offer's 30 ft range, so a rider at the spawn is not offered the plane on the
  first tick; they ride to it.
- Nothing in opentennis parks machines; nothing taken from it.

## What the original does

Nothing to port: the original has no worlds. On its own track the plane stands where `machine.txt` says,
and that is unchanged (free06 still prints `parked by tracks\free06\machine.txt at 2304, 135, 2281`).

## How it was proved

- Unit test `vehicle::tests::a_world_parks_the_plane_beside_its_spawn`: a home tile offset 5000, 3000 on a
  sloped ground, a spawn facing +x: every machine's spots move by one exact step, keep their facing and
  their clearance, the first plane stands exactly 40 ft right of the spawn, and an empty file stays empty.
- Shell test `spliced_maps.rs::the_parked_plane_stands_beside_the_worlds_spawn` on `free_all` (disc only):
  the parked line is 40 ft to the right of the start line and on its ground. Before the fix it printed
  2304, 135, 2281 against a start at 2884, 338, 2750 and would fail.
- Live runs of the rebuilt shell (`--headless --frames 2`, real mods folder):
  - free06 (no world): 2304, 135, 2281, as before.
  - free_all: plane 2844, 327, 2750; start 2884, 338, 2750 facing 0.
  - world_ultra: plane 93852, 327, 21470; start 93892, 338, 21470 (was 2304, 135, 2281; with the world
    move but the old home-tile probe it stood at y 135, buried — hence `lot_ground`).
  - world_mxu_free: plane 3132, 327, 3038; start 3172, 338, 3038.

## Not verified / not 1:1

- Where the plane stands in a world is the port's own choice (the original has no worlds): one spacing to
  the spawn's right. Nothing checks that spot is clear of props, trees or a building; on the worlds run it
  stood on open dirt by the start height (327 vs 338), not looked at in a picture.
- The helicopter, buggy, monster truck and baja are moved with the plane, keeping free06's layout around
  it; they are not stood up (not ported), so whether that layout lands any of them on the spawn or in a
  wall was not looked at.
- The clearance assumes the home tile's ground in the world is the tile's own ground at the spot plus its
  offset; a spot the home tile's `keep` cuts away is measured on whatever ground the world has there.
- Online seats' bays use the same moved spots; not ridden online in a world.

## Next

Nothing: the task is done.
