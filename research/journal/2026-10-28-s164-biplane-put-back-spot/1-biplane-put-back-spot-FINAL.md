# 1 — biplane put back spot (FINAL)

Task S164. 2026-10-28.

## What I did

- `game_sim::flight::Flight::spawn` now holds the original's spawn transform: the **body's** spot (the
  centre of gravity) and the nose, as the plane's +0xe10.. does — not the model's origin. Its type is
  unchanged, so the snapshot wire and every tick hash stay as they were.
- `Flight::place` / `Flight::reset` still take a model-origin spot (every port-found spot: switch, marker,
  start line, landing) and turn it into the body's (`origin + turn(cofg)`), which puts the plane exactly
  where it stood before. `Flight::place_body` / `Flight::reset_body` take the body's spot itself, for the
  original's bay; `Flight::spawn_origin` gives the model origin back for the shell's `Spawn` round trips.
- `flight::spawn_lift(psg)` reads the plane model's node tree and returns the merged bounds' centre y
  (`prop_world::node_bounds`, S130's port of the same routine); `flight::bay_spot(ground, lift)` is the
  original's spot: the ground point under the file Position, y + lift, in its float order.
- Shell: `Lot::lift` (read in main.rs from the machine archive's `biplane\anim\biplane.psg`), carried to
  `Plane::lift` for a plane standing in one of `machine.txt`'s own bays. `Plane::step`'s first placement at
  the bay probes the world's ground under it and resets the flight onto `bay_spot`; elsewhere, or with no
  ground, the old model-origin placement. The four `Spawn { at: f.spawn.0, .. }` round trips and
  `Plane::around` use `spawn_origin()`.
- `plane_crash_pilot.rs`: `the_plane` places the plane the original's way (free06 terrain ground under
  `[Biplane1] Position`, lift off the disc's model) and `each_wreck_is_put_back_after_151_ticks` asserts
  the spot bit for bit.

## What the original does

`./kit fn 0x0020f6e0` (machine_placement) reads `[Biplane1] Position`, `Direction` and `SetGroundHeight`
(default 1). With it set, `./kit fn 0x001dc960` (ground_query) moves the point onto the ground, x and z
kept; the normal is the spawn's up either way. `./kit fn 0x0020eb10` (vehicle_spawn) for kind 1 calls
`0x001b44b0` on the model node, which hands back the node's merged bounds (+0xe0 centre, +0xf0 half,
built by `0x001b3f50` over every child), and adds the centre's y to the point. `vt(+0x88)+0xe0`
(`0x0015a180`) stores (pos, dir, up) at +0xe0/+0xf0/+0x100, which are copied to the spawn +0xe10.., and
`vt +0xe4` (`0x0015a1b0`, physics_object_place) puts the **body** position on pos with the orientation
from dir and up. `0x00355b00` (mxbiplane_reset, the wreck recovery and the chord) goes back through
`0x00351df0 -> 0x0015a030 -> vt +0xe4` on the same three. Notes in research/oracle.txt under
machine_placement.

free06: ground under (2304, 135, 2281) is 134.9986 (0x4306ffa4), the psg tree's merged centre y is
3.6154737 (the root's own is 3.6148021; the rudder's box reaches above it), and F(ground) + F(lift) =
138.61406 (0x430a9d33): S143's captured body y exactly. Body (2304, 138.614, 2281); model origin
(2304, 135.114, 2278.5). The port stood the model origin on the Position (body 2304, 138.5, 2283.5).

## How it was proved

S143's three PINE crash captures (`context/capture/s143/{nose,wing,belly}.csv`, from s1):
`crates/game-sim/tests/plane_crash_pilot.rs::each_wreck_is_put_back_after_151_ticks` puts the body back
at the game's spot bit for bit (nose and wing; the belly's row is read a tick late and is held to 0.01 ft),
and the tick after it within 0.001 ft (nose and belly 0, wing 0.0007). The spot itself comes from the
disc's model and free06's terrain, no constant. `crates/game-sim/tests/flight.rs`'s chord test re-pinned
to the spawn `place` stores. The online tests that read `f.spawn.0` as the origin (lobby.rs,
machines_everywhere.rs, netplay.rs) read `spawn_origin()`; netplay's three hold it to a rounding (1e-4 ft)
because origin → body → origin is not exact in floats. `./kit check --all` green (2825).

## Not verified / not 1:1

- The spawn's up: the original's is the ground normal under the bay; the port's is world up. Free06's bay
  is flat (normal 0, 0.99999994, 0), so the capture cannot tell; a bay on a slope would sit level in the
  port. Filed as a follow-up.
- `SetGroundHeight = 0` (spot kept at the file's height, still lifted) is not ported: the port always puts
  a file bay on the ground. No disc track was checked for a 0.
- The parked (not yet taken) plane is drawn with its model origin on the file Position, as before; it moves
  to the original's spot (2.5 ft along, 0.11 ft up on free06) on the step that first flies it. Not
  compared with a capture of the parked plane.
- Online seats (`start_lane`, `online_bays`) and every port-found spot (beside-bay copies past the file's
  two, start-line spots, worlds' moved bays use the lift only where the bay is `original`) keep the
  model-origin placement: there is no original to match there. World bays are marked `original` and so
  take the lift on the world's ground: the original rule at a spot the original never has.
- The lift is read from `biplane.psg` for every livery; `biplane2`'s root matches, its children were not
  compared.
- The bounds are built with the root node at its own local transform (unplaced), which is what reproduces
  the capture; the original's node may be placed at that moment — it matched bit for bit, so it is either
  that or equivalent at free06's spot only.
- Only free06 was measured; the second bay ([Biplane2]) was not captured.
- The wing capture's tick after the put-back is 0.0007 ft off (stick held full over through the reset);
  not chased.

## Next

S204 (filed): the spawn's up as the ground normal, SetGroundHeight = 0, online bays.
