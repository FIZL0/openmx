# 1 — path car root turn (FINAL)

Task V46. 2026-10-21.

## What I did

- **The question.** V41 found that a scene prop is drawn with its placement in place of the file's
  root node. V46 asks whether the same holds for the `.toy` path cars, the bike and the rider.
- **The answer: yes.**
  - The original's path car uses the same model object class as a prop.
  - That class's node reader treats the file's root node as the object itself.
  - So the root's matrix is read into the slot that the car's pose then overwrites, every step.
- **The fix.** The shell (`crates/openmx/src/main.rs`) now loads the cars with `Assets::load_placed`
  instead of `Assets::load`. The doc comment on `load_placed` (`world.rs`) now names the cars too.
- **Which cars this changes.** I listed the root node of every `.toy` car on the disc (all 5 freeride
  tracks and their `_ss`). Only free06's `train_car_03`, `_04` and `_05` have a root that is not
  identity, and it is a half turn about up.
  - The trucks, scrapers and trailers have identity roots.
  - So do the `HasPhysics` cars, which the port does not draw.
  - The port used to draw those three train cars turned 180° on their own pose.
- **Bike and rider.** Every `.psg` root under `Bike\`, `Rider\`, `UI\Bike`, `UI\Rider` and `machine.res`
  is identity (402 + 324 + 408 + 80 + 26 roots checked). Whether the root is dropped or kept, they
  draw the same, so nothing changes for them.
- **Took from opentennis:** nothing. It has no scene-object loader of this kind.

## What the original does

- **One class for props and cars.**
  - The prop reader (`./kit fn 0x001a8cd0`) creates its model object with the 0x1c0-byte class
    `0x00176430` (vtable `0x0044f870`).
  - The car init (`./kit fn 0x001147c0`) creates the same class and loads the model through the same
    vtable slot +0xa0 (`0x00176a90`, which calls model load `0x001a0360`).
- **The node reader** (`./kit fn 0x001b3890`):
  - It sets `param_3[0] = object`, so node 0 is the object itself.
  - It reads each node's 64-byte file matrix into `node+0x40`. For the root, that is the object's own
    local matrix.
- **The placement** then overwrites that same matrix:
  - `0x001b2640` writes the position to +0x70.
  - `0x001b1d70` rebuilds +0x40..+0x70 from look and up (through `0x0013e330`).
  - The car's update (`0x00115540`) calls both on every scene step.
  - The file's root turn is therefore gone after the first placement.
- **The box.** The object keeps the file root's box at +0xc0 / +0xd0. It puts that box in the world
  with the object matrix alone.

## How it was proved

- **Save-state capture:** the biplane recording's state on free06 (`context/v46/biplane.ram`, copied
  from s4's `context/v41`). In it, I found the model objects of train_car_03, _04 and _05 by their
  pose and by the `.psg` name beside them in the car:
  - The object matrix (+0x40) is exactly the car's pose. Its z row is the car's heading
    (−0.0016, 0, −1), with no half turn on top.
  - Its model box is the file root's box. That box equals the bounds of the file's vertices *untouched*
    (to 0.01 ft), so the box is in the raw vertex frame.
  - The world box centre the original computed (for example, train_car_03 at
    2313.517, 144.045, 2810.838) equals the pose applied to that box. With the half turn it would sit
    0.14 ft further along the car (0.13 ft sideways on _04).
- **Test:** `crates/game-render/tests/toy.rs`
  `the_trains_turned_cars_are_drawn_without_their_files_root_turn`, against the real disc and the state's
  numbers. For each of the three cars it checks:
  - the file root is still a half turn;
  - `load_placed` makes it identity;
  - the drawn box matches the original's box;
  - the box under the RAM matrix lands on the RAM world centre to 0.01 ft, and lands ≥ 0.1 ft off with
    the root kept;
  - the port's car pose equals the RAM object matrix, row by row, to 0.01.

  To prove the test is not skipped, I flipped its last assert to `> 100.0` once. It failed with
  "the root's turn would move it only 0.14111413 ft", then I put it back.

## Not verified / not 1:1

- **No screenshot.** I did not compare the port's train with the original's in a picture. The proof is
  the RAM matrices and box. A train car is nearly symmetric end to end, so the turn would hardly show
  anyway.
- **Bike and rider animation roots.** The bike's `.mot` clips animate the root node (chassis.rs). The
  original's root node is the model object, whose matrix the placement owns. I did not trace how the
  clip's root channel and the placement combine in the original. This task only shows that the file's
  root (identity everywhere) makes no difference. V20's chassis work covers the drawn pose.
- **The `HasPhysics` cars** (ctruck, ranger/water trucks, police car, free10_truck) are not drawn by the
  port (S81). watertruck and free11_policecar have a near-half-turn root. The rule above will apply
  when they are drawn, because they are the same class. That is assumed from the shared loader, not
  read from a state.
- **Vultures (nat02/nat19).** Their root has a half turn too. They are V44's creatures and are not
  drawn yet; whoever draws them should go through `load_placed` or check it.
- **Other states.** Only the biplane state on free06 was read. The other path cars have identity roots,
  so they need no state.

## Next

Nothing for V46.
