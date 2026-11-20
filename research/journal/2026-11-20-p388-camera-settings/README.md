# P388 — camera settings

2026-11-20. Task: `./kit plan P388`.

Camera settings past what we had, after MXA. FOV, distance and height per preset were already there (P82,
P163). Each preset now also has Angle (its `AngleX` swung up or down), Look Lag and Follow Lag (its
`LookFilters` / `PosFilters` time constants as a percent). They start on the disc's numbers and save in
`settings.txt`, and an old file reads the same. The Xbox build's presets as a choice went to P388a,
because the Xbox disc is not on this machine. MXA's own files were not here to compare. Details in
`1-camera-settings-FINAL.md`.
