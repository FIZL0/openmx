# 1 — the cars drawn (FINAL)

Task S245. 2026-11-02.

## What I did

- **`crates/game-render/src/car.rs` (new):** the two rigs that pose a car's nodes from its tires,
  plus the driver's seat. The module docs describe both rigs.
  - The truck: hubs lifted and steered, tires spun and stretched, axles rolled, drive lines and
    swingarms aimed.
  - The baja and the buggy share a rig: upper arms swung by 1.25 × compression ÷ the arm's length,
    hubs steered and kept upright, lower arms and the rear swing arm aimed, strut pairs aimed at each
    other.
- **`crates/game-render/src/psg.rs`: node lists.** Before this, each car drew as a pile of tires,
  with struts sticking out and loose debris. The cause was that the parser read a vertex's
  `(w>>4)-1` as a node index. Strictly it is a slot in the mesh's node list. Those lists sit in a
  trailing section after the last stream (layout in the module docs and in `research/oracle.txt`
  S245). I surveyed every `.psg` on the disc:

  | Files | Section |
  |---|---|
  | 4013 | none |
  | 2119 | flag 0x40, one identity list |
  | 44 | flag 0x80, a list per mesh |

  The 44 are the three cars and their liveries in every archive, plus free10's `shovel.psg`. So the
  fix changes only those models. `monstertruck2.psg` writes its map twice; the first copy is read.
- **`crates/openmx/src/car_draw.rs` (new):** the shell's side of the car.
  - The tires' state off a driven `game_sim::car::Machine` (the truck's rear steer scaled by
    `RearTireSteerScale`).
  - The GPU actor and the software skinned copy (`car@seat{p}`).
  - Two `--trace-picture` lines: `picture car:` and `picture bay:`.
- **`crates/openmx/src/main.rs`:**
  - loads the cars' models;
  - draws a driven car with its driver (`pilot:` lines too) on the GPU and on the software path;
  - draws the car bays see-through on the GPU;
  - stands the car bays on the ground once the cars are built, and again after a re-park.
- **`crates/openmx/src/vehicle.rs`:**
  - `Machine::kind` now covers the cars, so their bays draw and are offered.
  - `Machine::home` / `parks_by` / `parked_by`: each car parks only by its own track's `machine.txt`.
  - `CarRide.livery`.
  - `CarRide::take` stands the car in its bay when it has one.
  - `Lot::stand_cars`: puts each car bay on the ground under it, on its tires with the springs
    unloaded. free07's file parks the baja at y 0, under the dirt; it now stands at 138.5.

## Decisions (taken unattended)

- **Every track still offers every car in the pause menu.** A car with a bay is taken there. One
  without (off its own track) spawns on the rider, as before.
- **The bay copy stays drawn see-through while the car is driven,** as the plane's and the Kiowa's
  do.
- **The driver's seat is the plane's and Kiowa's measured seat carried to the car's limb nodes**
  (`SEAT_FROM_LIMBS`), not `.phy` `PelvisBoxPosition`, for the reason P216b gives. The driver is
  posed by the plane pilot's clips with the default lean.
- **The aims are relative.** Each node is turned by the turn that takes its target from where it
  was in the bind pose to where it is now. So a car at rest is exactly the model; the test checks
  this.
- **Inside a world, no car is parked.** The lot parks by the home tile's file, and the mxu worlds'
  home is free06. A car is still switched to from the pause, and is drawn while driven (seen in
  world_mxu_free by eye).
- **Only the GPU draws bays,** as for every machine. The parked check is GPU-only.

## How it was checked

- `game-render/tests/car.rs`, on all six models:
  - every vertex lies inside the bounds of the node its list names;
  - at rest the pose is the model exactly;
  - each tire rises by its own compression while the one across stays;
  - the seat sits between the feet and the hands, behind the wheel.
- `openmx/tests/car_picture.rs`, on each car's track:
  - "X parked here", and one `picture bay:` of that car, on the ground;
  - a shot from behind the bay;
  - driven on the GPU and on the software path: taken at its bay, at rest when taken, the springs
    take the weight, the gas spins the tires and moves it, and the driver's root is at
    `car::seat` on every frame;
  - a 640×448 shot of each run;
  - free06 parks no car.
- By eye (shots in `target/tmp/s245-car-picture/`):
  - the three bays on the ground, see-through;
  - the driven truck, baja and buggy from behind, and from the side in earlier scratch shots, with
    the driver in the cab, on both renderers;
  - the truck driven in world_mxu_free.
- `./kit check -- -p game-render`: 291 passed.
- `./kit check -- -p openmx --no-fail-fast`: everything passes except
  `biplane_particles a_plane_flown_into_the_ground_puffs_where_it_hits`. That is F9, red on main
  before this task (touched 11, first puff 11), in code this task does not touch.

## Not verified / not 1:1

- The rigs' aims are the port's relative form. The original builds each frame from an up vector and
  the target. The two agree by eye, but they were not compared against a RAM capture of the
  original's node matrices.
- The driver leans by default; the steering does not feed the lean. The steering wheel does not
  turn.
- A taken car spawns at its rest height plus `CAR_DROP` and settles about 4 ft (the truck's travel).
  The parked bay is drawn with the springs unloaded, so the first driven frame is higher than the
  bay was.
- No car parks inside a world. Off its own track a car has no bay.
- The software path draws no bays, as for every machine.
- `shovel.psg` on free10 now reads its node lists too. It was not looked at by eye.
- Only one player was run. Copies past the file's spots (two per car) are parked by the port's
  nearby rule and stood on the ground the same way.
