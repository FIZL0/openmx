# P346 — gpu times on the phone

2026-11-22. Task: `./kit plan P346`.

Wanted: what holds the drawable in the iPhone's long frames, from a phone log with GPU times. The 11-17
change still left nearly every phone second untimed: the timestamps were resolved in the frame's own
command buffer, before Metal had written the last pass's end. They are now resolved in a submit of their
own once the frame is done; on the iPhone 15 Pro every second is timed (sx01: 129 of 129 untimed before,
0 after). With that, a 350 s world_ultra ride answers it: the GPU holds the drawable — 117 of 118 seconds
with a frame of 28 ms or more had a GPU frame over a refresh, 20–40 ms. The GPU gets slow in waves as the
phone heats (same scene at 2–4× the GPU time, the sim slowing too, thermal fair then serious). The
user's 11-13 few-second stalls were not seen on either ride. Follow-up P399: let the GPU-timed render
scale step down under the Auto preset.
