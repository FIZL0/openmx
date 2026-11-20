# F76 — a split-screen car's own rays strike the bikes, and the car steps first

R22 showed the original's order on a split-screen tick: every record collides first (the car's
before the bike's), then the dynamics world steps the cars (4 resolves of the car record), then the
players update (2 resolves of the bike record). On the strike (frame 12012) the car's 3 body rays
file 3 kind-0 entries on the car naming the bike, with their mirrors at the head of the bike's list.
The car's first resolve stops the bike (46.16 -> 4.28 ft/s) and pushes the car (3.42 ft/s). The
bike's resolves after it write nothing of the car's body, only the car record's +0xa0.

## What changed

- `car.rs`: `search_met` is the car's search with the bikes in its scene, as the plane's N22b
  search has them (`Scene::flown` with `Bikes`, `owned` velocities). It renames the entries on the
  bikes to their players, marks them `pushed`, and hands the bikes' mirrors back in `met.struck`
  named after the car. `step_met` and the props path use it when `strikes_bikes(met)` holds (a
  name for the car, the bikes' collision files, and one chassis place per bike).
- `flight.rs`: `Met` gained `rides` (each bike's frame where it stands) and `bike_col`.
- `rider_contact.rs`: `met_car` builds that met for a driven car: `met_plane`'s, plus the bikes'
  frames (`plane_bikes`) and the collision files.
- `main.rs` (`World::simulate`): driven cars step right after the planes and before every bike
  (the old order stepped other seats last-first, so a seat-0 car came after a seat-1 bike). A
  driven car j is named `n + flown + j`, as the bike's resolve names it. Its met carries the
  mirrors other cars left on it, and `met_after` puts its mirrors at the head of each bike's list
  (`struck_first`), as for a plane.
- The bike's resolve is unchanged. Once the car's resolve has pulled them apart, the bike's
  impulse is never a hard one, so it pushes the car's body no more. The spring and friction write
  only the car collider's `cofg_world`, which is the original's +0xa0.

## How it was checked

- `tests/ai/split_car.rs` `f76_the_ports_car_strikes_and_stops_the_bike_as_r22s`: R22's image run
  whole in the interpreter to the strike. Every input read off RAM as its call finds it.
  - The port's `search_met` from the car's body at its collide, the bike's frame at its collidable:
    the 3 car entries and the 3 mirrors equal the original's field for field, bit for bit.
    `pushed` = [true, false].
  - The port's `contact::resolve` of the car's first resolve, weighed by the car collision's mass
    (77.64, what the step weighs by; the record's +0x64 is 155.28 and gives the wrong force): the
    car's velocity, shock, rates, twist, force and moment, and the bike chassis' velocity, rates,
    shock and twist equal the original's bit for bit. The bike goes 46.16 -> 4.2808557 ft/s.
- `./kit check -- -p game-sim --test ai split_car`: 2 passed (R22's and F76's).
- `./kit check -- -p openmx --bin openmx split_contact`: 7 passed, among them F26's
  `a_split_bike_is_stopped_by_the_other_seats_car`.
- `cargo check --release -p game-sim --tests`: builds.

## Not verified / not 1:1

- No whole-tick port replay of R22. The search and the car's first resolve are proved in
  isolation. The car's other 3 resolves, the bike's 2 resolves, and where both end up on 12012 are
  not compared with the port's.
- The test reads the car's resolve inputs off RAM, so it does not prove the port's car reaches the
  strike with the same body. The car's step itself is S-series work.
- Online (`vehicle.rs` drive lanes, F23) still steps the car without rays on the bikes. Filed as a
  follow-up.
- The seat order changed only for driven cars. A car is still stepped after the planes, and R22
  has no plane to show that order.
