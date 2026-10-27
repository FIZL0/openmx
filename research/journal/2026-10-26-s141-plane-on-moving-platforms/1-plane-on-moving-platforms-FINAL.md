# 1 — plane on moving platforms (FINAL)

Task S141. 2026-10-26.

## What I did

- Found how the original's plane meets the path cars (below; addresses in `research/oracle.txt`'s
  S141 block). Its wheels never see them. It meets them through its collision object's rays, with
  the car as the contact's mover.
- `flight_search`:
  - `world_flown` / `refresh_flown` / `Scene::flown` are the plane's world. A prop with a `Moving`
    (a path car, S136) is a toy there, not tree scenery: category `TOY` (0x48), flags
    ENABLED|NO_GROUPS, in the moving list ahead of the two probes, at its prop's box centre, with its
    prop's radius. Every other world (`world_among`, `world_wooded`, `Scene::wooded`: the wreck bike
    and back_search) is unchanged, bit for bit.
  - `Scene::flown` makes one `contact::Mover` per car from its `Moving`: its spin, its velocity, its
    pose as the node and origin `(0,0,0,1)`. A ray or sweep hit on a car's mesh gets
    `Struck::mover = (that mover, kind 2)`. `Scene::velocity` (the sweep-back's) gives the car's
    velocity, where it gave zero before.
- `Flight`:
  - Gains `movers`, rewritten by every search from this tick's car motions.
  - `forces` hands `movers` to `contact::resolve`, so S42's mover branch (proved in tests/contact.rs)
    works the spring and friction against the car's point velocity under the contact.
  - The movers are not on the wire (`snapshot.rs` skips them, `take` leaves them empty): a restored
    plane's next frame searches before it resolves, and the search makes them again from the cars,
    which are state.
- `tests/hash.rs`'s `Sim` size pin moves 11536 → 11560 for the movers Vec, which the walk leaves out.
- Filed S165 (the wreck bike's world) and S166 (the two driving toys and the HasPhysics trucks).

## What the original does

- **The wheels.** The plane's tire probe calls the world's ground query (`./kit fn 0x001dc960`): the
  cell cache and the cell reader, the terrain alone. A plane on a wagon's roof is held up by its
  collision object's ray probe (biplanem.col's 36 segments), as it is on any mesh.
- **The plane's moving list in biplane.p2m2's state** (free06, the plane flown, the train and the
  earth movers driving):
  - Slots 0..20 are the 21 path cars' mesh collidables, in free06.toy's Car1..Car21 order.
    - Category 0x48, flags 0x05 (ENABLED|NO_GROUPS) plus the search's mark 0x10.
    - +0x7c is the car: the path car vtable, vehicle kind 2.
    - +0x80 (the owner) is 0.
  - Slots 21..22 are two other driving toys (another vtable).
  - Then come the plane's own body and probes, the bikes and the rest.
  - Scenery meshes are category 0x220, flags 0x0c, in the static tree.
- **The contact.** A collidable whose mover is a vehicle goes through contact::resolve's mover
  branch: the car's velocity, plus its spin crossed with the contact point unplaced into the car's
  node, less the origin (S42).
- **The crash test.** It spares a strike only on the VEHICLE bit 0x2000, which 0x48 does not have.
  So a plane flown into a car wrecks as on scenery. MinWreckSpeed (plane+0x3b8) is 73.333 ft/s,
  over the train's ~55, so a plane resting on a roof rides on.

## How it was proved

All in `crates/game-sim/tests/traffic.rs`:

- **`the_planes_world_holds_the_cars_as_the_originals_does`**, on biplane.p2m2's state:
  - The first 21 slots of the moving list are the path cars, by model name in free06.toy's order,
    each kind 2, with no owner and category 0x48.
  - The port's `world_flown` over s89-along's cars lists them ahead of `[RAY, SWEEP]`, each with the
    state's category, flags (marks masked) and radius bit for bit (matched by model: s89's car list
    is in address order, not the .toy's).
  - `world_wooded` still files them as scenery.
- **`a_plane_set_down_on_the_train_rides_it`**: the biplane off the disc, set down a foot over the
  sixth car's roof, facing its way at its speed, throttle shut, for 150 ticks, cars first and then
  the plane (as `Sim::step` orders them).
  - **Drag off:** the plane moves under 0.08 ft along the deck and 0.009 ft up after settling, never
    wrecks, and every tick has a contact on the car. It ends at 55.128 ft/s on a 55.129 ft/s wagon.
  - **Control (the cars' motion zeroed, still toys):** the plane is off the deck at tick 50.
  - **Drag on:** it slips 2.7 ft back over the five seconds and ends at 54.42 ft/s. It still never
    wrecks and stays on. The slip is all the plane's own drag (zeroing drag0/drag1/drag_de takes it
    away), which reads the airspeed as the original's does. This is the same story as S136's bike.
  - **Rollback:** the drag-on ride is kept at tick 75 through `snapshot::encode_flights` /
    `decode_flights` (movers empty, probe put back as `decode` does), with the cars' state cloned and
    their meshes stood again. Ticks 75..149 flown again match bit for bit (position and velocity).
- **`a_plane_flown_into_the_trains_side_meets_it`**: the plane flown level at 120 ft/s from 60 ft
  abeam of the wagon, 6 ft under its roof.
  - It wrecks at tick 14 with `CRASHED`, the record's struck mesh is a car, and its contacts carry
    the car as mover.
  - Over free06 with no cars the same flight does not wreck in 45 ticks.

`./kit check -- -p game-sim --test traffic`: 8 passed. flight_collision, flight_candidates and
flight_bikes also pass unchanged.

## Not verified / not 1:1

- **No capture of the original's plane on the train.** No recording of the original landing on a
  wagon or flying into one exists. The world's layout is proved against the state and the contact
  branch against the oracle (S42), but the plane's ride as a whole is not. In particular, the 2.7 ft
  drag slip is a port result that no capture confirms or denies.
- **The side strike would have struck before this task too.** The cars were already scenery meshes
  in the tree. What is new is the mover on the contact. Whether the original's wreck tick or the
  struck car for this flight matches is not checked.
- **Only path cars are toys.** The two driving toys of slots 21..22 and the HasPhysics trucks are
  absent from the port's plane world (S166). Other worlds (the wreck bike's, back_search's) still
  file the cars as scenery with no mover (S165).
- **`category()` of a car.** flight_search's category lookup for a struck prop still answers
  SCENERY, not 0x48. Both lack the VEHICLE bit, so the crash test is the same. Any other reader of
  the category was not checked.
- **Which pose the search sees.** The port's plane searches against this tick's car poses (the
  traffic steps first in `Sim::step`). The original's order of path_car_update and the plane's step
  in one frame was not checked. If the plane goes first there, it sees last frame's.
- **Order of the toys.** The port lists the cars in its props' order, which a ride's load takes from
  the .toy (the original's order). The test's s89 cars are in address order, so the port-side
  order against the state was proved by model only, not slot by slot.
- **The toys' placement.** Each sits at its prop's box centre with the prop's radius. The radius
  matches bit for bit; the centre against the state's +0x40 was not compared.
- **The first frame** after the cars attach, before the first `Traffic::frame`, has no `Moving`, so
  the cars are scenery in it (as S136's).
- **The tree's bounds.** A fieldless plane world's tree box still spans the cars' boxes, though they
  are no longer in it. This was not compared against the original.

## Next

- S165, S166.
