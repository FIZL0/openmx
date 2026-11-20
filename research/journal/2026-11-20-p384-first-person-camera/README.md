# P384 — first person camera

2026-11-20. Task: `./kit plan P384`.

The disc's own first-person camera, ported from SLUS_207.67 with its `appstart\fppreset.ini`: the camera
button walks onto "First Person" after the bike's presets (or `--camera "First Person"`), its eye on the
rider's Head node, its look swung by the yaw rate and lagged with the file's numbers. MXA-style adjust is
`--fp-adjust` (ideas only; MXA files absent). The roll is computed but not drawn (the renderer has no up).
The spec review's fixes (one-preset bikes, section lags) are in. Details in 1-first-person-camera.
