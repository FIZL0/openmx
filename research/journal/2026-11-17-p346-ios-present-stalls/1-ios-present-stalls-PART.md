# 1 — ios present stalls (PART)

Task P346. 2026-11-17.

## What I did

- Looked for a phone log with GPU timing. There is none. Every iPhone log we have (research/p340's,
  and ~/Downloads' phone-114109, -115536, -160542, all v0.13.0 build 4179) reads `gpu no frame timed`
  on every line a second (186, 703 and 542 lines), and never a `gpu N ms a frame`. The newest log is
  2026-11-13; nothing newer came in.
- So the GPU timer exists on the iPhone (the adapter has `TIMESTAMP_QUERY`) but no frame ever comes
  back timed. Why: V122's timer (`crates/game-render/src/gpu/timer.rs`) wrote its two timestamps from
  empty compute passes. Metal does not write timestamps from an encoder with no work in it. wgpu-hal
  30's own Metal backend says so (`metal/command.rs`, `enter_blit`: "blit encoders won't write
  timestamps if they don't have a blit operation", gpuweb#2046, and Dawn works round it). The reads
  come back zero or unset, and `take` drops a frame whose two ticks do not give a time above 0.
- Fix: the timestamps now ride on the frame's own render passes. The first pass writes the start, the
  last pass writes the end; a frame drawn in one pass (full scale, or `render_over`) writes both from
  that pass. `begin` only picks the slot now; `end` only resolves and copies. `begin_pass` and
  `begin_pass_over` take the pass's timestamp writes (the sprites-only pass passes none). The empty
  marker passes are gone on every backend, so there is one path everywhere.
- The task file `ios_gpu.rs` needed no change: the iOS surface draws through the same
  `Renderer::render`, and main.rs's line a second already prints the GPU time when frames come back.
- Took nothing from opentennis: it has no GPU timer.

## What the original does

Not applicable: frame timing on the phone is ours (AGENT.md, rendering).

## How it was proved

- New test `crates/game-render/tests/frame_time.rs`: on a headless adapter with timestamps, frames
  drawn in one pass, at render scale 0.5 (two passes), and over a picture each come back with a GPU
  time above zero. Here (desktop GPU) all 8 frames of each way were timed (21 µs, 53 µs, 7 µs for an
  empty scene). It skips, saying so, on an adapter with no timestamps.
- A 10 s `--track free06 --timings` ride on a private weston (llvmpipe, GL) prints
  `gpu 41.6–88.8 ms a frame` each second, so the line a second reads the new timestamps.
- `cargo check -p game-render --target aarch64-apple-ios` builds.

## Not verified / not 1:1

- **Not run on Metal.** No Mac or iPhone here. That empty encoders write no timestamps on the iPhone
  is inferred from the logs (timer present, no frame ever timed) and wgpu-hal's notes. That timestamps
  on render passes (Metal's start-of-vertex / end-of-fragment sample points) do come back on the A17
  Pro is not seen yet. The next phone log shows it: `gpu N ms a frame` in place of `gpu no frame timed`.
- **The test cannot fail the old way here.** The old empty-pass timer worked on Vulkan and GL, so
  frame_time.rs passes with either timer on this machine. It guards the three pass layouts, not Metal.
- **What the GPU time now covers differs a little.** Before: from an empty pass before the views to one
  after the frame. Now: from the start of the first view pass to the end of the last pass. The ground
  meshes and sun maps were before the first mark already, so in practice the same work is timed; not
  measured side by side on the desktop.
- **The task itself is not done.** What holds the drawable in the 28–55 ms present stalls (research/p340
  log, 6 s, 8 s, 10 s) is still unknown. It needs a phone log from a build with this change.

## Next

The human: ride on the iPhone for a few minutes with a build from this change on, and send the log
(LocalSend to ~/Downloads). Then the next session reads, for each second with a 28–55 ms frame, the
`gpu N ms a frame (worst M)` field: worst M near the stall means the GPU is late (look at what it drew
that frame); a small GPU time beside a big `waited` means the drawable was held by the compositor.
