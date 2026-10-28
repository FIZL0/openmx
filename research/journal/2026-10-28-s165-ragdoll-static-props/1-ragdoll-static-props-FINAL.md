# 1 — ragdoll static props

Task S165. 2026-10-28.

## What I did

- **The bones' search now lists the track's static props**, as the original's search lists them from
  the static quadtree. The addresses are in `research/oracle.txt`'s S165 block.
- `ragdoll_scene.rs`:
  - `Cells::query_among` puts the static props in the cleared tree under their keys, with the bike
    after them. It walks the original's query (`Query`: the node list, chunks of 4, stamps) and
    answers the props met in walk order and where the bike stood among them. The old `query` was
    folded into it, since nothing else called it.
  - `Statics { props, bike_at }` is the listed props, in list order, and the bike's place in the
    list. `Statics::keys` gives every non-moving prop's key (`broad_build::key` of its `prop_box`).
  - `BikeSearch::gather_among`: at a regather, a prop goes on the list when the query's cells meet
    it and its reach from the probe is below the bound. Scene meshes ask for the bound; the bike
    does not. The list stops at the capacity of 128. A kept list keeps the props. `gather` is
    `gather_among` with no props, so its behaviour is unchanged.
  - `StillProp { mesh, frame, distance }` is a scene mesh in its `to_world` frame. `near` copies only
    the listed props that the largest bone sphere can reach. `touch` is the shared `touch_mesh` with
    a held distance, as `PathCar::touch` does.
  - `WreckBikeScene.props` and `bike_at`: `touching` offers one record the static candidates in list
    order (the bike at its place, then the props), then the cars as before. Strictly nearer wins.
    A prop that takes the record sets `with` to None, so a contact on it is with the still scenery.
- `ragdoll_ride.rs`: `Down.statics` is gathered with the bike's search from `world.collision()`'s
  props. The props within reach are filled into the scene each frame, measured from where sphere 0
  last stood.
- **Rollback.** `Down.statics` is on the snapshot wire (`snapshot.rs`) and hashed as `search_prop`
  leaves only while the list holds any, so the walk's leaf count does not move.
  - `tests/hash.rs`'s size pin: 11776 → 11808.
- `tests/ragdoll_cars.rs`'s "Gone" mode now rides a world with no props. Its old way, a car with
  `moving` cleared, is a static prop since this task, and the bones met it.
- `crates/game-sim/tests/ragdoll_props.rs` (new): four tests, listed below.

## What the original does

- **The search.** The bones' search (body actor +0x628) lists placed groups too (`no_groups` 0), and
  it queries the static tree before the moving list.
  - free06's scene meshes in the tree are class 2, category 0x220. Their flags +0x116 ask for the
    bound (WITHIN_BOUND), so a mesh is listed only when its reach is under the bound.
  - The pair filter passes the bone sphere to every one of them.
- **The rest.** A contact's push-out with the other a scene mesh measures the bone against a still
  (0, 0, 0, 1) velocity.
  - The record and pair rules are S142's: one contact, the strictly nearest, over the list in order.

## How it was proved

`./kit check -- -p game-sim --test ragdoll_props`: 4 passed. All four run on `context/states/s89-along`
(free06) with the disc's scene meshes and the shell's `Props::load`. Every one of the state's scene
meshes maps to a port prop by its box centre and reach bits.

- **`the_bones_search_lists_the_props_as_the_originals`**: function oracle, 3,000 generated probes.
  - Setup: each probe stands within a random prop's reach plus 30 ft. Each goes through the
    original's `collision_search_gather` on the real bones' search with built cleared, then is moved
    inside the bound so the list is kept. The port's `gather_among` runs on the same probes.
  - Compared: the scene meshes listed, in order, with their distances, plus the centre and the
    bound, bit for bit. The tree the port lays over free06's field is also compared with the state's
    (origin, far corner, extent, cell, scale).
  - Coverage: 6,000 lists compared, 1,296 of them empty, up to 11 props in one, none at capacity.
  - The filter answered 1 for every scene mesh in the tree.
- **`the_sphere_test_on_a_prop_answers_as_the_originals`**: function oracle, 20,000 generated spheres.
  - Setup: each sphere is stood near a random vertex of a prop, with radii 0.4, 0.5, 0.75 and random
    values from 0.1 to 1.
  - Each goes through `sphere_collider_clear` + `sphere_collider_pair` against one of three
    candidate lists: one mesh, a mesh then its nearest neighbour, or the same mesh twice.
  - Compared against `StillProp::touch` with the held distance: the count, which collidable holds
    the record, the held distance, and the world point and normal, bit for bit.
  - Coverage: 12,465 hits. The second prop took the record 64 times, and the first kept it 8,290 times.
- **`the_push_out_against_a_prop_is_the_originals`**: function oracle, 10,000 generated bones.
  - Setup: the bones are on the state's bone collisions, with the contact's other a real scene mesh.
  - Compared against `push_out_against(.., moving(point, Against::Still), ..)`: the position,
    velocity, quat, rates and dv, bit for bit, swept cases included.
- **`a_ragdoll_dropped_on_a_roof_lands_on_it`**: the whole ragdoll.
  - Setup: dropped from 4.5 ft over the middle of the widest roof among free06's props (prop 38, y
    164.7), for 60 ticks.
  - With the props: by tick 30 it has landed with the main bone 0.75 ft (its radius) over the roof, and
    stays there.
  - Without them (the port before this task): it falls through to 19.5 ft under the roof.
- `ragdoll_cars`, `hash` and the rest of the gate pass with the changes above.

## Not verified / not 1:1

- **No capture of the original's ragdoll against a building.** Each step is proved function by
  function, and the roof test shows the port's whole ragdoll. No recording of the original's
  ragdoll meeting a fence or building exists, so a bounce's size is the port's result.
- **Placed groups (plants) and the tree's other static collidables are not in the bones' scene.**
  - Plants: the search's `no_groups` is 0, so the original lists the class-1 group entries.
  - Also not ported: the category 0x18 meshes (12), category 0x100 (42), and one 0x9a058 mesh.
  - Calling the pair filter on a group hangs the Ee, so none of these were checked. Filed as S204.
- **The order of the bike and the props within one tree node.** The port inserts the props before
  the bike. The gather oracle runs with the bike not offered (the state's bike is not the port's),
  so the bike's place among the props in a shared node was not compared. It only matters for an
  exact tie of plane distances.
- **Capacity.** No probe filled the list's 128. The port stops listing at 128, counting the bike but
  not the cars. The original's count also holds its moving entries (cars, groups, bikes), so near a
  very crowded spot the port could list a prop the original drops. This was not reached on free06.
- **No sweep.** As with the bike and the cars (S142), `WreckBikeScene` has no swept test against a
  prop, so a fast bone can tunnel through a thin fence between frames.
- **One state, one track.** The filter, the flags and the gather were checked on s89-along (free06)
  only. Other tracks' scene meshes are assumed to carry the same category and flags. In tiled worlds
  the props come from `collision()`'s whole set and were not exercised.
- **Without a bike mesh there are no props.** That is the existing condition for the bones' scene,
  the same as the cars.

## Next

- Follow-up filed: S204 (the plants and the other static collidables in the bones' scene).
