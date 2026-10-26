# 1 — moving platforms (FINAL)

Task S136. 2026-10-26.

## What I did

- Found how the original's wheel takes the velocity of a path car it stands on (below; addresses in
  `research/oracle.txt`'s S136 block).
- `game_sim::traffic::Moving`: one car's motion over the tick — its pose after, its velocity and its
  spin. It is made from the poses before and after the step, the same way the car's virtuals make
  it. `Traffic::frame` keeps each car's pose before it places them and writes `Prop::moving` after.
  `Moving::at(point)` is the wheel branch: the car's velocity, and the velocity of the car's point
  under the wheel.
- `world::Prop` gains `moving: Option<Moving>`, which is `None` for every prop that is not a path car.
  It is not hashed and not snapshotted. It is rewritten from the cars' poses (which are in the
  state) every tick, before the rider steps, so a rollback that restores the cars restores it on the
  next frame (rule 3). The state hash does not change.
- `ride.rs`:
  - `wheel_hits` gives each hit the car it is on, as `(handle, Moving)`. `contacts` writes the
    wheel's vehicle handle and kind 2 for it, and returns the movers.
  - `under(wheel, movers)` reads the wheel's vehicle word and gives `Moving::at(contact point)`.
  - `wheel_contact_velocity(own, chassis velocity, under)` subtracts the car. `tire_forces`,
    `share_load` and `wheel_integrate` read through it.
  - With no car, every result is what it was before, bit for bit: the second operand is the
    original's `(0,0,0,1)` and `sub_xyz` leaves `w` alone.

## What the original does

- **Saving the old pose.** `path_car_update` (`./kit fn 0x00115540`) copies the node's world matrix
  to car+0x2e0..0x31c at its top. That is the car's pose *before* the step. It does this before the
  dt test, and skips it if app+0x325 bit 3 is set.
- **The car's virtuals** (vtable 0x0044d370; every one of the 21 cars has it, all of kind 2 with
  origin (0,0,0,1)):
  - vt+0xb8 is the velocity: (node position − old position) × app+0x348.
  - vt+0xc0 is the turn: cross(old z, new z) + cross(old y, new y), × app+0x348.
  - vt+0xbc is the spin: the turn put through the node's rotation.
  - app+0x348 is one `div.s`, 1/app+0x344, written only when dt > 0.
- **The wheel's vehicle branch** (`./kit fn 0x0025b710`; +0x1a0 == 0, +0x1f0 == 0, +0x1f4 != 0):
  1. +0x1b0 = the car's velocity.
  2. local = unpoint(node, contact point).
  3. +0x1c0 = rotate(node, cross(spin, local − origin)) + velocity.
  4. +0x170 = the wheel's own point velocity − +0x1c0.
  5. +0x180 = chassis+0x260 − +0x1b0.
  6. +0x190 = unrotate(chassis node, +0x180).
  7. +0x1a0 = 1 caches the result for the substep.
- **When the branch runs.** Only wheel_clear clears +0x1f4. So a wheel whose terrain ray wins over
  the car's mesh still subtracts the car for that substep.
- **+0x1e0 is not used here.** The brief's idea (from S129) that the collidable's previous matrix
  feeds the contact is not what the found code does. The mesh collidable's +0x1e0 (written by
  vt+0xac 0x001edb60) has no reader in the contact code that was found. The wheel goes through the
  car's own +0x2e0 copy.

## How it was proved

All in `crates/game-sim/tests/traffic.rs`, on `context/states/s89-along` (free06 with the train
moving):

- **`the_original_drives_the_cars`** now also checks every car's `Moving` against the original's
  vt+0xb8 and vt+0xbc, run on the same RAM after each of the oracle's ticks. All 6300 of 6300
  car-ticks match bit for bit, turning ones included.
- **`the_frames_one_over_dt_is_the_ports_divide`** checks that app+0x348 is `F::ONE / dt`, and that
  every car is kind 2 with origin (0,0,0,1).
- **`the_wheel_on_a_car_moves_as_the_originals_does`** runs 20,000 generated cases through the
  original's wheel_contact_velocity: random contact points, own velocities, chassis velocities and
  chassis nodes, three in four of them on a car. It compares +0x1b0, +0x1c0, +0x170, +0x180,
  +0x190 and +0x1a0 against `Moving::at` and `wheel_contact_velocity`, bit for bit.
- **`a_bike_parked_on_the_train_rides_it`** parks a stock bike, both brakes held, on the sixth car (a
  boxcar with a flat roof) for 150 ticks. The tick order is the sim's: cars, then rider.
  - **Air drag off:** the bike moves 0.08 ft on the deck and 0.0007 ft up after settling. It stays
    on for all 150 ticks and ends at 55.13 ft/s on a 55.15 ft/s wagon.
  - **Control, wagon motion kept from the wheels:** the bike is off the deck after 39 ticks and at
    rest.
  - **Stock bike's drag on:** it slips back about 8 ft/s and leaves the back of the wagon at tick
    90. Its control leaves at tick 29.
  - **Why it slips:** air drag (`mv::air_drag`, every substep) reads the chassis' velocity over the
    world. At 55 ft/s that pulls harder than both braked tires hold. A run with drag zeroed by hand
    confirmed that this is all of the slip.

`./kit check -- -p game-sim --test traffic`: 5 passed.

## Not verified / not 1:1

- **No capture of the original on the train.** The brief asks to compare against S129's recording,
  and S129 is not done. The oracle tests prove the car velocity and the wheel branch, function by
  function. That the bike as a whole behaves like the original's on the roof is not proved. In
  particular, the stock drag's slip is a port result that no capture confirms or denies.
- **Air drag stays absolute.** That is what the found code does (it reads chassis+0x260). No other
  drag or air term in the original was searched for anything that would subtract a platform.
- **Order of cars and bike.** The port steps the cars before the rider every tick. The original's
  order of `path_car_update` and the bike's step in one frame was not checked. If the bike runs
  first there, its wheels read last frame's car velocity.
- **Not ported:**
  - The body/chassis probes' velocity kill (`velocity_bundle`) still works against the world.
  - The owner branch (+0x1f0 != 0) is not ported.
  - flight_search's `Scene::velocity` (the biplane and body contacts) still gives zero for a car.
- **Other moving platforms.** Only path cars get a `Moving`. Moving toys (`Toys`, HasPhysics props,
  trailers and trucks that are not path cars) give `None`, so a bike on one is still carried by the
  world.
- **First frame.** `Moving` is `None` until the first `Traffic::frame` after the cars are attached.
- **app+0x325 bit 3.** When this bit is set the original skips the old-pose copy. The port never
  skips it.
- **Paused frames.** At dt ≤ 0 the port takes 1/dt as 0. The original keeps the last app+0x348.
- **The pose the wheel reads.** The port's `under` reads the contact point after suspension.
  `share_load` and `wheel_integrate` do not write the +0x190 copy (that was so before this task).
  Neither was compared against a substep of the original.
- **Tiled worlds** (world_ultra and the like) have no props, so no platforms.

## Next

- S139: once S129's capture exists, compare a bike on the train against it, drag slip included.
- S140: moving toys, the body contacts and the owner branch.
