# V73 — nat01 rain

2026-10-28. Task: `./kit plan V73`.

nat01's rain is ported. The `.toy`'s weather box (`Camera0Weather`) follows camera 0, and its
`RainBegins`/`RainEnds` triggers ramp its density from 0 to 1 and back at 0.01 a frame. The read, the
actions, the ramp and the packet restart/cut follow the PS2 functions, and the streaks are drawn by eye as
additive glows of `rain_gpu.pst` in both renderers. It was not compared with the original's picture, and
the random draws are the box's own (see 1-nat01-rain-FINAL.md).
