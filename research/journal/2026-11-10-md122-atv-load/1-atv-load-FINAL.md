# 1 — atv load (FINAL)

Task MD122. 2026-11-10.

## What I did

- `game_sim::quad::Load` (crates/game-sim/src/quad.rs, at the end): the quad's loader from an `atv_*.phy`, with
  no RAM image. `Load::read` / `read_bytes` read, in the original's order:
  - the base loader's `[Mass Properties]` through `handling::mass::Part`, then its `[Wrecks]`;
  - the moto loader's `mass::Bike`, the tire reader (`quad::tire_phy`, given the part's mass), and MX Unleashed's
    bike keys through `phy::Phy::read` (the same names, defaults and units);
  - then `[Three Sixty]` (`air::SpinTuning`), the camera keys (`car::MxvatvCamera`, with the quad's
    first-person offset and its `FC_ATVDefault.pb`), and the pelvis box.
- What comes from the model rather than the file is handed in: the half box and each wheel's mount and sweep.
- `WheelTire::constructed` holds the wheel constructor's words.
- The loaded values go out in three ways:
  - `Load::words(&LoadAt)` gives every word the loaders write, as `(address, mask, bits)` in write order: the
    part's, the wrecks', the bike's, the four wheels', the engine's and its tables, the quad's, the camera's
    and the pelvis box's. It also includes the init's +0x2c/+0x30 wheel words.
  - `params()`, `clutch_keys()`, `air_keys()`, `tune(&mut Bike)`, `tune_quad(&mut Quad)` and
    `tune_body(&mut atv_body::Body)` put the loaded tuning over the port's ride types.
  - `clutch_keys` and `air_keys` carry only the fields the quad's steps read; the rest are 0, as the image
    reader leaves them. The control authority time is the quad's own word.
- Test: crates/game-sim/tests/atv_quad/load.rs, mounted from tests/atv_quad.rs.

## What the original does

- `./kit dis -p atv 0x004a7e70 …` is the base loader: the part reader 0x00493210 (see # MD39b and the rows
  near 10612 in research/oracle.txt), then `[Wrecks]`.
- `./kit dis -p atv 0x004def40 …` is the moto loader that the quad and the bike share. It reads:
  - the Bike mass keys and the tire reader;
  - MX Unleashed's steering, roll, pitch, air, wreck, balance, donut and joystick keys, at the quad's offsets;
  - `[Three Sixty]`, the camera through vt +0x370 (0x004a7f50), and `[Animation]`.
- atv_quad_init (0x004b9db0, which has no Ghidra function; use `./kit dis`) does the rest:
  - it builds the four wheels through the constructor 0x004a0030, which sets lock 0.78539807 / −0.78539807,
    the 1.0 words, and the rest 0;
  - it sets the mounts and sweeps off the model's wheel setup;
  - it sets +0x2c to 1 on the fronts and +0x30 to 0 on all four.
- All the offsets are in research/oracle.txt under # MD122.
- The model box call 0x001961f0 builds the box from the world rows of the node's children. Once the quad has
  spawned and been posed, the call no longer gives the load's box. Resetting the root's rows to the identity
  doesn't help either, because the children keep their posed rows.

## How it was proved

- A save-state capture: tests/atv_quad/load.rs `the_c28_quads_words_are_their_phy_files`, on C28's four images
  (c28-atv-r-spawn, c28-atv-r-air, c28-atv-s-spawn, c28-atv-s-air):
  - **1994 words per image are bit-exact** with `Load::words`, for atv_race250p on the r images and
    atv_s_race250p on the s images.
  - **Only the right file matches.** Each image was tried against all eight `atv_*` / `atv_s_*` files, and
    every other file leaves differing words.
- **The ride types match.** On each image, `image::read_ride` overwritten with `params()`, `tune_quad` and
  `tune` gives the same ride as the image. The only words put back first are the ones the ride moves (listed
  below). `seed::body` with `tune_body` on top leaves the body unchanged.
- **The constructor's words:** `the_wheels_the_reader_leaves_are_the_constructors` checks the rears' locks and
  the fronts' +0xb8 against `WheelTire::constructed`.
- Run: `./kit check -- -p game-sim --test atv_quad load::` passed 2 tests, with all four images present (none
  skipped; checked with `--nocapture`).

## Not verified / not 1:1

- **The half box comes from the image, not from the model.**
  - The record's inertia depends on the model's bounding box, which the part reader gets from the scene
    graph's box call. That call is not ported.
  - The box can't be read off the spawned images either, because they are already posed.
  - So the test works the box back from the record's inertia, as tests/mxvatv_bike_ride.rs's `load_box` does
    for the MX bike. The result is (2.1281617, 1.9494964, 3.016569), the same on all four images.
  - This proves the inertia arithmetic, not the box. A load with no image still has to be handed the box.
- **The wheel mounts (+0x90) and sweeps (+0x1dc) are read off the image.** The model's wheel setup is not
  ported, so `Load::read` takes them as inputs.
- **Some words are skipped because the ride has moved them by the time of the images:**
  - wheel +0x48 (the spring, swapped between stopped and moving), +0x60 (the steering torque, cleared every
    tick) and +0x100..+0x110 (the rest point);
  - engine +0x5c/+0x74/+0x78 (the gear), +0x84/+0x88 (the curve's cursor) and +0xa8 (the peak torque's revs,
    which the spawn's engine reset zeroes).
  - The values the port writes there are the reader's as disassembled, but no image shows them at load time.
- **The ride comparison puts back the image's tail of the ratio and curve tables.** These are the lanes past
  `gears` and `points`, i.e. heap memory past the tables that the image reader also reads.
- **Keys with no default in the original read as 0 in the port.** The original leaves whatever was in the
  word, so a file missing such a key can differ. Both retail quad files have them.
- **Not ported:** the init's other work (+0xc0 = 2, the probes, the trigger segments, the collidables, the
  scene nodes, the pose and the contacts) and the `[Animation]` keys. None of these are compared.
  - A full `Quad` / `Bike` from a file with no image needs that spawn half. MD123 owns the quad's step on MX
    Unleashed's worlds.
- **Only race250p was checked.** No image has the other six files' quads (the 450s and the sport/utility
  variants). Those files are proved only by being told apart.

## Next

- MD124 (filed): the half box and the mounts and sweeps from the quad's model, so `Load::read` needs no image.
- MD123: spawn the quad from `Load` on an MX Unleashed world. Port the init's spawn half (pose, probes,
  collidables) and the model box, or the box and mounts from the quad's model, so that no image is needed.
