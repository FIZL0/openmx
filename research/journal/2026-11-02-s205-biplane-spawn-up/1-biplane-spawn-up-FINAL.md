# 1 — biplane spawn up (FINAL)

Task S205. 2026-11-02.

## What I did

- `game_sim::flight::Flight` has a `spawn_up`: the third of the original's spawn transform (position,
  nose, up). `Flight::place_body` / `Flight::reset_body` take it (a bay's ground normal); `place` /
  `reset` (the port's own spots) set the world's up. `fly_as` and `launch` keep it with the spawn. It
  is on the snapshot wire after `spawn` (16 bytes more a plane); it is not in the state hash, as the
  spawn is not.
- `flight::spawn_axes(facing, up)` is the original's place's squaring, and `flight::spawn_turn` is it
  followed by `axes_to_quat` (`mat4_to_quat` of `basis(0, nose, up, false, true)`). It replaces the
  old `spawn_turn`, which was `basis(at, facing, world up, true, false)`: the old one kept the up and
  flattened the nose; the original keeps the nose and moves the up. `put` (every placement), `body_at`
  and `spawn_origin` use it with `spawn_up`.
- The original's reset squares its stored spawn in place, the first placement squares copies: `put`
  takes `keep`, true from `unwreck` (wreck recovery, the chord, `reset`/`reset_body`), false from
  `place`/`place_body`, and with it writes the squared nose and up back into `spawn.1` / `spawn_up`.
- Shell (`crates/openmx/src/vehicle.rs`): `Plane::step_met`'s first placement at a file bay passes the
  ground query's normal to `reset_body`; `vehicle::ground_heights` reads each bay's `SetGroundHeight`
  (`Section::flag`, default on, the same `T/F/ON/OFF/Y/N`/integer reading as the original's ini
  bool); `Lot::ground_heights` (filled by `Lot::new` and in main.rs for worlds) goes to
  `Plane::ground_height`, and with it off the bay's spot keeps the file's height (still lifted).
  `Spawn::biplane`'s doc no longer says it is ignored.
- `plane_crash_pilot.rs::the_plane` passes free06's ground normal.

## What the original does

`./kit fn 0x0020f6e0` (machine_placement): `SetGroundHeight` (ini bool, default 1). On: ground_query
(`0x001dc960`) on the Position itself (y moved onto the ground); off: on a copy, the Position kept. The
normal (`auStack_a40`) goes to `./kit fn 0x0020eb10` (vehicle_spawn) as its up in both cases.
vehicle_spawn stores the three raw through vt +0xe0 (`./kit dis 0x0015a180`: three `lq/sq` to
+0x90/+0xa0/+0xb0 of the physics wrapper) and then calls vt +0xe4 with its own argument copies.

`./kit dis 0x0015a1b0` (physics_object_place): `side = vopmsub(vopmula(look, up), up, look)`, then
`up = vopmsub(vopmula(side, look), look, side)` stored through the up pointer (`.xyz`, so its w is the
look's), then look and up each `vmul/vadday/vmaddz` + `vrsqrt` + `vmulq.xyz`, written back; `pos.w = 1`;
`axes_to_quat` (`0x0013f9f0`) on them into sp+128, and that quaternion places the body. The reset
(`0x00351df0 -> 0x0015a030`) calls vt +0xe4 on the wrapper's stored +0x90/+0xa0/+0xb0, so after the
first reset the stored nose and up are the squared ones.

What it means for a slope: the nose stays the file's Direction; a normal leaning across the nose rolls
the wings with the ground; one leaning along it is taken off (the up stands straight). The plane never
pitches with a bay's slope.

The disc (`crates/openmx/tests/bay_up.rs::the_discs_biplane_bays_are_flat`, and a one-off listing of
every section of every freeride `machine.txt`): every `[Biplane1]`/`[Biplane2]` on free06, 07, 09, 10
and 11 is on flat ground, normal (0, 0.99999994, 0); no biplane section has `SetGroundHeight`. The one
section on the disc that has it is free06's `[Machine1]` (a bike, `FALSE`).

## How it was proved

- **Oracle, 10,000 inputs**: `crates/game-sim/tests/spawn_axes.rs::a_spawn_is_squared_as_the_originals_place_squares_it`
  calls the original's physics_object_place in `ps2-oracle` with a hook right after its
  `axes_to_quat` call (place + 0xb4) that copies the quaternion off its stack and ends the call, and
  compares the squared nose, the squared up (all four lanes each) and the quaternion bit for bit with
  `spawn_axes` / `spawn_turn`. Inputs: the disc's Directions, level noses of any length, ground
  normals up to ~60° off up with w 1 or 0.9999999, the flat ground's 0.99999994, world up, zero and
  f32::MAX vectors, and arbitrary ones. 0 differ. Checked that it bites: leaving the up unnormalised
  fails it on many inputs (reverted).
- `spawn_axes.rs::a_sloped_bay_rolls_the_wings_and_keeps_the_nose`: the across/along behaviour, and the
  flat-ground normal squares to exactly the world-up spawn.
- S143's captures (copied from s1's `context/capture/s143`): `plane_crash_pilot.rs` still passes —
  `each_wreck_is_put_back_after_151_ticks` holds the put-back's body position and `last_quat` bit for
  bit (nose, wing) with the ground's normal as the spawn's up and the original's squaring.
- `crates/openmx/tests/bay_up.rs`: a plane taken from a bay on a sloped `Flat` (normal 30° across the
  nose) gets `spawn_up` = the squared normal and its body on `bay_spot` of the ground; a bay with
  `SetGroundHeight = FALSE` keeps the file's height + lift and still takes the normal;
  `ground_heights` per bay and per lot copy; the disc's bays are flat.
- `./kit check -- -p game-sim --test plane_crash_pilot --test flight --test flight_collision` (20
  passed); `-p openmx --test lobby --test netplay --test machines_everywhere --test machines --test
  teleport_air --test bays --test switch_trick` (316 passed); `--test bay_up` (12 passed, none
  skipped).

## Not verified / not 1:1

- **No capture of a sloped bay**: there is none on the disc for the plane, so the slope path is proved
  only through the oracle (the squaring and the quaternion) and the shell test (that the normal reaches
  it). Where the ground normal comes from is S64's port of the terrain query.
- **SetGroundHeight = 0** is not in any biplane section on the disc; ported from the decompilation
  only. Off the terrain (no ground under a bay) the original's normal is left unset; the port then
  falls back to the model-origin placement with world up.
- **Online seats** (`vehicle::start_lane`, `Online::bays`) still stand the model's origin on the bay with
  world up: filed as B5. Port-found spots (copies past the file's two, start lines, switches, worlds'
  bays that are not `original`) keep world up — there is no original to match there; world bays that
  are `original` take the world's ground normal, the original rule at a spot the original never has.
- The port-found spots now go through the original's squaring too (nose kept, up re-crossed) instead
  of the old flattening of the nose. Identical for a level nose; a spot handed a nose with a y
  component now pitches with it, as the original's place would. No disc case has one at a bay; not
  compared with anything else.
- The parked (untaken) plane is still drawn level with its model origin on the file Position (S164's
  note): the drawn bay does not show the slope until the plane is taken.
- `spawn_up` is not in the state hash (nor is the spawn): a ride differing only in where a wreck would
  put the plane back hashes the same until it does.

## Next

Nothing for this task. B5 for the online seats.
