# 1 — car hull among the bikes (FINAL)

Task F26. 2026-11-03.

## What I did

- **The hull files.** `machine.res` keeps each car's collidable mesh beside its ray probe: the
  stock skin's `%sf.col` (`buggyf.col` 20 triangles, `bajaf.col` 12, `monstertruckf.col` 96). The
  `2`, `_ss` and `2_ss` skins' files are the same sizes. `car::Kind::hull` names them.
  `vehicle::CarRide::new` reads the hull into `CarRide::hull`, as it reads the `m.col` rays.
- **`rider_contact::Hull` carries its category.** `gather_met` used to hard-code
  `flight::COLLIDABLE` for every hull. A plane's hull now says `flight::COLLIDABLE` and a car's says
  `car::COLLIDABLE` (0x52058). The riding bike's category lists ignore neither. Both collide with
  everything (0xffffffff, the plane's and the heli's mask).
- **The car's side** (`crates/game-sim/src/car.rs`):
  - `Machine::hull` stands the mesh where the collision object's node will stand it: the body's
    pose (`wreck_bike::node_of`), the same thing `Collision::search` sets the record's node to. Its
    capacity is the record's.
  - `Machine::flown` and `Machine::pushed` are `Flight::flown` and `Flight::pushed` on the car's
    body and collider. They hand the bike's resolve a `rider_contact::Flown` and write it back.
  - `Machine::step_met` is `step_struck` with a `flight::Met`. After the car's own search it files
    the mirrors on the record (`Collision::file_mirrors`, the heli's from C21: touching flags set,
    up to the room left). Then the car's resolve runs every substep with the riders' chassis as its
    others. A mirror's spring and friction read the other side and never move it (`contact.rs`:
    only `impulse_on` on a kind-0 entry moves the other), so nothing is written back to the bikes.
    `step_struck` is `step_met(None)`.
- **The shell** (`World::simulate`):
  - Driven cars' hulls go after the flown planes' in `contacts_met`'s list. A bike's entry on car
    `j` names `n + planes + j`. `Bump::planes` gets the planes' `Flown` and then the cars', so the
    props stay after them all (`b.vehicles.len() + b.planes.len()`).
  - The car's seat gets `met_riders` over its mirrors, at the car body's position, through
    `Among::met`. `Garage::step_among` hands it to `Garage::drive_met` and then to
    `CarRide::step_met`.
  - `met_after` writes each pushed body back with `Garage::hull_pushed`: to the car up if there is
    one, otherwise to the plane.
  - A car is in the world on the same terms as a plane: its seat is not waiting for its pad and is
    riding (not frozen on Start).

## What the original does

Not read for this task beyond what N22a, S229 and C21 established. The collision object's search
keeps every object in one list, and an object's ray strike puts a kind-0 entry on the striker and
the kind-1 mirror on the struck (N22a for the plane, C21 for the heli's hull). The car's collision
object is the heli's class (S229), with category 0x52058 off S149a's switched truck. Following the
brief, this task assumes the car's hull meets a bike's body rays as the plane's does.

## How it was proved

- `split_contact::a_split_bike_is_stopped_by_the_other_seats_car` (`crates/openmx/src/main.rs`).
  Setup:
  - Two seats on a flat floor, with race125p's collision tuning and the bike's collision files from
    `free09_ss_2.res`.
  - Seat 1 switches into the buggy where its bike stands, 25 ft up seat 0's way and facing along
    it, and stays on an idle pad.
  - After 30 ticks to settle, seat 0's bike is given 30 ft/s into the buggy's tail for 45 ticks.

  Results with the hull: the bike ends at z 13.9, short of the car. The car is pushed to 2.42 ft/s
  and moves 1.94 ft. With `CarRide::hull` removed (the control in the same test), the bike rolls
  through to z 41.9 and the car only creeps (0.94 ft/s, 0.87 ft).
- Negative controls, run by hand and reverted:
  - With `Garage::hull_pushed`'s car write-back stubbed out, the car reaches only 1.05 ft/s and
    moves 1.00 ft, and the test fails.
  - With the mirrors not filed, the car moves 1.86 ft instead of 1.94. The test still passes, so
    the mirrors' share is not singled out by an assertion.
- Neighbours:
  - `./kit check -- -p openmx --bin openmx -- split_contact`: 4 passed.
  - `-p game-sim --test car_drive --test car_step --test car_spawn --test car_tire --test plane_contact_capture --test physics_car_struck --test ragdoll_cars`: 39 passed.
  - `-p openmx --test car_switch --test online_car --test car_particles --test car_tire_tracks --test car_engine_sound --test car_picture --test car_camera`: 184 passed.

## Not verified / not 1:1

- **No capture of the original.** No split-screen capture of a bike striking a car exists. The
  test proves the strike happens and pushes, not that the numbers are the original's. Filed as R22.
- **Where the mirrors go on the car's record.** They go after the car's own search's entries, as
  the brief says and as the plane's and the heli's are. That order is only right if the original's
  collide pass searches the car before the riders. Not checked.
- **When the car steps.** The car's seat steps where `World::simulate`'s walk puts it: after the
  flying seats, among the bikes in reverse seat order. The bikes' push on the car is written back
  after each bike's pass. Whether the original steps cars before or after its riders, as it steps
  planes first, is not known.
- **The car's own rays do not strike the bikes.** Its search sees only the world's meshes and
  plants, so it does not push a bike with entries of its own. The bikes' strikes on its hull are
  the only meeting. Unported, like the plane's before N22b.
- **The stock skin's hull for every livery.** The rays already make the same choice. The `2`
  skins' `f.col` files are the same size as the stock ones, but their bytes were not compared.
- **Online (F23) is untouched.** `net/rides.rs` and `among_met` still leave the lane's car out.
