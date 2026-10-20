# P44 — gpu renderer

2026-10-20. Task: `./kit plan P44`.

The shell drew everything in software on one CPU thread: 1.6 fps at 1080p with one player and 1.1 with
four. It now draws through wgpu by default (`crates/game-render/src/gpu.rs`, opened by
`crates/openmx/src/gpuwin.rs`), with `--renderer software` as the fallback. At 1080p it holds 60 fps
with one view and with four, at 1–2 ms of work a frame, and the ride hash is unchanged. `--fps` prints
the frame rate, the slow frames and the time per stage. The browser half is split off as P47. Details:
[1-gpu-renderer-FINAL.md](1-gpu-renderer-FINAL.md).
