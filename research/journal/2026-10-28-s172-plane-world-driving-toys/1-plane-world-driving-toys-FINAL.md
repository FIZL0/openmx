# 1 — plane world driving toys (FINAL)

Task S172. 2026-10-28.

## What I did

- **Found what the two "driving toys" are.** S141 found two driving toys in slots 21..22 of
  biplane.p2m2's moving list. They are free06's two `HasPhysics` trucks: `ctruck.psg`, the
  `.toy`'s `Car22` and `Car23`, and their mover's vtable is the physics-car subclass's. So this task's
  two halves are one thing.
- **The port already had them.** S127 (which landed after S141 filed this task as S166) made the
  `HasPhysics` cars ordinary `Traffic` cars, in the `.toy`'s order. Once a car has a `Moving`, the
  plane's world (`world_flown`) files it as a `TOY` like any other. So the port's free06 plane world
  already holds all 23, and no code change in the sim was needed. This task proves it slot by slot,
  and fixes the order claim below.
- **New test:** `crates/game-sim/tests/physics_cars.rs`,
  `the_planes_world_holds_every_toy_as_the_originals_does`, on biplane.p2m2's state alone.
- **`flight_search.rs`:** the docs on `TOY` and `world_of` now say "the `.toy`'s cars, path and
  `HasPhysics` alike, in `.toy` order", not "the path cars". `S186` is named for the struck mode.
- **S127's test had the order wrong.** `the_planes_world_lists_the_trucks_after_the_path_cars`
  claimed the original lists path cars first, then the physics cars. The init read below says it
  lists them in `.toy` order. The two agree on free06, where the trucks are last, but not on free11
  (`PPP....P....`). That one line of the test now expects `.toy` order.
- **`research/oracle.txt`:** a new S172 block (comment lines only, no new names).
- Nothing was taken from opentennis: it has no counterpart for this.

## What the original does

- **The `.toy` car reader** (`./kit fn 0x00289710`) news each car and inits it as it reads it:
  - `Car1..NumberOfCars`, in order.
  - A path car's init is vt+0xc4. A `HasPhysics` car's is the subclass's vt+0xf8, which runs the
    path car's init first.
- **The path car's init adds the mesh to the world** (`./kit fn 0x001147c0`, the call to
  `0x00119c60` at `0x00114f40`).
  - It adds the mesh collidable (+0x428) to the world inside the init itself.
  - So the moving list's order is the reader's order, whatever the class.
  - The optional second collidable (+0x42c) is also added in the init. None of free06's cars has
    one.
- **The subclass's physics object adds nothing to the moving list.** In the state, the plane's
  own collidable sits right after `Car23`.
- **Which tracks have `HasPhysics` cars** (from a throwaway scan of every track's `.toy` on the
  disc):
  - free06: `.....................PP`
  - free07: `PPPP`
  - free10: `PPPP`
  - free11: `PPP....P....`
  - free09 and every nat and sx track have none.

## How it was proved

A save-state capture plus the interpreter: biplane.p2m2's RAM image, 23 cars, one tick.

- **The state's moving list** opens with 23 car slots, a path car or a physics car each. That is
  free06.toy's `NumberOfCars`.
  - Slot k has the `.toy`'s `Car(k+1)` model, and it is a physics car exactly when that car has
    `HasPhysics`.
  - Each slot is its car's own `+0x428` mesh, of category 0x48, with no owner.
  - Exactly two slots are trucks.
  - Slot 23 has an owner: it is the plane's.
- **The port's world, built from the state's 23 cars.**
  - Each car's mesh comes off the disc as `<model>.col`. Each car is attached in slot order and
    stepped one tick by `Traffic::frame`.
  - Beside that, each original car runs its own update through its vtable: the path car's, or the
    subclass's.
  - `world_flown`'s moving list is then exactly the 23 toys followed by `[RAY, SWEEP]`.
  - Each toy has the state's category, flags (with the search marks masked), and radius, bit for
    bit.
  - Its placement centre (`prop_box`) equals the original's `+0x40` after the update, bit for bit
    (xyz). S141 had left this unchecked.
- **The test reaches its end.** A temporary `panic!` at the end fired with "23", so it does not
  pass by skipping. `./kit check -- -p game-sim --test physics_cars`: 4 passed. `--test traffic`:
  12 passed.

## Not verified / not 1:1

- **Interleaved order is proved by reading code, not by a state.** free06's trucks are the last two
  cars, so its state cannot tell `.toy` order apart from "path cars first". The `.toy`-order claim
  for free11 (`PPP....P....`) rests on the decompiled init adding the mesh inline. There is no
  free11 RAM image.
- **The port's own load order is not checked end to end here.** The test builds the port's cars
  from the state's, in the state's order. It does not check that the shell's
  `game_render::toy::Traffic::load` + `sim` keeps the `.toy`'s order. It reads `Car1..N` in
  sequence, and game-render's `the_simulations_cars_are_the_originals_paths` matches cars by
  content, not by position. game-sim tests cannot call game-render.
- **The struck trucks.** A struck truck, which the original turns into a rigid body with its meshes
  stood down, is still S186. Until then, the port's truck stays a path-driven toy in the plane's
  world after a hit.
- **Other tracks.** free07, free10 and free11's physics cars in the plane's world are not checked:
  there is no state of a plane on those tracks.
- **Only one tick.** The centre is compared after one tick. S127's drive test covers the trucks'
  poses over 300 ticks, and S141's covers the path cars.

## Next

Nothing for S172. S186 (the struck mode) is where the trucks' remaining difference lives.
