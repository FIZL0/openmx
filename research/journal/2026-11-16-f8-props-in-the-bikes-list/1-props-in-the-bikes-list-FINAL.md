# 1 — props in the bikes list (FINAL)

Task F8. 2026-11-16.

## What I did

- `flight_search::Knocks`: the knockable props as collidables of the riding bike's world, after the
  plants' trunks, one per body with a mesh and a collision object, in set order (`crate::flight::knocks`).
  - Asleep: filed in the static tree after the plants, keyed by the mesh's root box where its node stands
    it (F7's key), flags `0x04` (listed wherever the cells reach), measured from the posed centre less
    its reach. Their sleep, reach, centre and box go into the kept world's stamp.
  - Awake: in the moving list after the two probes, placed each search (`Kept::place`).
- `flight_search::refresh_knocked` / `gather_knocked`: the bike's gather with them. `Ride::gather_nearby`
  gathers through it with the lent props (`prop_wheel::riding`).
- `Scene::ridden`: the wheel probe (`meet`) meets the knocks where the list names them, interleaved with
  the scene, gated on the stored distance. Struck knocks are recorded in strike order
  (`Scene::knocks_struck`) and woken after the search (`PropSet::struck`). A contact on one names the
  prop's handle and its mesh's material, as S210's did.
- The body rays (`rider_contact::scenery_heard_among`) and the sweep back search the same world with
  the knocks taken off the list (`meet` false), so the list is never regathered by them and they still
  meet the props only through `prop_contact`, as before.
- `prop_wheel::search` is kept only for worlds that answer `World::solid` alone (no list there).

## What the original does

- The bike's search `*(rider+0x7bc)` is gathered once a tick after the substeps round the body rays
  `*(rider+0x7c4)`: `./kit fn collision_search_gather` with `(search, probe, 3)`.
- A sleeping prop's `f.col` is in the static tree (F7), so the gather lists it among the scene meshes
  and plants. The wheel probe `+0x7c8` searches that one list.

## How it was proved

- Function oracle: `tests/prop_wheel_gather.rs`
  `the_bikes_list_holds_the_props_at_rest_as_the_originals`. The original's gather on recording2's
  rider bike search against the port's `gather_knocked`, round 2000 probes near free09's 62 sleeping
  props and trees, then moved within the bound (kept). 4000 lists bit-exact: entries in order,
  distances, centre and bound. 8775 sleeping props listed; 935 lists mix them with scenery or plants.
- Port ride: `tests/prop_strike_paths.rs` `a_wheel_meets_the_ball_through_the_bikes_list`. In a world
  that hands the bike a scene, the bike gathers the ball into its list, the wheel's owner word is the
  ball's, and the hit wakes it. The S210 tests (`a_wheel_stands_on_the_ball_in_every_path`, Flat world)
  still pass through the solid-only path.

## Not verified / not 1:1

- The wheel search's hits against the original's record with a prop and scenery on one segment: the
  S195 knock states (`prop_knock.rs`) are not in the shared context, so its wheel test skips. Only the
  list is proved; the narrow phase is S241's `pair_placed`, proved for the plane.
- Awake props' place in the moving list: put after the probes in set order. The original appends a
  woken prop's mesh at wake time (S240); its order among the moving entries was not read.
- The original searches the wheel once a tick before the substeps; the port still searches every
  substep (unchanged here).
- The body rays and the sweep back leave the listed props out and meet them through `prop_contact`,
  as before; whether the original's body rays meet them through the list was not compared.
- A ride with no props lent (no props on the track) gathers a world without knocks; a list gathered with
  knocks and then searched without them regathers (`refill`). Not hit in play: the props are lent for
  the whole frame.

## Next

- Remake the S195 knock states (memory: research/s195 aim.py + knock.py) and compare the wheel probe's
  record through the list against the original's `collide_against_search(+0x7c8, *(+0x7bc))`.
