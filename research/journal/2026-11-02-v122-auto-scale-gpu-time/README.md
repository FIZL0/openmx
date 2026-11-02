# V122 — auto render scale judged on GPU time

2026-11-02. Task: `./kit plan V122`.

The Steam Deck's split screen fell from 0.9375 to 0.5 render scale when 2P joined and stayed there:
`AutoScale` was fed `w.draw`'s wall time (surface acquire + draw + present), so CPU-late frames (sim
spikes of 17-60 ms) pushed the mean over the budget, and under VSync the waits never let the mean fall
below half the budget to step back up. Details in 1-gpu-timestamps-FINAL.md.
