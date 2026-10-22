# 1 — ragdoll wreck bike (FINAL)

Task S105. 2026-10-22.

## What I did

- New `crates/game-sim/src/ragdoll_scene.rs`:
  - `Bike` is `wreckbikef.col`'s mesh, with the loader's packed normals. `Bike::touch` is the sphere
    test against it in the bike's frame.
  - `Cells` is the track's tree as the broad phase lays it (`broad_build::tree_over`, least 96,
    reporting its cells' box). `Cells::key` is where the bike goes in it.
  - `BikeSearch` is the bones' one search, as far as the bike goes: rebuild or resort, the bound,
    whether the bike is listed, and its distance.
  - `WreckBikeScene` is the `ragdoll_contact::Scene` that answers a bone with the bike's own body.
- `ragdoll_ride::Down::step_posed`:
  - It gathers once a tick, before the wreck arm, round the first collision's sphere where it last
    stood, with the bike offered once `ignores_bike` is down.
  - It hands the bones the `WreckBikeScene` instead of `Open` when the kit has the mesh.
  - `Kit::bike_mesh` is set by the shell (`crates/openmx/src/main.rs`) and by `tests/ragdoll.rs`'s
    kit from the same `wreckbikef.col`.
- `ragdoll_body::wreck_arm` drops `ignores_bike` when the bones come off the bike.
- `Down` carries `search`, which is in the snapshot wire and in the hash walk with `ignores_bike`.
  `tests/hash.rs`'s Sim size pin moves to 11304.
- Tests:
  - `tests/ragdoll_wrecks.rs` (S105 section):
    - `the_wreck_bike_mesh_is_the_ones_in_ram`
    - `the_sphere_test_answers_as_the_originals` (20,000 generated spheres)
    - `the_bike_is_in_the_tree_where_its_sphere_stands` (20 states)
    - `every_sphere_test_of_the_wrecks_is_the_bike_meshs` (the five s62 wrecks)
  - `tests/ragdoll.rs`: `the_rides_bones_gather_the_wreck_bike_once_off_it`.
- `research/oracle.txt`: the pair and mesh-walk names, plus comment lines on the search, its
  filter, the bike's placement and the un-ignore.

## What the original does

- **The narrow phase.** The sphere collider pairs through vtable +0x94 into `./kit fn 0x1f1790`. For
  a mesh other this is `./kit fn 0x109ed0` and then the tree walk `./kit fn 0x109490`. The walk:
  - takes the first child, and the second only if the first said no;
  - at a leaf, takes the plane distance in 0..r;
  - needs the foot inside the triangle, or an edge within r;
  - keeps the nearest in the record, and hands it back through the frame.
- **One search for all the bones.** It is a pointer at the body actor's +0x628. It is gathered by
  `./kit fn 0x218b80` at 0x24d128 in `rider_body_mode`, and only on the wreck mode's path (the
  `bne` at 0x24cf90). So a body put back on the bike (mode 0, then 1) stops gathering. The replay
  showed this at front 2682.
  - The gather keeps the list when built and `|p − centre| + r0 < bound`; it then only re-measures
    the distances.
  - Otherwise it queries the static tree round the sphere with a box of half `least` (10). The new
    bound is `min(x − lo.x, z − lo.z, hi.x − x, hi.z − z, max(extra 30, r0))`, then `max(least)`.
    Here lo/hi is the box the queried cells cover.
- **The filter** is `./kit fn 0x119d70`, (sphere, candidate, 0). It checks both ignore lists, the
  masks, and bit 0x4 of +0x116. Beyond the filter, a candidate is listed if bit 0x8 of its +0x116 is
  clear or its distance is under the bound. The bike's +0x116 is 0x14. So the bike is listed whenever
  the query reaches its node, whatever its distance.
  - A sphere is paired only with listed candidates whose distance is within its radius
    (`./kit fn 0x11a9d0`).
- **The ignore.** The wreck entry puts the bike on every bone sphere's ignore list (mode 1). The frame
  the bones are off the bike (+0x6b0), `./kit fn 0x11aed0` takes it off: its slot is written 0. The
  gather runs before that test, so the bike is offered from the next gather.
- **The bike's placement.** B+4 is a record: +8 is the track's tree, +0xc its key, +0x10 its centre
  (equal to B+0x40). The key is `broad_build::key` of the mesh root box through B+0x120, not of the
  sphere's centre ± radius. The radius gives the wrong corner bits at tip 2473.
- **The 132 hits** S62 quoted were counted over all nine captures (s56 and s62) and the whole test
  run (its journal, part 2). Over the s62 replay alone the original hits the bike 64 times.

## How it was proved

All under `./kit check` (`-p game-sim --test ragdoll_wrecks`, `--test ragdoll`), on save-state
captures from context/capture/s62 and on the oracle.

- The mesh is equal word for word to the collidable's arrays in RAM.
- 20,000 generated spheres in front 2603's state go through the original's pair and the port: same
  hit or miss, plane distance, world point and normal, bit for bit (4000–16000 hits).
- 20 states: in the 16 where the bike is in the broad phase, its placement is in the track's tree at
  B+0x40 under the key `Cells::key` gives. The tree's nodes read back to that key, and the port's
  tree has RAM's origin, extent, cell and scale. In the other 4 it is in none.
- The five s62 wrecks, replayed as S62's test does. The port's search is gathered where the
  original's gather runs (hooked), and after every update it equals the original's: centre, bound,
  built, listed and distance, bit for bit. At every one of 24,822 sphere tests, the port pairs the
  sphere with the bike exactly when the original does, and answers it as the original did (point and
  normal bits, or a miss). The totals are 600 pairs, 64 hits, 367 gathers, 12 rebuilds and 176
  gathers with the bike listed.
- The ride: a reason-5 wreck on the test slope gathers from its second tick. It never lists the bike
  while the bones ignore it. It lists the bike at tick 43, the first rebuild after the bones came off
  at 35.

## Not verified / not 1:1

- **The replay's bike key is the Ee's, not the game's.** The replay never runs the bike's own update.
  Laying in the capture moves the bike's placement record but not the tree's nodes, so the Ee's tree
  holds the bike where it no longer is. The original's query reads those nodes. So the replay test
  hands the model the key read back from the nodes that hold the bike (`bike_key_in_tree`), and does
  not use `Cells::key` of the bike's frame that tick. The ride uses `Cells::key` every tick. That is
  what the game's bike update should do, but it is proved only on the 16 save states, not tick by
  tick.
- **The ride's bike frame** is `quat_to_mat4(crash.bike.body.quat, crash.origin)`. No test holds it
  equal to B+0x120 in the ride.
- **The ride's tree box** comes from the terrain field's box: samples × spacing from 0. For free09
  that is 768 × 9 = 6912 both ways, which is RAM's tree. That is argued, not asserted against a loaded
  track in a test. In a world without a field edge (the tests' worlds), the bike is always found and
  the bound is its least.
- **The search at the wreck's entry.** The original's search carries over from riding. The ride's
  starts unbuilt and is first gathered on the second tick, when the first sphere has stood. The
  replay starts from RAM's search, so this is not compared.
- **The bike leaving the broad phase.** About 400 frames after the wreck (back 2877, front 3002, side
  3007, tip 2872), B+4 is 0 and B+0x30 0xff. This is not modelled: the ride keeps offering the bike.
- **Only the bike is in the port's search.** The original's list also holds about 22 far objects;
  their order and the capacity of 128 are not modelled. A list that fills before the bike would drop
  it, which the s62 wrecks never did.
- **The rest of the filter** (`0x119d70`'s masks and its other lists) is reduced to the ignore flag,
  and so is B+0x31 bit 0 (the pair's own gate, 3 on every state seen).
- **The riding entry re-ignore.** A body put back on the bike stops gathering, which matches mode 2
  only. Whether the next wreck's entry restores the ignore, and the search state between wrecks, are
  read from the code, not compared.
- **The hierarchy class (1) and the swept test** against the bike are not ported. The wreck bike is a
  mesh, and S62's wrecks never swept against it.
- **The scene's body copy:** `WreckBikeScene` copies the bike's body and collider for the
  push-out's velocity query, as S62's `Moving` does. This is not compared inside the ride.

## Next

Nothing for this task. S106 is filed for the bike leaving the broad phase late in a wreck.
