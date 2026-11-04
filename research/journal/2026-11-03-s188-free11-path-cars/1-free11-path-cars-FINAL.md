# 1 — free11 path cars (FINAL)

Task S188. 2026-11-03.

## What I did

- **The image.** s2's `context/states` was gone. S196's two save states were still in kit copy 2's
  slots, at `openmx-slots/s2-slots/pcsx2/s2/PCSX2/savestates`: slot 21 is sx01 and slot 22 is free11.
  The image's strings name `Tracks\free11\`. I unpacked them again with `./kit state ram <p2s> --out`
  as `context/states/s196-free11.ee.bin` and `s196-sx01.ee.bin`. I rebuilt recording2's image from the
  main checkout's `context/recordings/recording2.p2m2_SaveState.p2s`, so the free09 cases run here too.
- **`crates/game-sim/tests/presample.rs`** now takes a `Case`: the state, the track and its car count.
  - S128's three tests run as before on `FREE09`.
  - Three new tests run the same bodies on `FREE11`: `free11s_keys_are_presampled_as_the_originals`,
    `free11s_cars_drive_from_their_load_as_the_originals_do` and `free11s_cars_drive_level_as_the_originals_do`.
  - `on_free09` became `on_track(track, …)`.
  - The car scan finds the `HasPhysics` subclass (`physics_car_vtable`) too. Each car is stepped through
    its own vtable update (+0x38), as the toy world's list calls it.
  - The switch check compares the state's `PresampleTerrain` with the shell's spec, so a car that does
    not presample no longer fails it. On free09 every car must still presample.
  - The drive test asserts that no `HasPhysics` car turns physical.

## What the original does

- **The cars.** free11's toy world has twelve cars. Eight are plain path cars and four are the S127
  subclass. Two cars presample and none is level or on the soultree.
- **Key normals.** A car that does not presample leaves its keys' normals at (0, 0, 0, 1), which is
  where the clip load leaves them. It never reads them, because it asks the ground every frame. The
  port's raw keys carry (0, 1, 0, 1), and that difference never reaches any output.
- **The subclass.** Until it is struck, the subclass's update drives the path exactly as the path car's
  does (S127's finding on free06, now seen on free11 too).

## How it was proved

All of this is bit-exact interpreter comparison on RAM images. `./kit check -- -p game-sim --test
presample` passed 6 tests. A pass hides skips, so a throwaway probe (deleted before the commit) wrote
what each test exercised to the scratchpad:

| case | keys compared | presampling cars | physics cars | stale-tangent car-ticks | level held / riding car-ticks |
|---|---|---|---|---|---|
| free09 | 1506 | 6 | 0 | 12 | 3259 / 341 |
| free11 | 1452 | 2 | 4 | 56 | 4450 / 2750 |

Each case runs 600 ticks. On every car and every tick, the clock, cursor, point, heading, up, position
and world matrix match. In the level run, the level height, last height and hold match as well.

## Not verified / not 1:1

- **Key normals of a car that does not presample.** The original keeps (0, 0, 0, 1) and the port keeps
  (0, 1, 0, 1). Neither side reads them, so the test skips this comparison for those cars.
- **The level run on free11 is synthetic, as on free09.** The test switches `PathIsLevel` on in RAM.
  No disc car sets it.
- **Physics cars are stepped while asleep only.** Waking, settling and the return to the path are
  S186/S127's tests, not this one's.
- **The images exist only in s2.** They live in `context/states` in openmx-slots/s2, which git ignores.
  In other slots the free11 tests pass as skips.

## Next

Nothing.
