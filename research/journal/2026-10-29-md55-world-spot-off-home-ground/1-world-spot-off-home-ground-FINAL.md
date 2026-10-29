# 1 — world spot off home ground (FINAL)

Task MD55. 2026-10-29.

## What I did

- `vehicle::world_spots` took each spot's clearance as its file height minus the home tile's ground
  at the file place moved by the tile's offset. A home tile whose terrain does not reach that place
  (a mod tile smaller than the track its machine.txt is for, e.g. MD32's 1143-ft tile) answers the
  no-ground height (-1e6), so the clearance came out ~1e6 ft and the plane parked a million feet up.
- Now a home ground at or below `NO_GROUND` (-1e5, the module's existing threshold) gives the
  clearance `rest`, a new argument: the plane's height on its wheels. The spot then stands on the
  world's ground at its moved place, as `start_spots` stands a plane.
- `vehicle::wheels_height(&Flight)` is the tires-bottom rule `Plane::rest_height` already used, pulled
  out so main.rs can measure it before the lot (and the garage's plane) exist: main.rs reads the
  biplane .phy with seat 0's mods and passes that as `rest` (0 without the .phy).

## What the original does

Nothing to compare: the original has no worlds; `world_spots` is the port's own placement (MD24).

## How it was proved

- `vehicle::tests::a_spot_off_the_home_ground_stands_on_the_worlds_ground` (crates/openmx/src/vehicle.rs):
  a 1143-ft home tile at 5000,3000 that answers -1e6 off its square, a [Biplane1] at file 2000,40,1500
  (off the tile): the moved spot stands `rest` over the world's ground; a spot the tile covers keeps its
  own clearance. On the old code the first assert reads ~1e6.
- `a_world_parks_the_plane_beside_its_spawn` unchanged but for the new argument.

## Not verified / not 1:1

- Not run on MD32's actual test world in the shell: the unit test stands in for it (no shot or `--timings`
  run of a world with a small home tile was made).
- Every machine's spot off the home ground takes the plane's wheel height, helicopters and buggies too:
  only the plane is ported, so no other machine's rest height exists to use.
- A moved spot that itself has no world ground under it still lands near -1e6: outside this task (the
  world's spawn is on the world, and a lot spot beside it is too in every world seen).

## Next

Nothing.
