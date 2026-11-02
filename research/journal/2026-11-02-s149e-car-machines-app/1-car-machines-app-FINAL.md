# 1 — car machines in the app (FINAL)

Task S149e. 2026-11-02.

## What I did

- `game-sim/src/car.rs`:
  - `CarModel::read` takes a car's tire nodes and the model's bounds.
  - `Machine::new(car, model, at, facing, up)` builds a fresh car as the original's constructor does:
    - inertia from the model's full extents (`Part::inertia`);
    - the wheel mounts at each tire node plus the tire's width out;
    - the body's centre of gravity is CofGAI (an undriven car);
    - the launch and preload state as the constructor leaves them;
    - undriven tires at the undriven top wheel speed and friction scale 1;
    - the turn from `flight::spawn_turn(facing, up)`.
  - `Machine::drive(on)` hands the car to a player, or takes it back.
- `openmx`:
  - `vehicle::Kind` gains Buggy, Monster and Baja. A tape's `switch N monster|truck|baja|buggy` takes them.
  - `vehicle::CarRide` is one car of one seat: its template, the car taken at the rider's spot, stepped
    each tick on the seat's pad through the seat's controls table.
  - `Garage::cars` holds them, and `switch_car` moves the player between the bike or plane and a car.
    `Lot::cars` keeps the templates, read once at setup from machine.txt's .phy and model files.
  - `menu::Machine` gains the three cars. From the bike or plane the pause's Switch To Machine row offers
    the other of the two and every car. From a car it offers the bike and the plane.
  - `chase::driven` makes the camera's target from the car. The bike's follow camera follows it.
  - The shell says `car: <name> N ft from where it was taken, …` when a car is left, and at the end of a
    ride that ends in one.

## What the original does

- **The constructor.** The addresses are in research/oracle.txt under "S149e".
  - It builds the part with inertia from the model's bounds, as `Part::inertia` on twice the half
    extents. It matches RAM to about 1e-6 relative.
  - For each tire it finds the tire node by name, takes the node's world position with the model
    placed at the spawn, and brings it back to the car's frame through the inverse. The mount is that
    plus (width, 0, 0) on the tire's side.
  - The launch timer, scale and speed and the preload limits come from the .phy. The body's cofg is
    the AI one until a player drives the car.
- **RAM.** `tests/car_spawn.rs` reads the parked truck (first_recording), baja (s149a-free07) and buggy
  (s149a-free11). These match bit for bit: inverse mass, damping, cofg, gravity, yaw limit, launch,
  preload, top wheel speeds and friction scale. The inverse inertia matches to 1e-4 relative and the
  mounts to 2e-3 ft.

## Checks

- `./kit check -- -p game-sim --test car_spawn`: 2 passed.
  - The constructor test above.
  - Each fresh car drives off on the gas on free07's ground: truck 455 ft, baja 470 ft, buggy 337 ft
    in 180 ticks, upright and grounded.
- `./kit check -- -p openmx --test car_switch`: 1 passed, run with `--nocapture`, no skip. The binary
  runs headless on free09, free07 and free11 on a full-throttle tape: `switch 30 <car>`, `switch 200 bike`.
  - Each tape gives "vehicle: bike / <car> / bike".
  - On the gas the monster went 309 ft, the baja 286 ft and the buggy 327 ft.
- `./kit check -- -p openmx --no-fail-fast`: every test binary passes but `biplane_particles`
  (`a_plane_flown_into_the_ground_puffs_where_it_hits`: first puff on the contact tick 11, not 12).
  That test mounts only particles.rs and game-sim, and this task changes neither for it (car.rs only
  gains code). It fails the same alone, so it is not this task's.
- `./kit audit`: its one leak is plan/DONE.md:519 (P256's line).

## Not verified / not 1:1

- The mounts differ from RAM by up to 0.0013 ft (the buggy, whose spot leans). The port takes the node
  in the car's frame directly instead of placing the model and inverting. That rounds differently.
- The launch timer is assumed to start at the launch time. The parked images all show the timer full.
- A car is taken at the rider's spot, dropped from a rest height the port works out (0.5 ft over the
  lowest tire). It is not parked where machine.txt puts it. All three cars are offered on every track,
  not only on their own.
- On free09 the monster truck, launched from the start hill at full throttle, rears past vertical
  (up.y −0.14) and comes back down. Whether the original does the same there was not checked.
- The engine noise input to `Machine::step` is zero. The car is not heard.
- Not drawn: no car model and no PelvisBox driver pose. In a window the camera follows an invisible car.
- No car cameras (MonsterTruckCPresets etc.). The camera is the bike's follow camera aimed at the car;
  its target was not read from the original.
- No dust or particles from the cars.
- Cars are not in the bays, the offer, the markers or online play. An online join gets no cars.
