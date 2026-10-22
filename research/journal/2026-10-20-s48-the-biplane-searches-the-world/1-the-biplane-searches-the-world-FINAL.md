# 1 — the biplane searches the world (FINAL)

Task S48. 2026-10-20 (blocked), 2026-10-21 (done). User, 2026-10-21: "the biplane has no collision at
all in the shell — it flies through the ground and everything; priority".

## Session 1, 2026-10-20: blocked

S41, S45, S46 and S47 had not landed on main (a2054d0), so there was no substep loop to call the search
from and no broad or narrow phase to build it on. I wrote no code and only read the hit listener (its
`this` is confirmed below).

## What I did (2026-10-21)

- `crates/game-sim/src/flight_search.rs` (new): the plane's collision search.
  - `Rays`: the ray probe out of `Biplane\col\biplanem.col` (36 segments, their nodes, the tree's root).
  - `reach`: the probe's bound, centre and radius, in the original's VU order.
  - `pack_normal` / `facets`: the loader's facet normals, packed into the i16 lanes.
  - `Scene`: `contact_search::Search` and `Narrow` over a `world::Props`. The probes sit in slots 0
    (ray) and 1 (sweep), and the props start at 2. The search runs on `world_search::search_nearby`
    with S46's `ray_probe::pair` and S47's `sweep_probe::pair` as the narrow phases.
- `world.rs`: `World::collision()`, the heightfield plus the meshes a ride collides with. `Place`
  answers its ground and its props; the default answers the heightfield and no meshes.
- `flight.rs`:
  - The plane owns a `Collider` and a `contact_search::Record`. They are built like the wreck bike's
    (`wreck_bike::collider` / `record`), with the plane's own collidable category, actors and
    collides-with off the state.
  - `with_rays` gives it its ray probe. Without one (old fixtures, a ride with no disc) the plane
    searches nothing, as before.
  - Each frame sets the record's node and clears it, then calls `contact_search::collide`
    (`Flight::search`), then `WORLD.step`. The force listener runs `contact::resolve` in
    `collision_force` every substep.
  - The crash test reads `touching` and the struck object from the record. A struck vehicle
    (category & 0x2000) does not wreck.
- `snapshot.rs`: the plane's wire carries the collider and the record, but not the ray geometry, which
  is taken back from the live sim on decode. Sim size 11160 (`tests/hash.rs`).
- `openmx/src/main.rs`: the shell loads `biplanem.col` off the disc and gives every flight its rays.
- `contact_search::Search::moved` now takes the node, so a search can follow the sweep's move;
  `wreck_bike::NoObjects` and the test fake follow.
- `tests/replay.rs`: the biplane acceptance run flies with the rays over the original's ground and the
  port's track (`WithTheTrack`). `BIPLANE_AGREES_THROUGH` goes from 0 to 22.

## What the original does

- **Hit listener**: `this` = plane+0x50, the body. On the state: this+0x58 = 1, this+0x38 = vtable
  0x0045b2ac, and vt +0x124 = 0x0015b790. That function does work only when the word +0x110 points at
  is 0 (an asleep body). It re-seats the +0x138 frames and re-enables the body. The global mask
  (0x00468f68) is 0x40, and every scenery mesh's category 0x48 has it, so the listener fires on any
  mesh hit, but on an awake body it does nothing. Ported as a no-op.
- **Sound listener**: presentation only, not ported.
- **Candidates**: the save state's nearby search lists 34 candidates:
  - 23 scenery meshes, category 0x48;
  - 3 bike collidables, category 0x9a048, each with one listener (bike_struck_listener);
  - 8 hierarchies, category 0x200020.
- **Facet normals**: the loader (0x001eded0) stores each one as the low 16 bits of
  `(n + 768.0)`'s float bits.
- **Crash mask**: the plane's is +0x580 = 0x2000 (VEHICLE). The crash test (0x00353180) spares a
  contact whose struck object's category meets it.

## How it was proved

`tests/flight_collision.rs`, over `biplane.p2m2_SaveState` and the disc:
- the collision object field for field;
- the ray probe against `biplanem.col` and its placed reach, bit for bit;
- `biplanef.col`'s packed normals facet for facet;
- the masks;
- three rides: nose-down into flat ground (it now crashes; before S48 it flew through), a take-off on
  its wheels (the rays touch nothing), and level into a column of meshes in the air (struck, crashed).

`tests/replay.rs`'s biplane run agrees through tick 22. The shell, flown on a hand-written tape into the
free06 ground nose-down, stops dead at the ground, wrecks and is reset to the start line.

## Not verified / not 1:1

- **The candidate list is approximated.** The props are taken in scene order and sorted by distance
  from the probe's centre less their radius. Both probes share the one list, gathered once at the
  search's start and not again after the sweep moves the plane. The original's tree query order was
  not reproduced.
- Hierarchies/groups (category 0x200020) are not built. Only the props' meshes are collidables.
- Other riders' bikes are not collidables, and bike_struck_listener is not ported: the bikes are not
  in `Props`.
- The hit listener is a no-op on the claim that a riding plane's body is always awake. The +0x110 word
  was not read off the state.
- The non-nearby path uses the same list as the nearby one.
- The ray probe's `ray_probe::Record` is rebuilt every frame, so anything it keeps between frames
  (nearest, flags) is not carried.
- Only `biplanem.col` is used, for every livery; `biplane2m.col` and the splitscreen files are not
  checked.
- The struck object's category is always SCENERY (0x48) in the crash test.
- The acceptance run parts at tick 23, the human's first press of the gas: every lane parts at once,
  with no contact or sweep before it. So it is the press (pad timing), not the collision.
- Over the port's own ground the run still parts at tick 1 (one ulp low, from before S48).
- The snapshot carries the record but not the ray geometry.
