# 1 — swung follow camera (FINAL)

Task R28. 2026-11-16.

## What I did

- **The tape.** `research/r28/swing.pad` is MD39a's `drive.pad` tick for tick, with the same buttons and
  left stick on all 735 ticks. The right stick is added on top. Checked by comparing the two `.ticks.csv`
  files that `research/c15/tape.py` writes.
- **The capture.** I played the tape from `context/mxvatv-ps2/md39a/buggy.p2s` with MD82b's
  `research/md82b/play.sh` and `cam.toml`. The camera is 0x1cd9a50 and the state's vsync is 27957.
  - Emulator: my own copy 6, `KIT_CONFIG=context/c32-atv-kit.toml`, started with
    `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 WAYLAND_DISPLAY=wayland-1`. Without the
    accessibility variables, play.sh stops at "no accessible app".
  - Output: `context/capture/r28/buggy.csv`, 730 rows. Every row steps vsync by 2: no gaps, no late rows.
  - Command:
    `./kit pcsx2 with -- bash research/md82b/play.sh PID context/recordings/r28/swing.p2m2 context/mxvatv-ps2/md39a/buggy.p2s 0x1cd9a50 27957 730 context/capture/r28/buggy.csv`
  - `cam.toml` needed no change. It already reads the player's offsets (`cam_user`, cam +0xd8..+0xe4).
- **The test.** `crates/openmx/tests/mxvatv_camera.rs` lists the capture as `("r28", "buggy", Buggy)`.
  Before each step it now gives the camera that row's four player offsets (`stick`), the same way it
  gives the target block. Every MD82b/MD82c row holds zeros there, so their replays are unchanged.
- Nothing from `../opentennis`: it has no follow camera like this one.

## What the original does

- **Full right stick turns the user yaw (cam +0xdc) by 0.05 a tick.** That fits MX Unleashed's
  `Follow::nudge`. The camera's yaw is π minus the user yaw, wrapped. On release the user yaw stays
  where it is, with no spring back. No clamp was seen out to ±2.95.
- **The stick does nothing for about the first 36 ticks after the state loads.** It was held from
  tick 20, and the user yaw first moved around row 36.
- **The lag taus are rewritten every frame, as MD82b ported.** With the yaw past the rear fade, look0's
  tau goes 0.075 at speed, 0.2467 at 6.95 ft/s, and 0.2996 standing. That is 0.3 × fade(speed,
  0..29.3332, 1 → 0.25). The capture holds 130 distinct look0 taus.

| | rows |
|---|---|
| total | 730 |
| \|yaw\| < π/2 (rear fade < 1) | 195 |
| … of them with speed under 29.3332 | 118 |
| \|yaw\| inside the rear fade band (1.0472..π/2) | 20 |
| \|yaw\| < 1.0472 (rear fade at its low end) | 175 |
| \|yaw\| inside the swing fade band (π/2..2.0944) | 437 |

## How it was proved

- **Replay.** `./kit check -- -p openmx --test mxvatv_camera -- --nocapture`:
  - r28 buggy: 729 rows replayed bit for bit from row 0, with 0 late, 0 dropped and 0 reseeded.
  - The eleven MD82b/MD82c captures replay exactly as before (MD82 2026-11-15's table).
- **Mutations** of `crates/game-sim/src/camera.rs`, each restored with cp + touch:

  | mutation | result |
  |---|---|
  | `MOD_LAG_SPEED` 29.3332 → 29.0 (green before this task) | fails r28 buggy row 432 |
  | `MOD_LAG_FADE` 0.25 → 0.26 | fails row 404 |
  | `MOD_REAR_FADE` low end 0x3f860a91 → 0x3f860a92 (one ulp) | fails row 406 |
  | look-ahead's `rear(0)` → `rear(0.001)` | fails row 405 |
  | `MOD_SWING_FADE` high end 2.0943952 → 2.2 | **stays green** (see below) |

## Not verified / not 1:1

- **The swing fade is still unproved.** The capture has 437 rows inside its band. But the fade
  multiplies the preset's `Swing`, which is 0 in every camera file (MD82a's key quirk), so the
  term is zero whatever the fade is. A mutation of its angle stays green. Only a camera file with a
  non-zero `Swing` could prove it, and the disc has none.
- **The stick's mapping into the user yaw is not ported or proved here.** The replay takes the user
  offsets from each row; it does not run the stick through `nudge`. Things not checked:
  - full stick reads as 0.05 a tick. In a first capture, thrown away, half stick gave about 0.014 a
    tick rather than 0.025, so the stick's curve is not linear. That curve was not looked into;
  - the dead time after the load (about 36 ticks) is not explained.
- **Only one capture: the buggy, Desert Oasis, 1p 4:3.** No flier and no other car with the stick.
  The fades are the same code for every machine (MD82b/MD82c), but that rests on reading the code,
  not on a capture per machine.
- **The rear fade's band is only crossed in passing.** 20 rows sit inside 1.0472..π/2, all of them
  while the stick is moving, and none with the yaw held still inside the band. They still catch a
  one-ulp change of the low end.
- **The user pitch, zoom and fov stayed 0.** The tape only moves the right stick's x. The pitch-down
  lock and the zoom/fov modifiers are not exercised.
- **The ride differs from MD82b's buggy capture from row 17, by about 0.002 ft.** That is before the
  stick had moved anything (user yaw 0), and it grows into a different ride: 30 airborne rows against
  167. Both captures replay against their own targets, so the camera proof is not affected. I did not
  find out whether MD82b's capture used a different drive tape or the machine is not deterministic
  across runs.

## Next

Nothing for R28.
