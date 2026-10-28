# 1 — wreck bike on moving platforms (FINAL)

Task S171. 2026-10-28.

## What I did

- **The wreck bike's world is now the plane's.** `WreckBike::tick_among`, the ride's wreck bike
  (`ride::Crash::step`), searches `flight_search::Scene::flown` instead of `Scene::wooded`.
  - A prop with a `Moving` (a path car, or since S145 a toy's collision part) is a toy there:
    category 0x48, ENABLED|NO_GROUPS, in the moving list ahead of the two probes, not in the tree.
  - The contacts already carried the car as their mover. S145 put that in `Scene::struck_of`, which
    both worlds share, and `WreckBike::tick` already resolved with `Search::movers`. So this task
    changed only where the cars are filed: their category, their flags, and the structure and order
    they are gathered from.
- **The put-back's query** (`back_search::gather`) builds its broad phase with
  `flight_search::world_flown` instead of `world_wooded`.
  - A gather of both structures lists every car first, because the moving list hands its entries out
    wherever they are.
  - A race's static-only gather lists none.
  - The query's own distance and sphere tests still decide whether a car is swept.
- `flight_search` docs say which worlds still file the cars as scenery: the riding bike's wheel and
  body searches (`ride.rs`'s `Scene::wooded` calls) and `prop_world`. No code changed there.
- No ride code outside these two calls changed. `world_wooded` / `Scene::wooded` are unchanged, bit
  for bit.
- **A new RAM image**, `context/states/s171-thrown.eeMemory.bin`, exists only in s3's context.
  - Made in PCSX2 copy 3 from S89's `context/s89/along.p2s`.
  - The bike was teleported 40 ft ahead of the train's car 0 with S89's `tele.py`, through a wrapper
    script that reads the car's position over PINE first. The train threw the bike at about 300 ft/s.
  - The state was saved about 5 s later (slot 9) and unpacked with `./kit state ram 9`. An earlier
    image, `s171-hit`, was saved right after the throw (slot 7).
  - A 60 ft drop on the rails, tried first, did not wreck the bike.

## What the original does

- Both of the in-world wreck bike's probes are placed in the world's one table.
- Its moving list is the one biplane.p2m2's plane is in, at the same address in both sessions. At
  its head are the path cars in free06.toy's order, then the two driving toys. All are 0x48 with
  their car as a kind-2 mover, the same models in the same order with the same flags as in the
  plane's state.
- Neither probe's leave-out list or category list excludes 0x48. So the original's wreck bike meets
  the cars as toys with their motion, as the plane's probes do.
- Addresses and lists are in `research/oracle.txt`'s S171 block.

## How it was proved

All three tests are in `crates/game-sim/tests/traffic.rs`.

- **`the_wreck_bikes_world_holds_the_cars_as_the_planes_does`** (a save-state RAM image) runs on
  s171-thrown and biplane.p2m2. It checks:
  - the wreck bike's collidable category;
  - that both probes are in one table, whose moving list is where the plane's is;
  - that the 23 toys at its head equal the plane state's (model and flags, in order), the first 21
    being the .toy's cars;
  - the two probes' leave-out and category lists.

  S141's `the_planes_world_holds_the_cars_as_the_originals_does` already proves that `world_flown`
  lists the port's cars with those categories, flags and radii.
- **`a_wrecked_rider_thrown_onto_the_moving_train_slides_with_it`** puts the real
  `bike\wreckbike.phy` 2 ft over the sixth car's roof. It starts at the wagon's velocity less
  12 ft/s along it, falling at 10 ft/s, and runs through `WreckBike::tick_among` for 150 ticks,
  cars first.
  - Carried: it stays on the deck all 150 ticks and slides back at most 6.87 ft. It ends
    0.013 ft/s off the wagon's velocity, and 355 contacts name the wagon.
  - Control (the cars' motion taken away, so they are scenery in the tree): it leaves the deck at
    tick 29, 221 ft off by the end.
- **`the_put_back_gathers_the_cars_from_the_moving_list`**: a bundle over the wagon's roof.
  - Both structures: the gathered list starts with the 21 cars, and probe 0 meets the roof
    (y within 0.5 ft of 151.6).
  - Static only: no car is listed, and nothing is met.
- I made these tests fail on purpose once to read the numbers above, so none of them skips in s3.
  I then took the forced failures out.
- These targeted runs passed: `traffic` (15), `wreck_bike` (6), `back_search` (2), `off_the_map`
  (6), `ragdoll_cars` (4), `backwards_bail` (3) and openmx's `train` (3).

## Not verified / not 1:1

- **No capture of the original's wreck bike on the train.** The world's layout is proved against the
  state, but the thrown bike's ride is the port against itself. It reuses the mover arithmetic S142
  proved (tests/contact.rs). Whether the original's wreck bike rides a roof the same way was not
  recorded.
- **Whose wreck bike it is.** The in-world record 0x01970060 is one of four wreck-bike objects in
  the state. I did not find which rider it belongs to: the human's request word (+0x488) read 0 on
  the rider I took, so it may be an AI rider's. The world it searches is the same table either way.
- **The gathered list's order and content against the original's.** The wreck bike's nearby list in
  s171-thrown (+0x10c) was not compared. The port gathers fresh each frame (as before, see
  `tick_among`'s doc), and its toys now come first, as the moving list hands them out.
- **The put-back's gather against the original's.** Its world is the plane's by S83's reading, and
  the moving list holds the cars in the state. But no put-back next to the train was captured, so
  the list's order, which now puts the cars ahead of the scenery, is unproved. The query's answer
  does not depend on the order, except which of two equal heights keeps its normal.
- **The two driving toys and the HasPhysics trucks** are still not in the port's worlds (S166).
- **The riding bike's wheel and body searches** (`ride.rs`, `Scene::wooded`) and the props' own world
  (`prop_world.rs`) still file the cars as scenery in the tree. The original's riding bike is in the
  same table (S141's moving list holds the bikes' probes), so it most likely meets them as toys too.
  This task did not ask for that and I did not check it. Filed as S208.
- **The state is only in s3's context.** The new world test skips in every other slot.

## Next

- S208, the riding bike's searches.
