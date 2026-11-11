# 1 — mxvatv fliers own cameras sounds particles (FINAL)

Task MD108. 2026-11-11.

## What I did

With `--fliers mxvatv` (or a seat's Garage pick, or a tape/pause switch into a pack skin), MX vs. ATV's
biplane and Kiowa now read the mxvatv_fliers pack's own files where its `mod.json` names them, and fall back
to MX Unleashed's disc files where it does not:

- **Camera.** `vehicle::PackFlier` carries the flier's `cameras` file (`Cam/FC_BiplaneDefault.pb`,
  `Cam/FC_HeliDefault.pb`). `chase::flier_presets` reads it with `game_sim::camera::Preset::read_layout` for
  the screen's aspect (Wide / Standard) and viewport (Full, SplitH, SplitV, Quad). If the viewport has no
  presets it falls back to Full. main.rs's `own_flier_cameras` hands each rig its pack presets when the ride
  loads and after every refit (tape switch, pause switch). `set_display`, `set_heli_display` and the
  per-rig framing read them before the disc's. Lines: `biplane: mxvatv_fliers's camera, 8 presets in …`,
  `… its camera starts on Race Camera 1`, and `biplane: MX Unleashed's camera` when switched back.
- **Sounds.** `audio::OwnFliers` (a plane's LoopEngine + wreck + touchdown, a heli's HeliEngine + wreck) per
  seat, built from `machine_packs::read(folder).flier_sounds(flier)`. `Sound::frame`'s plane and heli
  branches use the seat's own voices when it has them, and the disc's otherwise. Set at load and on refit.
- **Particles.** `load_plane_fx` / `load_heli_fx` take the pack. `pack_fx_read` reads the pack's
  `Part/biplaneparticles.dat` / `helicopterparticles.dat` and its `.emt` emitters first, and the track's
  otherwise. Each livery flown off the pack gets its own heli list (`heli_fx_own`).

## What the original does

Not ported here. This task is wiring: the files are SLUS-21104's own, but the steps reading them are
still SLUS-20767's (MX Unleashed's Race camera, SeatSound's engine model, the particle emitters).

## How it was proved

- `crates/openmx/tests/mxvatv_fliers.rs::the_fliers_read_the_packs_cameras_and_particles`. On free09 with
  `--fliers mxvatv --flier-skin 3`, the camera and particle lines name the pack's files. A tape switching
  into `biplane@mxvatv_fliers/biplane3` and then `biplane@original/biplane2` swaps the camera both ways.
  The existing off-by-default test still asserts that no `mxvatv_fliers's` line appears without the switch.
- Sounds: a windowed run on a private weston with a fake `pw-cat` (memory: shell-sound-to-a-file) said
  `biplane: mxvatv_fliers's sounds, 7 engine loops, 5 wreck, 1 touchdown` and
  `kiowa: mxvatv_fliers's sounds, its rotor loop and 5 wreck`, and wrote the stream. Headless has no sound,
  so there is no test for it.

## Not verified / not 1:1

- The camera steps on MX Unleashed's Race camera with the pack's presets, not on SLUS-21104's ModFollow
  (MD82b/MD82d, noted there). No eye/target is compared against a SLUS-21104 capture.
- RC_ replay-camera files are not read.
- A 4-player (Quad) viewport with no presets falls back to the Full screen's. I did not check what
  SLUS-21104 does there.
- The pack camera starts on its first preset. That is an assumption; I did not read SLUS-21104's start index.
- The particle zone table and particle sheet stay the track's (MX Unleashed's). An MX vs. ATV `.emt` group
  name missing from that sheet draws nothing. I did not check them one by one.
- Plane particles still run for seat 0 only, as before. Neither the plane's nor the heli's lists are
  reloaded on a mid-ride refit; they are the ones loaded with the ride.
- Joined players' (drop-in) rigs get pack cameras only at the next refit.
- The sounds were checked only by the load line and the written stream. I did not listen to them beside
  MX vs. ATV in PCSX2, and engine pitch/volume still follow MX Unleashed's model.
- opentennis had nothing applicable.

## Next

None for this task. MD82d carries the ModFollow camera.
