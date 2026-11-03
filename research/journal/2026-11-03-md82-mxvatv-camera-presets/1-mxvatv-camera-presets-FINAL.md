# MD82a — which camera each MX vs. ATV machine follows with

MD82 asked for each mod machine's own camera, proved against a SLUS-21104 capture. None of the
mod machines ride in the shell yet (MD37d, MD39c, MD80, MD81 are open), so MD82 was split:

- **MD82a** (this entry): what file each machine reads and what the game reads out of it, proved
  against the follow camera's records in RAM.
- **MD82b**: port SLUS-21104's follow-camera update, with eye/target captures behind the cars and
  the ATV.
- **MD82c**: biplane and helicopter captures (after MD80).
- **MD82d**: the shell follows each mod machine with its presets (after MD37d, MD39c; Untamed
  after MD81).

## Where the presets are

The `[Camera]` section of each `Veh\*.phy` holds only first-person data:

- the cars: `FPCameraOffset`;
- the ATVs and bikes: `Left/Right1stPerShoulderOffset`;
- the biplane and Kiowa: `ModelOne/ModelTwoFPCameraOffset`.

The reader (oracle `atv_vehicle_camera_phy`) also takes `FCamFileName`, but no `.phy` on the disc
sets it. So the file comes from the default, which is picked by the machine's kind word
(`atv_vehicle_camera_defaults`):

| kind | file |
|---|---|
| 1 | FC_ATVDefault.pb |
| 2 | FC_BiplaneDefault.pb |
| 3 | FC_HeliDefault.pb |
| 4 | FC_BuggyDefault.pb |
| 5 | FC_MonsterDefault.pb |
| 6 | FC_BajaDefault.pb |
| 7 | FC_GolfCartDefault.pb |
| 8 | FC_SandrailDefault.pb |
| other | FC_MXDefault.pb |

The camera controller prefixes `Cam\`. If the race camera is the current one, it swaps `FC_` for
`RC_`. It then loads the file out of `Cam\CameraPresets.res`, which is inside `MXVSATV.PSR` and holds
20 files: FPPreset.ini, FC_ATV.ini, and nine each of FC_ and RC_. The `.pb` files are text inis.

## The loader, against MXU's

These files have no `[NumberOfPresets]`. The sections are `[<aspect>/<viewport>/<n>]`:

- aspect is `1x1`, `4x3` or `16x9`, by the widescreen global;
- viewport is `1p`, `2ph`, `2pv` or `4p`;
- n counts from 1 for as long as the section exists;
- with no sections and an aspect other than 4x3, the loader retries at 4x3; at 4x3 with none, it
  fails.

The key reads are MXU's, in MXU's order, with one exception: the lean swing (record +0x80) is read
under `Swing`. The files say `CameraSwing = 0.10`, so **every MX vs. ATV follow preset has zero
swing**, and the RAM records show 0. The RC_ files add race-camera airborne filters ahead of the
presets. They are not read here.

## The code

- `game_sim::camera`:
  - new `Aspect` and `Viewport` enums;
  - new `Preset::read_layout(ini, aspect, viewport)`;
  - the private `Preset::one` now takes the swing key. MXU's `read` still passes `CameraSwing`.
- `chase.rs`: new `ModMachine`, `mod_presets` (the FC_ file), `mod_race_presets` (the RC_ name) and
  `MOD_CAMERA_ARCHIVE`.
- `tests/mxvatv_camera_presets.rs`: for each of eight RAM images, the test:
  - finds the follow camera by its vtable word;
  - checks that the file it loaded is the one `mod_presets` names;
  - reads that file at 4x3/1p;
  - compares the count, the current preset (0), and every record field (filters, offset, name and
    the twenty words +0x50..+0x9c) bit for bit.

  The images are md39a buggy/baja/sandrail/golf/truck, md37a atv-r-spawn/atv-s-air and state04 (the
  bike). A second test covers the fallback, the 16x9 count, the empty 4p and the RC_ name.

## The update (for MD82b)

The follow camera's update (vtable `atv_follow_camera_vtable`) is 1852 bytes; MXU's race camera
update is 1848. Differences found so far, not a full diff:

- **Heading.** It uses the velocity heading when airborne, speed > 20 and |speed − |vel.y|| > 0.01.
  MXU runs two tests here.
- **Distance.** It is user offset + preset distance (MXU adds one more term), floored at 4.5 × the
  viewport's near clip.
- **Look-ahead and ground probe.** These go through their own callees.

## How it was checked

- `./kit check -- -p openmx --test mxvatv_camera_presets --test car_camera`: 24 passed, nothing
  skipped. The log line for each of the eight images reads "every field as in RAM".
- Mutation: reading `CameraSwing` in `read_layout` fails on `Swing (+0x80): ram 0x00000000, port
  0x3dcccccc`. Reverted.
- main (292166a5) does not build on its own: `vehicle.rs:2592` calls `Blades::new` with four
  arguments after S271 made it five. To run the tests, `None, None` was patched in the slot and
  reverted before the commit. Filed as P273.

## Not verified / not 1:1

- No camera eye or target capture was taken, so nothing here replays a camera. That is MD82b.
- The update diff is partial (above).
- No state for the biplane or helicopter, and no 16x9 or split-screen state. Only
  4x3/1p records were compared against RAM. The other layouts are read by the documented rule, which
  no image checks.
- The RC_ files' header keys are not read. The race camera holds no presets in any of these images
  (free ride).
- The flags are compared as whole words (1/0); the loader's write width was not checked.
- Untamed's machines were not looked at (MD81).
- The camera files come from the disc copy in `context/mxvatv-ps2/md82/cam/` (discx
  `Cam\CameraPresets.res`). The shell does not read them yet, and the importer does not bring them
  in.
