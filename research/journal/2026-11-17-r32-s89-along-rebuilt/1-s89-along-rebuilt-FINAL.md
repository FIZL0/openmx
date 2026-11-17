# 1 — s89 along rebuilt (FINAL)

Task R32. 2026-11-17.

## What I did

- **Looked for a surviving copy first.** No `s89-along` in the shared context, old-context-s1..s3,
  kept-context, or any copy's save-state slots. The s89 captures (`context/capture/s89/`) are gone too.
- **Pinned the old state's moment.** The tests ride the train, so where the train stands matters.
  - The 21 path cars sit at the same heap addresses in biplane.p2m2's image as the tests' `CARS`, so a
    state made from it by the same route keeps them. (free06-switched and the c34 images are another boot:
    no cars there.)
  - S146's roof state (frame 11932, the engine's path clock 41.487) was made from S129's copy of the
    along state by 16 + 30 frames. So the old along state stood at clock ≈ 41.487 − 46/30 ≈ 39.95, the
    boxcar on the z≈2318 straight running +x. S207's ctruck state (frame 11992, from s89-along) agrees.
- **Rebuilt it in copy 3** (`research/r32/`, no uinput: `/dev/uinput` is root's):
  - `switch.pad`: a from-savestate tape (C15's `tape.py`) on biplane.p2m2's state: Start, d-pad down ×3,
    Cross ("Switch To Machine: Bike"), then both brakes for 11 s, then nothing.
  - `along.py`: waits for the tape's Square in the pad buffer, pauses over AT-SPI, does S76's teleport
    (every copy of the position shifted by one delta, velocities zeroed) to S89's spot (4591.5, 135.46,
    2899.7), resumes, pauses when the engine's clock reaches 39.9, saves the slot.
  - `make.sh` runs the two under `./kit pcsx2 with --`; `./kit state ram 9` gave the image.
- **The result**: frame 11911, clock 39.912 (target ≈ 39.95), the vehicle at (4591.06, 135.46,
  2898.79), settled, facing +z 5.7° off (S89: "facing +z (5° off)"). The vehicle is at 0x00a00050, as
  replay.rs's S252a note has for every s89-along state; the rider's bone sphere (0x0085c9d0, radius
  0.75) and the 21 cars are where the tests read them.
- Installed at `context/states/s89-along.eeMemory.bin` (shared) and kept, with its `.p2s`, in
  `~/repos/openmx-slots/kept-context/r32/`. `watch/pads/s89-along.pad` names the recipe.

## What the original does

Nothing new: this task only remakes a state.

## How it was proved

- `./kit check -- -p game-sim --test ragdoll_props --test ragdoll_cars --test traffic --test
  physics_cars --test bike_col` on a private context (KIT_ROOT) holding the new image: 38 passed.
- **They ran, not skipped**: the same run with every "image not there" and "no MX.PSR" skip in those
  files (and the ride kit's skip) turned into a panic also passed 38; with the shared context, which did
  not yet hold the image, ragdoll_props failed on the panic. The probes were reverted.
- After installing: `--test eco --test physics_car_struck` 14 passed.
- The full gate in `./kit done`.

## Not verified / not 1:1

- **Not the same image as S89's.** It is a new save of the same route: the frame (11911) and clock
  (39.912) are close to, not equal to, the old ones (estimated ≈ 11886 / ≈ 39.95 from S146's state; the old
  values themselves are lost). The bike stands 0.4 ft off in x and 0.9 ft in z from S89's spot. Every
  test that reads it passes, but none of their numbers can be compared with what they printed on the old
  image.
- **The switch tape's brake hold is from the tape, not `./kit pad`** (S89 used the uinput pad), and it
  was released ~3 s of game time after the teleport rather than 3 s before the save; the bike was still
  at speed 0.00–0.02 ft/s when saved.
- **The menu walk was not checked by screenshot.** It is taken as landed because the human's vehicle
  afterwards is the bike at 0x00a00050 and settles on the bed at S89's height (135.46).
- **s89-across and the s89 captures (along.csv, across.csv) were not remade**: replay.rs's S89 ride
  replays still skip. Filed as C39.

## Next

Nothing.
