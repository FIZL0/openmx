# 1 — a car in a seat's lane (FINAL)

Task S249. 2026-11-02.

## The design

Online, a seat's whole state is its `Sim`: ggrs saves it, rolls it back, hashes it and sends it in
bytes. A plane flown online is already the `Sim`'s own lane (`Sim::plane`, N17), not a garage
beside it. A car is now done the same way.

- **game-sim.** The new type `car::Lane` holds the car's kind, the `car::Machine` and its
  collision object. `Sim::car: Option<Box<car::Lane>>` holds it while the car is out.
  - `snapshot` writes the lane in a new tagged section (`S249car`) after the event section, so old
    bytes still decode. Every field of the car's state goes on the wire except the collision's ray
    probe, the car's `m.col`. That is a constant of the disc, so decode copies it from the seat's
    car on this machine, as the plane's probe is copied.
  - `hash` walks the lane as one `car` blob, only while a car is out, so the leaves of a seat on
    its bike do not move. `Sim` grew 8 bytes (tests/hash.rs pins 12120).
- **The switch byte.** `vehicle::Wanted` had the machine in bits 0–1, and only bit 7 was free. The
  machine number is now bits 0–1 plus bit 7: 0 none, 1 bike, 2 biplane, 3 buggy, 4 monster,
  5 baja. Old bytes read as before. The helicopter is still no pick online.
- **The switch.** `vehicle::switch_lane_among(to, plane, cars, sim, world)` builds a garage from
  the lane: the plane, as before, plus the copies of the car being left and the car being taken,
  with the lane's car put back in. It then runs the same `Garage::switch` offline runs:
  - Onto a car, `CarRide::take` stands it in the seat's bay on its own track, or at the player
    where the track has no bay for it.
  - Off a car, the bike or plane is stood where the car is.
  - The plane no longer has to exist for a car switch; it is only needed for a switch onto the
    plane.
  - `switch_lane` is the old call, passing no cars.
- **The step.** `vehicle::drive_lane` makes the seat's copy of the car out of the lane and runs
  `CarRide::step` on it, the same step offline's `Garage::drive` runs, after the track's path
  cars. The engine's smooth noise is built from the fixed `CAR_NOISE_SEED` in each step's copy.
  That gives the same tables every time, so it is a constant, as it is offline.
- **The cars each seat drives.** A car's physics read its livery's model (tire nodes, bounds), so
  every machine must step seat `i`'s car on the same copy. `Lot::online_cars` holds them:
  - seat `i`'s `cars_for(i)` from the lot parked for `net::SEATS`, the same lot the online biplane
    bays come from;
  - each car stood on the ground in its bay, as the offline lot's are (S245).
  - `Online::cars_on` hands the list to `Rides::cars`.
- **The session's step (`net/rides.rs`).**
  - The switch is `switch_lane_among` with the seat's cars.
  - A seat in a car runs `drive_lane` and nothing else that tick: no bike step and no markers,
    since offline a car has no markers yet (S149e).
  - It is left out of the other bikes' rivals and plane-bike lists, as a flying seat is.
  - `Rides::probe` puts a decoded car's missing ray probe back from the seat cars.
- **The shell (`main.rs`).**
  - The online pause/tape `Act::Switch` accepts a car when this seat has a copy of it.
  - Each drawn seat's garage is given its session seat's copy of the car the lane drives, so a
    peer is drawn in its own livery. This also fixes "online join builds no cars": peers' garages
    had none.
  - `Garage::follow` copies the lane's car into the garage for the picture. It marks where the car
    was taken on the first frame it shows the car, and puts away the cars the lane does not drive.
  - When a seat's car is left, its report line is printed, as the offline switch prints it.
  - New flag `--online-script F:WORD,...`: at this machine's online frame F, the first player picks
    a machine, or holds/releases Cross (`gas`/`lift`). An online ride rides no tape, so this is the
    two-shell test's driver.

## How it was checked

- `tests/online_car.rs`, `two_online_shells_see_the_host_switch_into_the_monster_truck_and_back`.
  - Setup: two `openmx --headless` shells, host and guest, on free09 online over localhost,
    420 ticks. The host runs `--online-script 40:monster,41:gas,200:lift,330:bike`.
  - Both shells print `vehicle: bike` (the load), then `vehicle: monster`, then `vehicle: bike`.
  - Both report 0 desyncs, and their state hashes before tick 300 (truck out, coasting) are equal.
  - Both print the same `car: monster ...` line when the truck is left. In one run that was
    853.4 ft from where it was taken, at 4586.7, 133.6, 2984.6, 103.5 ft/s. The numbers differ
    from run to run, as netplay runs do (an earlier run gave 856.9), but host and guest agree.
- `tests/online_car.rs`, `a_seats_car_lane_drives_as_the_offline_garage_and_through_its_bytes`, run
  for each of the buggy, monster and baja on a flat floor with the disc's cars:
  - `switch_lane_among` puts the car and its collision object into the lane.
  - Over 120 ticks (90 on Cross), `drive_lane`'s car matches an offline `Garage` with the same pads
    on every tick (Debug strings equal, which means equal bits).
  - Bytes taken at tick 45 decode with an empty probe. With the probe put back as `Rides::probe`
    does, they hash the same, and drive on hash-equal to the original for the remaining 74 ticks.
  - Switching back to the bike empties the lane and stands the rider within 1 ft of the car.
  - A car pick with no copy of the car is refused.
- `./kit check -- -p openmx --no-fail-fast`: 2563 passed, 1 failed. The failure is
  biplane_particles `a_plane_flown_into_the_ground_puffs_where_it_hits`, red on main before this
  task (S246 lists it).
- `./kit check -- -p game-sim --no-fail-fast`: everything passes except `the_humans_tricks`, the
  known red on C6's capture in this slot. `hash` passes after the size pin moved to 12120.

## Not verified / not 1:1

- The original has no online mode, so nothing here is checked against it. The car's own step is
  S149e/S229's, unchanged.
- Only the monster truck on free09 was tried between two shells, and only the host switched. The
  buggy and baja online, a guest switching, two seats in cars at once, and a join or resync while
  a car is out were not run end to end. The codec round trip covers the bytes a join or resync
  carries, but only in process.
- A car out online meets no other seat. It is not in the seats' contacts, so other bikes pass
  through it. The bike the player left stays where it stood and is out of the contacts too.
- Not looked at on screen: the online car's drawing, its camera (S246's, snapped by `took`) and its
  engine sound (S247's, which reads the garage's copy).
- The pause menu's machine list online was not checked to offer the cars. The `Act::Switch` it
  sends is accepted now.
- The host's modifiers do not touch the cars online (or offline).
