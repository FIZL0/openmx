# 1 — untamed follow camera

Task MD251. 2026-11-19.

## What I did

- **game-sim `camera.rs`.** A new section, "MX vs. ATV Untamed's follow camera (MD251)":
  - `UntamedPreset::read` reads `4x3/1p/n` as SLUS-21701's reader does, with its defaults.
    `UntamedPreset::is_slow_fast` spots the layout. `blend_at` blends slow and fast by speed.
    `stand_in` makes an MX Unleashed `Preset` for the shell's preset cycle and look-back.
  - `UntamedFollow`: the goal (`0x004b52b0`), the two springs (`0x003aa690` straight, `0x003aa000` orbit),
    the snap and the eye's 2.5 ft floor.
- **openmx `chase.rs`.** `untamed_presets` reads the file. `untamed_follow` steps the Untamed camera and
  takes the preset, the stick's yaw and pitch and a snap from the quad's MX Unleashed camera.
- **openmx `main.rs`.** `atv_own` keeps the Untamed presets when the `.pb` is slow/fast and says so.
  `kind_presets` gives the quad stand-ins. Each `Rig` has an `untamed_camera`, which draws the quad's view.

## What the original does

- Section names are `"%s/%s/%d"` with `"4x3"` and `"1p"`, so only the 4x3 one-player presets are read.
  The file's other screens are never used.
- The reader asks for `Swing` and `LookSwing`. The file spells the first `CameraSwing`, so it reads 0.
- Target = body point + up × 3 + the preset's offset (forward, up and side rows).
- Heading: the quad's forward row under 20 ft/s, the velocity from 20 ft/s. Its y is scaled by
  `AngleXAffectedByHills` (at least 0.0001).
- Pitch = blended AngleX + the stick's pitch, clamped to ±85°, times the quad's up row's y on the ground.
- The look-at is the blended look-ahead along the heading, shrunk as the stick turns the camera.
- Springs: the eye orbits the target past 3 ft of follow distance; the look-at moves straight. Nearer, the
  other way round. A preset with `FilterThisPreset=0` snaps every frame.

## Checked

- `cargo test -p game-sim --test untamed_camera`: 6 pass. At spawn, the goal's target, look-at and eye
  equal the original's RAM words to 1e-4; fov 75; tightnesses 14 and 35.
- `cargo test -p openmx --test untamed_atv --test atv_camera_sound --test machine_packs`: 4, 19 and 30
  pass, with `untamed_atv` and `mxvatv_atv` in the worktree's `mods/machines/` (no skips).
- `cargo test -p openmx --bin openmx`: 334 pass.
- A headless shot of the quad at rest on free09 (640x448, frame 95) against spawn.p2s's Screenshot.png:
  the quad sits the same size and place on screen, rider's head at the same height.

## Not done

- The filter's world sweeps (`0x003ab3a0`, `0x003ab800`): the eye can go into walls and props.
- The Camera page's framing changes reach only the stand-in presets, not the Untamed camera.
- The cars' `FC_*Default.pb` files, same layout, still ride their `.ini`s.
