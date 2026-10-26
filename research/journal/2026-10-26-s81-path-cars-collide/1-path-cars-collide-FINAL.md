# 1 — path cars collide (FINAL)

Task S81. 2026-10-26.

## What I did

- **The path cars are now simulated** in `crates/game-sim/src/traffic.rs`. These are the `.toy` cars
  on clips: free06's train, dump trucks and scrapers, and the free09/free11 trailers.
  - `Spec` holds a car's path keys, its arc lengths and its four numbers, and is shared.
  - `Car` holds the state: clock, cursor, last point, heading, up, pose and the first-frame flag.
  - `Traffic` holds both and stands each car's collision mesh among the world's props (`Prop::on`,
    `Prop::stand`, through the new `World::moving`).
- **The cars are part of `Sim`** (`Sim::traffic`):
  - They are stepped at the top of every tick, before the rider: `Sim::step`, `step_all`, the bumped
    step, the biplane arm, `fly_lane` and the shell's `--trace` loops.
  - They are hashed (as `cars`, only on a track that has any) and sent in snapshots (tag
    `S81cars`). A rollback or a replay therefore moves them with everything else.
  - The clock starts at zero when the track loads, as the original's does when the clip loads.
- **The shell builds the sim's cars from the `.toy` and the clips** (`game_render::toy::Traffic::sim`).
  - Each car's `CollisionFileName` mesh comes out of the track's archives.
  - Every seat gets a copy.
  - The picture now draws the first seat's cars, blended between the last two ticks' poses, so the
    drawn train is where the bike meets it. It no longer comes from the shell's own clock.
- **`.toy` numbers are now read the way the original reads them**: `strtod`, then a truncating
  double-to-float (`phy::number`). The new disc test found this: `TimeScale=0.95` was 1 ulp above
  the original's 0x3f733333, which would have drifted the train's clock.
- Names `path_car_update`, `path_car_measure`, `path_car_lag`, `path_car_curve` and `path_car_vtable`
  are in `research/oracle.txt` S81, with the car's field offsets as comments.

## What the original does

`./kit fn` on `path_car_update` does five things each scene update:

1. **Clock**: `clock = fmodf(clock + dt·TimeScale, last t)`, and the same again if it is still past
   the last key.
2. **Point**: the Hermite curve (Catmull-Rom tangents, chord at the ends) at the clock. A car with
   `LagDistance` instead uses the time that many feet of arc behind. Arcs are summed from ten
   chords a segment (`path_car_measure`).
3. **Heading**: the flattened step from the last point, blended by `HeadingFilter`.
4. **Ground**: a ground query under the point. The up is blended by `NormalFilter` only while the
   cosine is above 0x3f34fdf4 (≈0.707).
5. **Pose**: the look-at basis. The mesh collidable at car `+0x428` then takes it:
   - its world matrix (`+0x120`) is the mesh's own transform times the node's matrix;
   - its sphere's centre (`+0x40`) is the root box centre through the node's matrix alone, with the
     translation row first (`vaddax`/`vmadda`/`vmadd`);
   - its radius (`+0x60`) is the FPU `mula/madda/madd` and `sqrt.s` of the half extent;
   - the last tick's matrix is kept at `+0x1e0`.

## How it was proved

- **Function oracle over a save state.** `crates/game-sim/tests/traffic.rs` uses S89's `s89-along`
  RAM image (free06), its 21 path cars, and free06's terrain off the disc.
  - `the_ports_arc_lengths_are_the_originals`: every key's arc length (over 10 000 keys) is bit for
    bit the state's.
  - `the_cars_drive_as_the_originals_do`: `path_car_update` runs in the interpreter and the port
    steps beside it, for 300 ticks × 21 cars. On every tick, every field matches bit for bit: clock,
    cursor, point, last point, heading, up, position, node world matrix, and the collision mesh's
    world matrix, sphere centre and radius.
- **Disc against the state.** `crates/game-render/tests/toy.rs`
  `the_simulations_cars_are_the_originals_paths`: the shell's specs, built from free06's `.toy` and
  clips, contain every one of the state's 21 cars, with keys (t, arc, point) and numbers bit for bit,
  and each car has a collision mesh.
- **By eye in the shell (headless trace).** Command:
  `openmx --track free06 --headless --trace 1700 --trace-feel --trace-brake --trace-at 3600,2318.7,0`
  - The bike is braked on the z≈2318 rail. The train arrives at tick ~1481–1483 and shoves the bike
    along +x (speed jumps to 7.8). At tick 1484 it wrecks, and the bike is put back at ~1565.
  - The bike no longer rides through the train.

## Not verified / not 1:1

- **What hitting the train does to the bike and the rider has not been recorded in the original**, so
  there is no bit-exact replay of it.
  - The user asked for a wreck, a push, or being carried on the roof, recorded and replayed.
  - The port's outcome is whatever the port's existing contact against a moving mesh does with it.
  - Neither S89's captures nor any other capture rides into the train.
  - This needs a PCSX2 recording of the bike meeting the train, then a replay test. That is filed
    as a follow-up.
- **The contact treats the car as still within a tick.** The original keeps the collidable's
  previous matrix (`+0x1e0`), presumably for a swept or velocity contact. I did not read which
  contact code uses it. The port's props carry no velocity, so a car moving into a still bike pushes
  it only through penetration.
- **HasPhysics cars are neither drawn nor simulated.** That is unchanged:
  - free06 ctruck×3;
  - free07 ranger_truck×2, watertruck×2;
  - free09 free11_redtruck;
  - free10 free10_truck×4;
  - free11 ctruck×2, free11_policecar×2.
  - They are another class (rigid bodies), filed as a follow-up.
- **`PathIsLevel`, `DriveOnSoultree` (the ray onto props) and the original's first-frame presample
  are not ported.** Every free06 car sets them FALSE. The free09/free11 trailers get V41's render-side
  presample of the keys, which the sim takes over as is, and is not proven against the original.
- **Only free06's cars are proven.** The free09/free11 trailers have no RAM image with cars to
  compare against.
- **The ground query is the port's `World::ground`.** It matches the original over the 300 ticks on
  free06's terrain. A point off the terrain keeps the port's miss behaviour, not checked.
- **Each seat steps its own copy of the cars.**
  - Seats that tick together agree.
  - A paused seat, or one that joins late, has its cars at its own clock. The original has one
    scene clock.
  - A world-mod's tiled cars (MD15) stay picture-only, because tiled worlds have no props list.
- **Draw blend.** The picture slides each matrix row linearly between two ticks. This is by eye,
  not the original's.
- **dt.** The cars step with the sim's tick `dt`. The proof used 1/30, the scene step.
- **A car with a one-key path** has no `Spec` and the shell stops drawing the cars from the sim. None
  are on the disc's tracks.

## Next

Record the bike meeting the Dallas train in PCSX2 (S126; S127 the HasPhysics cars, S128 the level/soultree/presample branches) and replay it bit-exact.
