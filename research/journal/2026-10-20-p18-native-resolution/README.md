# P18 — native resolution

2026-10-20. Task: `./kit plan P18`.

The shell drew 640x448 and duplicated pixels up to the window. It now opens at the screen's size
(read from the X setup), draws the largest 4:3 frame that fits the window, re-makes that frame on
`ConfigureNotify`, and centres it with black bars. `--size` and `--scale` still work as before. The
native frame does **not** fit 60 Hz: about 25 fps at 1440x1080 against 59.8 at 640x448. The software
rasteriser is the cost. Details in 1.
