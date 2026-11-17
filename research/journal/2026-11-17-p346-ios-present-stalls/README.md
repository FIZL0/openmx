# P346 — ios present stalls

2026-11-17. Task: `./kit plan P346`.

Wanted: find what holds the drawable in the iPhone's 28–55 ms present stalls, from a phone log with GPU
timing. No such log could exist: the GPU timer wrote its timestamps from empty passes, which Metal does
not time, so every iPhone log reads "gpu no frame timed". Moved the timestamps onto the frame's own
passes (tested on the desktop GPU). Blocked on a phone log from a build with this change.
