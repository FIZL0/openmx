# R28 — swung follow camera

2026-11-16. Task: `./kit plan R28`.

MX vs. ATV's follow camera with the right stick swinging it toward the front. MD39a's drive was
replayed with the right stick added (`research/r28/swing.pad`) behind the buggy and captured with MD82b's
`cam.toml`. The capture has 730 clean rows: 195 with the rear fade below 1, 118 of them under
29.33 ft/s. `tests/mxvatv_camera.rs` now hands the camera each row's player offsets and replays all 729
steps bit for bit. The `MOD_LAG_SPEED` mutation (29.3332 → 29.0), green before, now fails, and so do
mutations of the lag fade, the rear fade's angle and the look-ahead fade. The swing fade is still
unprovable, because `Swing` is 0 in every camera file. Details in `1-swung-follow-camera-FINAL.md`.
