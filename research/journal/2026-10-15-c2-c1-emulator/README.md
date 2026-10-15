# C2 — c1 emulator

2026-10-15. Task: `./kit plan C2`.

The kit's frame clock. MX Unleashed simulates at 30 Hz on a 60 Hz display and keeps two counters: the
game's own one at `0x479d04` (R4's `@frame_counter`, +1 per simulated frame, frozen by the blocking
loaders and by a halted EE) and libpad's pad-buffer field counters at `0x47a9d8 + n*0x80` (+2 per frame,
still running through a 20-second load). `./kit probe vsync` can only ever find the second kind — its
rate band starts at 45 Hz — and the one it would save makes `./kit record` call every frame a dropped
frame, so `[emu] vsync_addr` is set to `0x479d04` by hand. Proved with a 300-frame `./kit pine watch`
(299 deltas, all +1), a 390 000-sample tight read loop, an emulator-pause test and a forced mode
transition. The in-game pause menu was not reached; see the FINAL file for that gap and the rest.
