# 1 — mxvatv car step (PART)

Task MD39b. 2026-11-05.

## What I did

- Ported the two MXVehicle loaders around the car's own loader into `crates/game-sim/src/car.rs`:
  - `MxvatvMachine`: the five cars by kind word (4 buggy, 5 monster truck, 6 baja, 7 golf cart, 8 sand
    rail). Each gives its archive file name, the `Kind` that `Car::read` reads it as, and its default
    camera file.
  - `Car::read_mxvatv` / `read_mxvatv_bytes` → `MxvatvCar { machine, car, wrecks, camera }`. MX
    Unleashed's files still go through `Car::read`; that is the other side of the switch.
  - I took the shape from `phy::Plane::read_mxvatv` (MD80b), which does the same for the biplane.
- Pulled the five car files off the disc again. `bsdtar` gave MXVSATV.PSR, and a scratch crate ran
  `archive::{Psr, Res}` on it. They went to `context/mxvatv-ps2/md39/phy/` (they had gone from this slot).
- MD39a's states, RAM images, tape and csvs had all gone from `context/` too. MD39a's own scratch, my
  PCSX2 copy's SLUS-21104 slots, still held three of its states:
  - slot 6 is the buggy (vsync 27957);
  - slot 7 is the truck (28131);
  - slot 8 is the baja (28313).
  Each vsync is MD39a's row 0 less 2, which is what MD39a's journal gives. I copied them to
  `context/mxvatv-ps2/md39a/{buggy,truck,baja}.p2s` and wrote their `.ee.bin` with `research/md39a/ram.py`.
  The player objects and vtables are MD39a's.
- New test `tests/mxvatv_car_fields.rs`: the field map, word by word.
- `tests/mxvatv_car_phy.rs`: two new tests, one on the reader's defaults and one on the five files' wreck
  and camera keys.
- Updated research/oracle.txt (`# MD39b`) and research/mxvatv-pc.md "The cars (MD39)".
- Filed the rest:
  - C29: remake the golf cart and sand rail states, the tape and the five csvs.
  - MD106: SLUS-21104's car frame, a whole frame bit-exact, as MD75 did for the quad.
  - MD107: the five drives replayed.
  - K44: `plan add MD39b` refuses a subtask of an id that ends in a letter (agent-runner's `check_kind`).
    That is why the three pieces have plain ids.

## What the original does

- `./kit fn 0x004a7e70 -p atv`, MXVehicle's base loader:
  - `[Mass Properties]` through the part reader (`0x00493210`, handling::mass);
  - then `[Wrecks]`: MaxWreckTime, default 5, to +0xc88; WreckGravity, default 3, to +0xc90; WreckDamping,
    default (1,1,1,1) at 0x396fd0, to +0xca0; WreckInertia, default (0,0,0,1) at 0x396fc0, to +0xcb0.
  - MaxWreckTime is read nowhere else: the binary has only one copy of the string. So the car's
    `max_wreck_time` is the same key with the same default, just read first.
- `./kit fn 0x004a7f50 -p atv`, vt +0x370, `[Camera]`:
  - Left1stPerShoulderOffset to +0x980 and Right1stPerShoulderOffset to +0x990, both defaulting to (0,0,0,1).
  - FPCameraOffset to +0xda0. Its default is (0, 5.1, -0.6, 1), or (0, 4.1, -0.6, 1) when the kind word
    +0xbbc is 1 (the quad). Data 0x5265b0 / 0x5265c0, bits 0x40a33333 and 0xbf19999a.
  - FCamFileName to +0xdb0. On a hit the getter `0x00144b50` snprintfs "%s" into 0x20 bytes. On a miss it
    prints the buffer into itself, so it keeps the name `0x004a6140` set from the kind word
    ("FC_BuggyDefault.pb" … "FC_SandrailDefault.pb").
- The car loader `0x004c7fd0` calls the base loader first. Everything after that is MX Unleashed's, as
  MD39 says.

## How it was proved

- **A save-state capture.** `tests/mxvatv_car_fields.rs` `the_spawned_cars_words_are_the_loaders` checks
  every word the loaders write, for each of the buggy, baja and truck RAM images:
  - The words: CofG, CofGAI and travel; yaw and hand-brake keys; every tire key; every surface
    coefficient; ABS; the drive slots; the whole engine with its RPM, torque and gear tables; aero; the
    wings, where a car has any; preload; clutch; joystick and steer tables; RearTireSteerScale and the
    truck's pelvis box; the four wreck keys; the three camera quadwords; the camera name byte by byte.
  - Each is read at its SLUS-21104 address and compared with `Car::read_mxvatv`'s value.
  - Result: buggy 1,273 words, baja 1,271, truck 1,283. All bit-exact; the run printed "3 of 5 cars proved".
  - The first run found one correction to MD39's map: the tire's first block (MaxCompression to
    MaxTireScale) moves −0xc, not −0x10. Everything else held as MD39 wrote it.
  - The tire springs come out equal, so the spring rates made against `phy::Part`'s collision mass are
    SLUS-21104's for these files.
- `tests/mxvatv_car_phy.rs`:
  - `mxvatvs_cars_read_their_wreck_and_camera_keys`: the five files' wreck and camera keys, against the
    reader's own number parse. "0.999" reads a bit under f32's nearest.
  - `the_wreck_and_camera_keys_default_as_slus_21104s`: every default and the 31-byte name, on files
    written in the test. It needs no disc and never skips.
- `./kit check -- -p game-sim --test mxvatv_car_phy` gave 3 passed, and `--test mxvatv_car_fields` gave 1
  passed. Both were run with `--nocapture`, and the files and images were present, so they ran rather
  than skipped.

## Not verified / not 1:1

- **The car step is not ported, and no drive is replayed.** That is the brief's test, and it is not met.
  MD39a's tape and csvs are gone, and SLUS-21104's car frame is not ported. Filed as C29, MD106 and MD107;
  MD39b is blocked on them.
- **The golf cart and sand rail field maps are not checked.** Their RAM images are lost, so the test skips
  them (it says so). Their objects sit elsewhere (0x01b0dac0, 0x01b158a0). The golf cart's class also reads
  rider boxes that `Car::read` does not keep.
- **The `[Mass Properties]` words on the body, collision record and gravity block are not compared on these
  images.** SLUS-21104's part reader works the inertia out from the model's bounding box, and I did not
  pull the car models. The reader itself is `handling::mass::Part`, proved over 10,000 generated files by
  tests/mxvatv_mass.rs. `MxvatvCar` keeps MX Unleashed's `phy::Part` (its CofG and the collision mass the
  springs use are the ones proved here), not `handling::mass::Part`.
- **The buggy-class PelvisBox is not located.** The buggy, baja, sand rail and golf cart pelvis box
  addresses in SLUS-21104 were not found; only the truck's (+0x14c0/+0x14d0) is checked. The box is the
  driver's animation, not the step.
- **A camera name longer than 31 bytes is cut to 31.** That is the snprintf as read off the decompile.
  No file has one, so it is not run against the original.
- **The recovered states may not be byte-identical to MD39a's kept `.p2s`.** They are from the same copy's
  slots, and the vsyncs match MD39a's table exactly, but the kept files themselves are gone, so there was
  nothing to compare them against.

## Next

- C29 first (needs the emulator). Then MD106, which will want splitting as MD75 was. Then MD107.
- `./kit plan reopen` brings MD39b back once all three are done.
