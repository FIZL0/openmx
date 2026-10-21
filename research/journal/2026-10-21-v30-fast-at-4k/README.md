# V30 — fast at 4k

2026-10-21. Task: `./kit plan V30`.

The shell ran at 4.9 fps (1 player) and 3.1 fps (4 players) at 3840×2160 on the GPU renderer. Almost
all of that time was the CPU drawing the HUD and the pause at window size and uploading the result as
overlay patches.

The HUD and every menu screen, the pause included, are now GPU sprites. A 4K frame costs about 2 ms
(draw 0.7 ms), against 196–302 ms before. The front end's splash went from 83 ms to 0.33 ms.

Also new:
- a `--render-scale auto|F` (auto by default, aimed at the display's refresh);
- a cheaper debug HUD;
- tests: a 4K frame within a 120 Hz budget (1.09 ms), half scale against full scale, and GPU against CPU
  for the HUD and for every menu screen.

The loop still paces at 60 Hz until P52, and the render scale's settings row is V34. Details in
1-fast-at-4k-FINAL.md.
