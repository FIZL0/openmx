# V94 — quiet stream timing

2026-10-29. Task: `./kit plan V94`.

V92's world_ultra gas ride was timed at load 45-66, where every frame was over 16.7 ms. Re-run on the
wrapped world_ultra (main has edea158e) at load 4.6-4.8: the `stream` stage peaked at 4.44 ms and averaged
0.24 ms a frame; 14 of the ride's frames went over 16.7 ms in all, each from `present` and `ground`, none
from streaming. The order the ride streamed its 8 mid-ride pieces in follows the rider's measured
heading, and the ride never waited for the piece underfoot. New: a unit test of the heading order
(`worlds::heading_order`) and a no-wait check in the ignored ride test. Details in 1-quiet-stream-timing-FINAL.md.
