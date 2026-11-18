# 1 — kiowa put back ground snap (FINAL)

Task MD214. 2026-11-18.

## What I did

- `crates/game-sim/src/kiowa.rs`: `Wreckage::put_back_on(ride, owned, ground)` and `Wreckage::act_on(…, ground)`.
  With `mxvatv` on and a ground (`atv_ground::Terrain`), the spawn goes through `snap_mxvatv` before the
  place: the ground query writes the point's `y` and the normal, the point moves `xyz` by normal × 0.3
  (`0x3e99999a`), the nose stays the spawn's, the up is the whole normal quad. `put_back` / `act` are the
  same with no ground (unchanged behaviour, MD199's oracle still passes).
- `crates/openmx/src/vehicle.rs`: an MX vs. ATV helicopter (`KiowaRide::mxvatv` set) is built with
  `wreckage.mxvatv = true`, its place and spawn the model's origin where it was taken (not the body's
  centre, which is origin + turned cofg). The crash's put-back passes the world's `mxvatv_terrain()`, so
  over MX vs. ATV's terrain it snaps; over any other world (no terrain) it does not, as the original with
  `+0x3b4` 0.
- Took nothing from ../opentennis (no helicopter there).

## What the original does

- Placement `./kit dis 0x004a6f70 --len 0x140 -p atv` (read again here): `+0x50 = +0xd40`; with a1 and
  `+0x3b4` set: `0x004833c0(ground, +0x50, sp+80, 0, 0)` (writes the point's y in place, normal out),
  `vmulx.xyz` normal × 0.3, `vadd.xyz` onto `+0x50` (w kept), `+0x60 = +0xd50`, `+0x70 = ` the normal quad.
  Else `+0x60/+0x70 = +0xd50/+0xd60`. Then the ground reset stands the body there (MD199's
  `place_mxvatv`).

## How it was proved

- Function oracle, 10,000 generated cases, bit-exact:
  `crates/game-sim/tests/mxvatv_kiowa.rs` `the_mxvatv_put_back_snaps_to_the_ground_as_the_original`. MD80a's
  `kiowa` image with its ground left in (Desert Oasis's terrain object at `*(heli+0x3b4)`), the query run
  as itself in the interpreter; the port's `Terrain` built over `MC02.ps2.trn` (MD37a's `mc02.res`) with
  the image object's words. Spawns mostly over the map, 20% past or on the box's edges, any directions,
  owner bit both ways. Compared: calls out, altitude fraction, the three place quads, body centre, last
  centre, velocity, both orientations, rates. 8,920 of the 10,000 landed on a slope (up not straight).
  Checked it really ran (temporary panic showed the count line).
- `openmx` unit test `an_mxvatv_helicopter_puts_back_as_slus_21104_by_its_model_origin`: the flag and
  spawn as built, MX Unleashed's untouched.
- `./kit check -- -p game-sim --test mxvatv_kiowa` (11 pass), `--test kiowa` (19 pass).

## Not verified / not 1:1

- Where the original writes the spawn `+0xd40`, and what it holds for a helicopter taken mid-ride, is
  still not traced (as MD199). The port uses the model's origin where the switch put it (ground +
  `KIOWA_DROP` 0.25 under the player's spot); the snap then puts it back 0.3 along the normal from the
  ground there, not at the take height. Not compared against a live MX vs. ATV put-back.
- No live capture of a put-back in the ride: proven as a function only. The ride's wiring (terrain
  passed in `KiowaAround::crashed`) is checked by build and the unit test only.
- Where the query writes no normal (a terrain with no map), the original reads stack leftovers; the port
  uses the spawn's up. The game's MX vs. ATV terrain always has a map, so not reachable there.
- MX vs. ATV's helicopter flown in an MX Unleashed world (no SLUS terrain) is put back unsnapped at the
  spawn, as the original with no ground; the original never flies there, so nothing to compare.
- Inputs exclude NaN and denormals.

## Next

Nothing for this task.
