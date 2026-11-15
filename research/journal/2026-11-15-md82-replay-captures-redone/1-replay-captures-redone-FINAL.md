# 1 — replay captures redone (FINAL)

Task MD82. 2026-11-15.

## What I did

- **Camera files.** `discx x mxvatv 'Cam\CameraPresets.res:**'` (openmx-mods) pulled all 20 files into
  `context/mxvatv-ps2/md82/cam/`. That is the shared context, git-ignored.
  - FC_ATVDefault.pb and FC_HeliDefault.pb are byte for byte the same as the packs' copies.
- **Captures.** All eleven were re-recorded in my own copy (KIT_PCSX2=2, KIT_CONFIG=`context/c32-atv-kit.toml`):
  - the seven cars and ATVs with `research/md82b/play.sh` on MD39a's `drive.p2m2`;
  - the four fliers with `research/md82c/fly.sh`;
  - each into `context/capture/md82b|md82c/`.
  - Captures that dropped ticks under the other slots' load were taken again. biplane3 only came out clean
    with the copy's ini at NominalScalar 0.5, and 0.25 made things worse. The ini is back at 1.0.
- **`crates/openmx/tests/mxvatv_camera.rs`.** The recorder's poll lands anywhere in the frame under load,
  and the old loop only knew one case: a "held" camera whose next row is two steps on. Comparing two
  sandrail captures of the same deterministic ride showed what really happens:
  - 496 rows agree;
  - in 193 rows, one capture's camera is the other's from a tick before;
  - 41 rows are torn inside the camera step (lift new, lags old, or eye old).

  The new rules:
  - **Every row is stepped on its own target.** Each compared field must be the port's after this
    tick's step or before it, bit for bit. Rows that needed "before" are counted as read late. This
    replaces the held rule and covers its case.
  - **Lost target, reseed.** A row's target can be the last row's word for word (on a moving machine),
    two ticks on (the point moves more than 1.6× its speed's worth, above speed 5), or past a vsync gap.
    The tick between then has no target block, so the camera reseeds after it, as it already did after
    ATV wrecks.
  - **Seed.** The seed is the first of twelve rows whose next three rows all pass the field rule, with at
    least one of them holding this tick's lags exactly. A seed one tick off never lands the lags.
  - **Coverage.** Rows compared (on time + late) must be over 80% of the rows that hold a target. This
    replaces the old 600-row floor, so the ATV wreck gaps fit too.

## What the original does

Nothing new about the camera. SLUS-21104's MXCameraFollow is as MD82b ported it. The order of a frame
(machine first, then the camera, whose lift is written before its lags, then the viewport's eye) is read
off the captures' torn rows, not from the code.

## How it was proved

`./kit check -- -p openmx --test mxvatv_camera -- --nocapture`. Every capture replayed:

| capture | file | on time | late | target lost | wrecked | reseeded rows |
|---|---|---|---|---|---|---|
| buggy | FC_BuggyDefault | 729 | 0 | 0 | 0 | 0 |
| baja | FC_BajaDefault | 729 | 0 | 0 | 0 | 0 |
| sandrail | FC_SandrailDefault | 525 | 105 | 16 | 0 | 86 |
| golf | FC_GolfCartDefault | 728 | 0 | 0 | 0 | 0 |
| truck | FC_MonsterDefault | 723 | 5 | 0 | 0 | 0 |
| atv-r-spawn | FC_ATVDefault | 589 | 0 | 0 | 136 | 4 |
| atv-s-spawn | FC_ATVDefault | 527 | 80 | 9 | 77 | 44 |
| biplane | FC_BiplaneDefault | 1624 | 0 | 0 | 0 | 0 |
| biplane3 | FC_BiplaneDefault | 2337 | 0 | 0 | 0 | 0 |
| kiowa | FC_HeliDefault | 1263 | 0 | 0 | 0 | 0 |
| kiowa3 | FC_HeliDefault | 1379 | 0 | 0 | 0 | 0 |

Two mutations, both caught and both reverted with cp + touch:
- the velocity-heading speed 20 → 21 fails truck row 463;
- MOD_SPEED_REFERENCE 117.3328 → 117.3329 fails sandrail row 108. So even the noisiest capture, with
  its late rows, still catches a one-ulp change.

`mxvatv_camera_presets` (now with the camera files present) and `mod_follow_camera` pass as well.

## Not verified / not 1:1

- **Sandrail and atv-s-spawn are noisy captures.** Under the machine's load I could not get them
  clean at 1.0, 0.5 or 0.25 speed:
  - sandrail: 105 of its rows were checked against "before or after" rather than exactly, and 86
    were reseeded past;
  - atv-s-spawn: 80 and 44.

  The reseeded rows are not compared at all. A port error confined to those ticks would not show.
- **Late rows check each field against two values.** A field that does not move in a tick (distance,
  yaw) passes either way. The moving fields still pin the step down; the mutation above shows that.
- **The two-ticks-on test only works above speed 5.** In the clean buggy capture, a nearly still
  machine's point moves up to 8× its speed's worth, so a slow target read a tick early is not caught
  by distance. It then fails the field rule and the test fails loudly. No current capture does that.
- **The frame order** (machine, camera lift, lags, viewport eye) is read off torn rows, not from the code.
- **The shell's target blocks for mod machines are still the port's guess** (MD82d's note); this replay
  hands the camera the original's blocks.
- **Untamed's machines:** none to follow. MD81 found that Untamed adds no machine, only retuned `.phy`
  files.
- **The `RC_` race-camera files are not read** (MD165).

## Next

Nothing for MD82.
