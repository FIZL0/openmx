# 1 — plane thrown off the train (FINAL)

Task S204. 2026-11-02.

## What I did

- Listed everything of free06 near the crash point (4489, 141.7, 2585). This was a throwaway test that
  is not committed. It covered the scene's models and `.col`s, the `.toy`'s path cars (by their clips'
  keys) and its animated toys. The nearest scene prop is a telephone pole 143 ft away. The trucks and
  scrapers are 300 ft or more away. Every train car's path passes within 10 ft.
- Added the train to the replay. `crates/game-sim/tests/plane_crash_peak.rs` now has `the_state`. It
  reads the biplane save state's 23 cars (`context/states/biplane.p2m2_SaveState.eeMemory.bin`) in the
  order of the plane's moving list (free06.toy's `Car1`..`Car23`, which `tests/physics_cars.rs` proves).
  For each car it reads the path, numbers and pose, and the collision mesh comes from free06.res. It also
  reads the plane's spawn (`+0xe10`, as `tests/replay.rs`'s `off_the_save_state` does).
- Rewrote `replay_among(…, Cars)` to put the cars in the plane's world (`Traffic::attach`). It drives
  them from the state's tick to the seed on C7's own `dt`. C7's rows begin at counter 10971, the tick
  after the state's 10970, and the test asserts C7 has a row for every tick it drives. It then steps them once ahead of each plane frame,
  which is what `Sim::step` does for a plane ride. The plane gets the state's spawn. Each tick is now
  also checked against C7's body, bit for bit (position, velocity, quaternion, rates), so P164's torn
  rows are covered too.
- Tests:
  - `the_second_crash_is_the_originals_bit_for_bit_off_the_train_and_back` replaced
    `the_second_crash_wrecks_on_the_originals_tick_bit_for_bit`. It runs frames 12493..12688 (C7's and
    P164's last row) with every car. On every tick the body is C7's. The wreck word is CRASHED from
    12498 to 12648 and 0 from 12649, the recovery. On every P164 row that is not torn, the wreck word,
    the touching flag and the shock peak are bit-exact. The only torn row is 12499. The wreck is heard
    on 12498. On 12500 the peak is the car side's (y = 0).
  - `the_train_throws_the_second_crash` shows which cars do it. Each run's first tick off C7's body:
    with no cars, 12499 (S163's finding); without the seventh car's mesh (the second `train_car_03`),
    12499; without the sixth car's (`train_car_06`), 12500; with every car, none.
- The nose-in test is unchanged. It still runs with no cars.

Nothing taken from opentennis: it has no trains or planes.

## What the original does

The train runs over free06's hilltop on its clip. At 12498 the plane's belly skid wrecks it (peak 9.23,
on the ground). On 12499 it meets the seventh car, a `train_car_03` moving at (45, 0, 31.5) ft/s, whose
bounding sphere it is already 4 ft inside. The port's peak on that tick is (−25.2, 20.9, 36.8),
|49.26|. P164's row is torn there, so the original's peak is not read. C7's body is the port's, bit for
bit: (135, −1, 15) ft/s becomes (100, 26, 60). On 12500 it strikes the side of the car ahead, a
`train_car_06` whose side axis is (0.70, 0, −0.71): the peak is (−8.65, 0, 8.75), P164's row exactly.
It then rides on, wrecked. 150 ticks after the wreck the crash timer recovers it onto its spawn, and it
stands there to the end of the capture. All of this is the existing `Flight` and `Traffic` code: the
path cars' meshes as moving collidables (S81, S136, S145) and the recovery onto the spawn (S164, S193).

## How it was proved

`cargo test --release -p game-sim --test plane_crash_peak`: 3 passed. I ran them with `--nocapture` to
make sure none skipped: 239 tick lines were printed and no skip messages.

## Not verified / not 1:1

- P164's 12499 row is torn, so the original's shock peak and touching flag on the train strike are not
  read. The body on that tick is checked against C7, and the next tick's peak against P164.
- The 23 cars are stepped by the port alone (the path cars' step is proved in `tests/traffic.rs` and
  `tests/physics_cars.rs`). Their poses are not compared with the original's on the crash ticks: no
  capture reads them there. The two HasPhysics trucks are driven as path cars on their clips. The
  original runs them through their own physics update. They are over 1000 ft away and never in the
  plane's search here.
- The test needs the biplane RAM image, P164's capture (s1 and s3 only) and C7's capture. Without them
  it prints a skip and passes.
- C7's biplane acceptance run in `tests/replay.rs` (`BIPLANE_AGREES_THROUGH = 1526`) parts on tick 1527,
  which is P164's 12499: the same missing train. Its fixture has no cars. Filed as a follow-up rather
  than changing `replay.rs` here.

## Next

The follow-up: the state's cars in the biplane acceptance run, to raise its ratchet past 1526.
