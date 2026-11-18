# 1 — cars parked on their tiles (FINAL)

Task S266. 2026-11-18.

## What I did

- New `vehicle::tile_cars`: a world's lot (the home tile's spots, as before) with each car's list
  replaced by its own track's tile's `machine.txt` spots, moved by that tile's `at` (x, z) only. The
  height is the file's own, since a tile keeps its heights in the world; `Lot::stand_cars` then puts
  the car on the ground as on the track. A car with no tile of its own keeps the home list.
- main.rs: in a world, each tile's `machine.txt` is the one in the folder its scene is stemmed
  (`tracks\free09\machine.txt` for FREE09), read off `machine.res`. The folder must equal the stem,
  so `find_suffix`'s loose ends-with match cannot hand a mod tile another track's file.
- Not limited to disc-origin tiles: the installed world_mxu_free reads its tiles from folders
  (`tiles/mxu_free09`), so a disc-only filter parked nothing.
- No code from ../opentennis: it has no vehicles or worlds.

## What the original does

Nothing to port: the original has no worlds. Each freeride track's own lot is S245's.

## How it was proved

- Unit test `a_world_parks_each_car_on_its_own_tile` (vehicle.rs): offset spots, file height kept,
  no baja from a file with none, no buggy with no tile, the fliers unchanged.
- New tests/world_parked_car.rs: world_mxu_free with `--trace-picture` parks buggy, monster and baja;
  each bay equals its track's bay alone plus the tile's `at`, same height (truck 20434.00 134.49
  3118.00 = free09's 4594.00 134.49 3838.00 + (15840, −720); baja and buggy likewise).
- Shot `target/tmp/s266-world-truck.png` from behind the truck's bay, looked at by eye: the truck
  stands on the ground among free09's sheds.
- bays, world_parked_plane, car_picture still pass.

## Not verified / not 1:1

- Where a car stands in a world is the port's own choice (the original has no worlds), so there is
  nothing to compare against the original. It is checked only against the port's own track bays.
- A car spot that falls outside its tile's `keep` would stand on a neighbour's or a join's ground.
  Not handled: the three disc spots are inside their tiles in world_mxu_free (they stand on their
  own tracks' heights, which the test checks), and no other world has a car tile.
- Only world_mxu_free was ridden. world_mxu, world_ultra and the disc worlds were not ridden.
- Taking the car at its world bay and driving it was not run. It uses S245's same bay path.

## Next

-
