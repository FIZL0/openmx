# 1 — p31 the biplane takes gas (FINAL)

Task P31. 2026-10-20.

## What I did

- **`Flight::frame` wired in the original's order** (`crates/game-sim/src/flight.rs`), following S41's
  wiring list. The plane is a `biplane::Body` in a one-iteration dynamics world at rate 115:
  - preforce: the shock peak is cleared, then the controls, then the three tires;
  - per substep: gravity, and while not wrecked the autopilot, engine and aero (forces and moments in the
    body frame), then the three tires' probe and their forces; then the body is integrated;
  - post: the contacts measure (which sets the aero's true speed), the crash test, wreck or unwreck,
    then the flight measure, attitude and air.
- **The built body matches the save state bit for bit.** This covers inverse mass, both dampings,
  centre of gravity, gravity force and the three tires off `phy::Gear`. The `.phy` names two of the
  gear's fields the other way round: the file's `BrakeFriction` is the tire's max braking and vice
  versa. Test: `the_body_and_the_wheels_are_built_as_the_original_builds_them`.
- **The bug that stopped it taking gas.** The tire mount's speed is the signed body-frame speed along
  the nose (the word the autopilot reads), not the airspeed. With the airspeed, rolling friction never
  changed sign, and a parked plane rolled backward faster and faster.
- **Pieces added to `biplane.rs`:**
  - `Body::add_force_at`, `Body::add_velocity` and `impl WheelBody for Body`;
  - `Crash::step_in`, which takes the area as an `Option` (`None` means no terrain, so no map-edge
    throw);
  - `Tire` derives the usual traits.
- **The shell flies it** (`crates/openmx/src/vehicle.rs`, `main.rs`, `hud.rs`):
  - The shell `Plane` carries a `Flight` built off `machine.res`'s `biplane.phy`. Its area is the
    terrain layout's extent. With the plane up it steps every frame on pad 0 through `PlaneInput`.
  - The model is drawn at the body's pose (its nose and up rows), and the prop blur turns about z by
    the engine's prop angle. A wrecked plane shows the bent cone instead
    (`game_render::biplane::Biplane::turned_parts`).
  - The plane's camera targets the body's position, nose, up and velocity.
  - The HUD's first two lines become airspeed and throttle. The per-second log line gives the plane's
    position, throttle, airspeed and whether it is on the ground.
- **Tests:**
  - `the_plane_takes_gas_off_flat_ground` (game-sim, 30 Hz on `Flat`);
  - `a_biplane_session_flies` in `crates/openmx/tests/ride.rs`. It was ignored before; it now uses the
    disc's `biplane.phy`, runs at 60 Hz through the garage, and the bike is never stepped. Throttle
    held, the plane gains airspeed and leaves the ground. Throttle closed, it loses at least 50 ft in
    10 s.
- The hash test's size pin went from 7120 to 9936. `Flight` grew by the body, tires, crash, area and rng.

## What the original does

Per frame, inside the dynamics world, in this order: preforce listeners, n = ⌊dt·115⌋+1 substeps of
(force listeners, integrate), then post listeners. This is S41's list; nothing here departs from it
except what the next section names. The crash test reads last frame's airspeed and up, because the
flight measure that refreshes them runs after it.

## How it was proved

- **The build:** a save-state capture (`biplane.p2m2_SaveState.p2s`). Every built field compares bit for
  bit with the plane object in RAM.
- **The flight:** behaviour tests only, not the original's numbers. On `Flat` at 30 Hz the parked plane
  settles nose-up (~10.3°, the state's attitude), leaves the ground at ~6 s of full throttle and climbs
  to ~515 ft. At 60 Hz in the garage it does the same.
- **In the shell** (`--vehicle biplane` on free06, a hand-written 60 Hz pad tape, two screenshots looked
  at):
  - the plane rests at 0.0 mph;
  - it lifts off at 7 s, about 58 mph, and climbs to 465 ft at a steady ~75 mph;
  - at z ≈ 180 the map-edge throw fires, which is a real wreck and not an accident of the area;
  - throttle shut, it comes down, lands on the heightfield and rolls out;
  - the thrown wreck's unwreck puts it back at the spawn at 41 s.
- **The ratchet:** `BIPLANE_AGREES_THROUGH` is still 0. No biplane `.csv` capture exists in this worktree
  or the main checkout, so there is nothing for it to agree with.

## Not verified / not 1:1

- **No frame of the flight is compared with the original.** There is no biplane capture to replay
  against (see the ratchet above). The takeoff, climb and descent are plausible, not proven.
- **No contact search.** S45, S46 and S48 are open, so the plane's world is the ground query alone,
  `Flat` in tests and the terrain's heightfield in the shell. It passes through props and trees. The
  contact resolve is empty, so no crash by contact can happen, only the map-edge throw and the
  crash test's own checks.
- **The inverse inertia is a measured constant** (the save state's four words). The original derives it
  from the plane's model box, which the port does not read. The wreck's inertia division therefore
  starts from that constant; its bits after a wreck were not compared.
- **The spawn:** the quaternion from `basis(facing, up)` and the origin-plus-turned-cofg placement are
  the reading of the reset routine. They were not compared with a state taken right after a reset.
  The original's ground-height snap at the spawn is not ported; the shell spawns at `machine.txt`'s y,
  which on free06 is the dirt (135.0 ft).
- **The area's pairing:** the terrain layout's (width, breadth, width scale, breadth scale) feed the
  area's (x count, z count, x size, z size). This pairing was not checked against the original's
  terrain object. The throw firing near z = 180 on free06 is consistent with it, but it is not a proof.
- **Noise and rng:** the smooth noise is built from the flight's rng on its first sample, and the rng's
  seed is the default. Neither the timing nor the seed was read from the original; both reach only the
  engine note's wobble.
- **Not simulated:** the touchdown sound and rumble, and the engine sound.
- **The pilot is not seated in the shell.** The rider is hidden with the plane up. S35's surfaces (flaps,
  rudder) are not drawn either; only the prop turns.
- **The prop's turn** replaces the rotation of `PropDummy`'s model-space transform. That equals its
  local rotation only if its parents do not rotate, which was not checked on the model. Positive
  angle was taken in the row-vector sense of S35's test.
- **The camera's target** is the body's own fields. Which of the plane's fields the original's plane
  camera reads was not traced.
- **The shell does not use `Sim::plane`.** The shell's `Plane` owns its `Flight`, so a biplane session's
  `Sim` tick count stays 0. A tape recorded in the plane replays the plane through the shell, but the
  plane is not in the `Sim`'s hash.
- **The tests' thresholds are generic flight:** >50 ft/s, >100 ft climb, a 50 ft fall in 10 s. They are
  not the original's numbers.

## Next

- A PINE capture of the human's biplane flight in `watch/biplane*.toml` would let `BIPLANE_AGREES_THROUGH`
  be raised. That needs the capture's `.csv` in the worktree.
- After S45, S46 and S48, give the plane the scene's props through `Ground` in `flight.rs`.
