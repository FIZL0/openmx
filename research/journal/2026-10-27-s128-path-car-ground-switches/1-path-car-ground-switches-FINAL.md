# 1 — path car ground switches (FINAL)

Task S128. 2026-10-27.

## What I did

- `crates/game-sim/src/traffic.rs`:
  - `Spec` gained `presample` and `level` (the `.toy`'s `PresampleTerrain` and `PathIsLevel`), plus
    `measured`: the tangents that load's measure leaves cached.
  - `Key` gained a `normal`.
  - `Car` gained `stale` (it still rides those cached tangents) and a `Level` (the path's level
    height, the one before, and the hold).
  - `Spec::presample` puts every key on the ground once, on the first step, after the clock and
    before the curve. The keys keep the lengths measured on the clip's own keys.
  - A presampling car rides the curve's own height and the two key normals blended lane by lane at
    the segment's `t`.
  - A level car rides the ground's height until two frames give the same one, and then holds it
    with a straight-up normal. Its basis takes the raw up, with no filter and no orthogonalising.
  - `Spec::curve_kept` keeps the measure's tangents until the cursor moves, as the original's
    tangent cache does.
  - `Spec::lagged` now walks to its segment as the original's lag does (`lag_find`). Walking back, it
    tests against the time of the key the cursor started on, so it goes to key 0 and extrapolates
    segment 0's arc. The old walk was one ulp off on a lagged trailer, 25 ticks after its clock
    wrapped.
  - The snapshot carries the new `Car` fields.
- `crates/game-render/src/toy.rs`:
  - `Path` keeps the clip's raw positions (`clip`).
  - `Traffic::sim` hands the simulation those raw positions, not the render's own presampled ones,
    with `presample`/`level` set from the `.toy`.
- `crates/game-sim/tests/presample.rs` is new: three tests on recording2's state of free09.
- `crates/game-sim/tests/two_p_truck.rs`:
  - `spec_of` now takes the image's presampled keys with their lengths and cached tangents, and
    `car_of` takes the tangent-dirty flag.
  - `the_paths_measure_as_the_originals` now measures the shell's specs from the disc's `.toy` and
    clips against the image's lengths.
  - It and `the_truck_drives_from_each_image_into_the_next` are no longer ignored. Both pass.
- **This covers S181.** It asked for exactly those two tests, bit-exact on the s159 images. They are
  un-ignored and green. I left S181 unticked, as the task said.
- Filed S185 (DriveOnSoultree) and S186 (free11).

## What the original does

See research/oracle.txt "S128". In short:

- **The switches.** The init (`./kit fn 0x1147c0`) copies the `.toy`'s switches onto the car. Every
  free09 car presamples, and none is level or on the soultree.
- **The first frame.** On its first frame, `./kit fn 0x115540` runs `ground_query` on every key's
  position and normal, in place.
- **Key lengths.** The lengths are what the load's measure (`./kit fn 0x115330`) made of the clip's
  keys. The measure leaves the cursor on segment n−2, with that segment's tangents cached from the
  raw keys.
- **The curve.** The curve (`./kit fn 0x116410`) reads its keys through two pointers that only move
  with the cursor. It recomputes its tangents only after a cursor move.
- **The lag.** The lag virtual (`./kit dis 0x116150 0x116410`, not a Ghidra function) walks back
  comparing `s` against `keys[$7].t`. `$7` is the cursor it was entered with, not the key it has
  reached.
- **Height and up.** A presampled car takes `y` from the curve, and its up from the FPU lerp of the
  segment's two key normals (ends clamped).
- **A level car.** It queries the ground until the height repeats, then holds the path's height with
  up (0,1,0,1). Its look-at uses the raw up.

## How it was proved

All of these are bit-exact interpreter comparisons on RAM images.

- `crates/game-sim/tests/presample.rs`, on `recording2.p2m2_SaveState.eeMemory.bin` (free09, six
  trailer cars):
  1. `the_trailers_keys_are_presampled_as_the_originals`: the shell's specs, built from the disc's
     `.toy` and clips, have the state's 1506 key lengths. Presampled on the port's ground, every
     key's position and normal is the state's.
  2. `the_trailers_drive_from_their_load_as_the_originals_do`:
     - Every car is put back as its load leaves it: raw keys, the original's own measure run in the
       interpreter (lengths and cursor compared), clock 0, first-frame flag 0, and the ctor's
       vectors.
     - Then 600 ticks of the original's update against `Spec::step`. On every car and tick these
       match: clock, cursor, point, heading, up, position and world matrix. The keys after tick 0
       match too.
     - One cab's LagDistance is set to half its segment n−2, in both games, so that it rides the
       measure's stale tangents. The test asserts that this happened.
  3. `a_level_car_drives_as_the_originals_do`:
     - The same six cars are switched to PathIsLevel from the state, and car 0 is given TimeScale 0
       so that it holds.
     - 600 ticks, with level height, last height, hold, heading, up and world matrix compared, and
       both the held and the riding branch exercised.
- `crates/game-sim/tests/two_p_truck.rs`, on the s159 images (free09_ss): 1506 lengths and 6 tick
  pairs, all fields equal. I confirmed both tests ran, not skipped, by raising their count asserts.
- `crates/game-sim/tests/traffic.rs` (S81, free06) and `ragdoll_cars.rs` stay green with the new lag
  walk.

## Not verified / not 1:1

- **DriveOnSoultree is not ported.**
  - Why: it needs the general collidable-vs-world query (`probe_collide_world`, 2124 bytes) run on
    the car's soultree segment, and every car on the disc sets it FALSE.
  - The original's `.toy` default is TRUE. A mod `.toy` that omits the key would drive on the
    soultree in the original, but not in the port.
  - The render side's `drive_on_soultree` also defaults to FALSE.
  - Filed as S185.
- **Not proven on free11.** There is no free11 RAM image in context/states. Only free09 (recording2)
  and free09_ss (s159) are proven, besides S81's free06. Filed as S186.
- **The PathIsLevel proof is synthetic.** No car on the disc sets it, so the test flips the switch in
  RAM on free09's cars, mid-ride, with presample off. It proves the branch against the original's
  code, not against a real level car's load.
- **A level car on a path that does not loop** (`+0x420 == 0`) is not ported. Whether it queries the
  ground depends on the clip-loops flag, and every car the port drives loops. In that case the
  original rides an up it never set.
- **The presampled up's `w`** is uninitialised stack in the original. The port keeps the car's own
  `w`. No test compares the matrix row it would feed; the world matrix matched on every tick, so
  it does not reach it there.
- **The `first` flag with presample.** A car whose `+0x4e4` is set, together with presample, takes
  that up unfiltered, garbage `w` included. No disc car sets `+0x4e4`, so this case was not
  exercised.
- **The render side's own path cars** (`game_render::toy::Path`) still presample with the render's
  ground and are not bit-exact. Only the simulation's are proven.
- **Rollback.** The presampled keys live in the `Spec` (a `OnceLock` shared by every snapshot), so a
  rollback to before the first tick does not redo the presample. The result is the same, since it is
  done once in either case, but no rollback test covers it.

## Next

Nothing for S128. S185 and S186 are the follow-ups.
