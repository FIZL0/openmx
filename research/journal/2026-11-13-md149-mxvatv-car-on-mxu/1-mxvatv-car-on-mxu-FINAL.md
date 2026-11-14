# 1 — mxvatv car on mxu (FINAL)

Task MD149. 2026-11-13.

## What I did

- Added `atv_car::Ride` (crates/game-sim/src/atv_car.rs, section "MD149"): MX vs. ATV's car as a shell
  steps it on an MX Unleashed world, one `Ride::step(pad, table, dt, world)` a tick, as `quad::Ride` does
  for the quad. Each tick:
  - the pad goes through the car's own input step (`atv_car::input`, the stick read through
    `AxisRange::PLAYER`), and its words are laid into `State` (`Ride::controls`);
  - `Ride::frame_on` puts the world's box (`quad::world_bounds`) and map area (`World::area`) into
    `Vehicle`;
  - then `atv_car::frame` and `atv_car::class_update` run through `Own` → `PutBack` → `Pieces` →
    `OnWorld`, with `WORLD` = (1 iteration, 115 Hz);
  - then the put-back spot is kept from MX Unleashed's put-back record (`ride::keep_back_on`, as
    `quad::Ride::keep_back`).
- `Load::state()` gives the pieces' fresh running words; `Load::ride(at, quat, drag)` builds the Ride
  from MD150's `Load`.
- Every call `frame` and `class_update` hand out, and who answers it on an MXU world:
  - tire ground (`TireGround`): `quad::world_ground` → `Tire::query` and the surface;
  - ground under the body (`GroundUnder`): `World::ground`'s normal → `Car::ground_normal`;
  - smooth noise (`Noise`): `SmoothNoise::build(Rng::default())`, then `at_accumulated`;
  - contact sort search (`SortSearch`): nothing struck (`SortAnswer::default()`);
  - node position, map-edge test and throw (`OutOfBounds`), body reset, spatial update, node bounds,
    collision sync and subtree marking: `Own` (MD106b/MD128's ports);
  - wreck and unwreck: `Own`'s `wreck_timer`;
  - respawn (`Respawn`, from the unwreck and from leaving the world) and the hazard clock's put-back
    (`PutBack`): the Ride's `PutBack` layer. It stands the body still and enabled at the kept spot. The
    respawn also clears the tires' contact words and the engine's revs and gear (`respawn_reset`);
  - ghost, hazard clock and base update: `OwnUpdate` (MD106e).
  - Dropped on purpose, so they go to `Ride::rest`: `GravityRecord` (no collision record, MD149b),
    `SpatialMove`, `PartMove`, `Gather`, `LeaveWorld`, `RidingLetGo`, `ReleaseRecord`, the sound cues
    (`Cue`, `LandingSound`, `ShiftSound`, `PreloadCue`), the rumbles, `WreckLook`, `SortPlace`,
    `CollisionOn`, `Unlist`, `Place`, `SubtreeOutOfDate`, `NodeRebound`, `ClassStep` (posing the model's
    parts, MD151), and the base update's `Prepare`/`Ride`/`Sync`/`Snapshot`/`Network`/`Parts`.
- Test: crates/game-sim/tests/mxvatv_car_mxu.rs `each_car_rides_free09_jumps_wrecks_and_is_put_back`.
- Filed MD149a (MD135's replays through `Ride`, after MD135) and MD149b (the car's collision record on
  MXU worlds).

## What the original does

- The original has no car on an MX Unleashed track, so there is nothing to match on an MXU world.
- The car's own frame and update are MD106/MD106b/MD106e/MD128's ports, used unchanged.
- The fresh `State` follows MD39a's five spawn images, read with a throwaway dump test. All five show:
  `held` 0, throttle 1, reverse 1, driven 1, mode 1, gear 1, ratio = final drive × first gear,
  `inv_ratio` its inverse, nitrous left 1000 (`NOXVolume`), and torque-curve cursor 1.
- The images' revs and launch words are what the cars had after settling. A fresh car takes the
  respawn's 0s instead.
- At rest, the body's height over its tire ground points is close to the images' (buggy and truck
  shown below), so the car is not sinking into free09.

## How it was proved

- Headless free09 runs, one per car (buggy, baja, sandrail, golf cart, monster truck). Each car is made
  from its `.phy` and `.psg` alone (MD150) and driven with a `Manager`/`Report` pad through
  `Table::SHIPPED`. The test does five phases:
  1. stands 60 ticks (moves < 3 ft);
  2. gas 90 ticks;
  3. gas with the stick hard right 150 ticks (heading turned > 20°, summed tick by tick);
  4. off a 4 ft lip near the spot (≥ 6 ticks with no tire grounded, then down on all four);
  5. ridden 240 ticks to keep a put-back spot, then taken 100 ft from the map's west edge: the edge
     wreck starts at once, and when the clock runs out the car is put back within 10 ft of the kept
     record and rides 10+ ft on the gas with no new wreck.
  Every call handed on must be on the dropped list.
- Results (dt 1/30):

  | car      | top ft/s | turn  | air ticks | put back (tick, ft from record) |
  |----------|---------:|------:|----------:|---------------------------------|
  | buggy    |     66.4 | 219.9° |        58 | 301, 1.0                        |
  | baja     |     85.0 | 470.2° |        54 | 301, 0.5                        |
  | sandrail |     84.1 | 507.6° |        49 | 301, 0.5                        |
  | golf     |     40.0 | 640.9° |        48 | 301, 0.2                        |
  | truck    |     86.8 | 264.9° |        68 | 260, 1.2                        |

- Body over the tire ground points at rest:

  | car   | port (standing on free09)    | spawn image          |
  |-------|------------------------------|----------------------|
  | buggy | −0.28, 0.11, 0.09, 0.52      | 0.23, 0.22, −0.17, −0.24 |
  | truck | mean 2.05                    | mean 2.09            |

## Not verified / not 1:1

- **MD135's five port replays through `Ride`.** Not done: MD135 is still open, so there are no port
  replays to run through the type yet. Split to MD149a (after MD135).
- **The put-back placement.** It is not the original's. The car's respawn (MXVehicle's put-back with its
  ground reset) is not ported; that is MD154. The Ride stands the node at MX Unleashed's put-back
  record, raised by the lowest tire bottom plus 1 ft along the ground's up and turned to the record's
  facing. The body is still, its forces are cleared, and it is enabled.
- **The respawn's tire and engine reset.** The fields are mapped from the original's offsets, but the
  two rodata quads it stores are not read. The port clears the vectors to (0,0,0,0) and stands the
  normals on (0,1,0,0); the `w` lanes are a guess.
- **No collision record on MXU worlds** (MD149b). `Vehicle::touching` stays false, so the tipped wreck
  (state 14) never starts. The car does not strike props. `GravityRecord` is dropped.
- **The contact sort's search answers "nothing struck".** MX Unleashed's props are not SLUS-21104
  objects. The tires find the ground by their probes only.
- **The class step (`ClassStep`) is dropped.** It is unported (MD151) and only poses the model's parts,
  so the physics don't depend on it as far as MD106e found.
- **The world's floor is the start spot − 1000 ft.** MXU worlds have no floor word; the value is a
  choice.
- **The drag word starts at 0.** The test passes 0; the world word the original reads for MXVehicle's
  drag is not known on MXU.
- **The fresh `State`.** It is taken from images captured after the cars had settled, not from the
  spawn function. `launch_*`, `rpm` and `wheel_rpm` start at 0, not at the images' values.
- **The odometer and side/ahead axes** given to the update come from the node's frame rows 0 and 2 as
  the frame left them, not from a proven read.
- **The hazard table is empty.** The clock never sinks or holds the car; MXU surfaces have no MX vs. ATV
  hazard entries.
- **No capture comparison.** The original has no car on MXU, so the runs are judged by riding
  thresholds only. That includes a lowest centre-of-mass reading under the terrain sampled at the body
  (−0.43 ft for the buggy) during the drive.
- **The smooth noise's generator.** It is built from `Rng::default()`, not the original's seeded state
  at load.

## Next

Done when MD149 lands. MD149a runs MD135's replays through `Ride`; MD149b adds the car's collision
record.
