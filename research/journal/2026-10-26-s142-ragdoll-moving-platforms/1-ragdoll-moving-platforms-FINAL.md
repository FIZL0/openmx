# 1 — ragdoll moving platforms (FINAL)

Task S142. 2026-10-26.

## What I did

- **Answered the brief's question: yes, the original's bone search gathers the path cars.** It reads
  their mesh collidable's frame and the car's own two vehicle virtuals. Details are below, and the
  addresses are in `research/oracle.txt`'s S142 block.
- `traffic::Moving::mover()`: the car as a bone's push-out sees it. It is a `contact::Mover` with the
  car's spin, its velocity, its node's world matrix and `ORIGIN`.
- `ragdoll_scene.rs`:
  - The sphere-on-mesh walk (`touch_mesh` / `descend`) now runs on any `track::Mesh`. It takes the
    distance held by the record's earlier hit, and a mesh's leaf takes the record only when its hit
    is strictly nearer.
  - `Bike::touch` is unchanged bit for bit: it passes no held distance and the same packed normals.
  - A car's normals are packed per leaf with the same `pack_normal`.
  - `PathCar { mesh, frame, mover, distance }`. `PathCar::near(props, probe, reach)` copies only the
    cars a sphere could pair with: those whose sphere's reach from the probe is within the largest
    bone radius.
- `WreckBikeScene` gains `cars` and `with`:
  - `touching` offers one record the bike first (when the search pairs it, as before) and then each
    listed car whose distance is within the sphere's radius. The strictly nearest hit stands.
  - `other`/`velocity` answer the bike's body for 0, and `moving(Against::Mover(car))` for car k+1.
- `ragdoll_ride::Down::step_posed_with` fills `cars` from `world.moving()`, measured from where the
  first sphere last stood, once the search is built. It does this only when there is a bike mesh,
  the same condition as the rest of the bones' scene.
- **Rollback.** No new state, nothing hashed. The cars' `Prop::moving` is already rewritten from the
  snapshotted car state every tick before the rider (S136), and the list is rebuilt every frame.
- `crates/game-sim/tests/ragdoll_cars.rs` (new): four tests, listed below.

## What the original does

- **The search.** The bones' search (`./kit fn 0x001f6c40`'s caller chain; body actor +0x628,
  gather kind 3) queries the sphere's space's table. It asks the static quadtree first, then the
  moving structure.
  - The moving structure is a block list. It hands out every moving collidable whatever its
    position, and reports no bounds, so the search's bound does not change.
  - Every car's mesh collidable (car+0x428) is in that list. Its fields:
    - mesh class, with gate bit +0x31 bit 0 set;
    - +0x116 lacks the within-bound bit, so it is listed wherever it is;
    - owner +0x7c is the car, and collision object +0x80 is 0.
  - `collision_pair_filter` lets every bone sphere through to a car.
  - A sphere is paired with a car when the car's distance in the list is within the sphere's radius.
    That distance is the car sphere's reach from where sphere 0 last stood, re-measured every gather.
- **The record** (sphere +0x6c, capacity 3). The tree walk's leaf (`./kit fn 0x00109490`) writes
  slot 0 when the record is empty or the hit is strictly nearer than the distance it holds, and sets
  the count to 1. Otherwise it answers no. So over all the candidates, one contact stands: the
  strictly nearest.
- **The pair** (`./kit fn 0x001f1790`, mesh branch). It walks the mesh in the collidable's frame
  +0x120 with its inverse +0x220, and has no bound test of its own. Each slot not yet claimed goes
  through that candidate's frame and is marked "with" that candidate.
- **The push-out** (`./kit fn 0x001f6c40`). For a car it takes the owner branch, because the car's
  class +0x1b0 is 2. The spin is vt+0xbc, the velocity is vt+0xb8, and the frame is the car's node
  world with origin car+0x190. S136 proved these are `Moving`'s.

## How it was proved

`./kit check -- -p game-sim --test ragdoll_cars`: 4 passed. All four run on `context/states/s89-along`
(free06, train moving) with the disc's car meshes.

- **`the_sphere_test_on_a_car_answers_as_the_originals`**: function oracle, 20,000 generated spheres.
  - Setup: each sphere is stood near a random vertex of one of the 21 cars. Radii are 0.4, 0.5, 0.75
    and random values from 0.1 to 1.
  - Each goes through the original's `sphere_collider_clear` + `sphere_collider_pair` on a real bone
    sphere collider, against one of three candidate lists:
    - one car;
    - a car, then its neighbour on the train;
    - the same car twice.
  - The same lists go through `PathCar::touch` with the held distance.
  - Compared: the count, which collidable holds the record, the held distance, and the world point
    and normal. All bit for bit.
  - Coverage: 12,848 hits. The second car took the record 67 times, and the first kept it 8,540
    times. The same-car-twice case checks the strictly-nearer rule, and it held.
- **`the_push_out_against_a_car_is_the_originals`**: function oracle, 10,000 generated bones.
  - Setup: the bones are on the state's bone collisions (found by +0x88/+0x7c), with contacts near
    one of the 21 cars.
  - The contact's other is the car's real mesh collidable, so the original runs the car's own
    virtuals.
  - The port uses `moving(Against::Mover(Moving::of(car+0x2e0, node world, app 1/dt).mover()))`.
  - Compared: the bone's position, velocity, quat and rates, and the contact's dv, bit for bit,
    swept cases included.
- **`a_ragdoll_thrown_into_the_trains_side_bounces_off`**: the sim's tick order (cars, then bones).
  - Setup: the wreck ragdoll's main bone is put 2.5 ft out from the wagon's flank (the sixth car, a
    boxcar), moving with it plus 20 ft/s at it, 17 ft back from its middle (away from the door
    opening).
  - With the cars: the main bone stops at its radius from the wall (x 5.21 against the wall at 4.5).
    Its speed at the wall goes 20 → ~0, its slide along the wagon goes to under 1 ft/s, and it
    slides down the wall.
  - Without the cars (the port before this task): it is through the wall at tick 5 and at x −5.2 by
    tick 19.
- **`a_ragdoll_landed_on_a_wagons_roof_moves_with_it`**:
  - Setup: dropped from 4.5 ft above the roof's middle, at the wagon's velocity, for 60 ticks.
  - With the cars moving: it lands on the roof at tick 27 and stays within 5 ft of where it landed,
    at under 1 ft/s over the wagon after landing.
  - With the wagon in the scene but reported still: it slides back off the 24-ft half deck by tick 42.
  - Without the cars: it falls through the roof.

## Not verified / not 1:1

- **No capture of the original's ragdoll on the train.** The sphere test and the push-out are proved
  function by function. The two ride tests show the port's whole ragdoll against the train, but no
  recording of the original doing it exists. A bounce's size or a slide on the roof is the port's
  result.
- **The sweep against the cars is not ported.** `WreckBikeScene` has no swept test, against the bike
  or a car. The original's segment sweep against a car mesh was not
  traced. A fast bone can tunnel through a thin wall between frames. The train moves about 1.8 ft
  per tick, the same as the original's once-a-tick car step.
- **Candidate order.** The port offers the bike first, then the cars in the props' (traffic) order.
  The original's static-then-moving order agrees for the bike. The block list's own order of the
  cars was not read. Order only matters for an exact tie of plane distances.
- **Other moving or still things.** Moving toys and trucks that are not path cars still are not in
  the bones' scene. Neither are the track's static props (fences, buildings): the bones still meet
  only the terrain and the bike. Filed as follow-ups.
- **Ignore lists.** The pair filter was checked for the bone spheres against the cars in the riding
  s89-along state only. No car was found on a bone's ignore list, and the port never ignores a car.
- **The probe's frame.** Cars are measured from where sphere 0 last stood. That is the same probe
  as the bike's, which S105 proved, but the car distances were not compared against a live search's
  list.
- **Without a bike mesh there are no cars.** That is the existing condition for the bones' scene. A
  shell always sets the mesh, but a test kit without it gets the old open scene.
- **Door openings and one-sided walls.** The car meshes are thin and open where the original's are.
  The sphere test only answers from a triangle's front, so a bone that gets inside a car (through a
  door) does not meet a wall from behind. This was not checked against the original
  beyond the generated spheres, which include inside centres.

## Next

- Follow-ups filed: static props in the bones' scene, and the segment sweep against cars.
