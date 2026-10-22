# V47 — train smooth

2026-10-21. Task: `./kit plan V47`.

Why does free06's train move in stutters when the original's runs smooth? The shell placed the path
cars once per 30 Hz tick and drew that placement unchanged on every frame until the next tick. The
original also moves its train once per 1/30 s step, but it draws one picture per step. The shell now
places the cars at the clock between the last two ticks, as far along as the bike's blend, so the
clip's curve and the lag walk are read at the moment shown and the cars stay coupled. Proved three
ways: a PINE watch of the original's car (1.837 ft every game frame, 55.1 ft/s); a frame trace of the
shell (no repeats over 4,081 frames, 55.1 ft per clock-second); and a disc test at 60/144/165 Hz.
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
