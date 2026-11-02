# What changed

- `game-render/src/gpu/timer.rs` (new): where the adapter has `TIMESTAMP_QUERY` (the device now asks for
  it when offered), `Renderer::render` writes a timestamp before the views' pass (after the sun maps and
  the ground meshes' follow, which do not go with the pixels) and one after the frame's pass (views,
  upscale, overlay), as empty compute passes with `timestamp_writes`, so `begin_pass` and its callers are
  untouched. Four slots in flight; each is resolved, copied to a mappable buffer and read a frame or few
  later. `Renderer::gpu_times()` drains them with the scale each frame was drawn at; `Renderer::timed()`
  says whether there are any.
- `AutoScale::gpu(spent, drawn_at)`: one timed frame, rescaled by (scale / drawn_at)² so late samples
  from before a change are read at the current scale. `AutoScale::frame(cpu, spent)` is the fallback
  (WebGL2, or a WebGPU without timestamps): a frame whose CPU part (time since the last draw began, less
  that draw's surface waits) ran past the budget is left out, and so is the one after it (it takes up
  the queue's backlog).
- Climbing: a sixteenth up only when the mean, scaled by the next step's pixels, is under 0.85 of the
  budget (was: mean under half the budget). After a drop the scale lands at ~0.9 of the budget, which
  is above the climb line, so it does not bounce.
- `openmx/src/main.rs`: feeds `gpu()` with every drained timing where timed, else `frame()`.

# Proof

- `late_cpu_frames_do_not_drop_the_scale_and_a_light_gpu_climbs_back` (scale.rs): untimed frames with
  17-60 ms CPU parts and backed-up draws keep 1.0; a GPU-late untimed frame still drops it; timed, from
  0.5 a GPU at a third of the budget (cost ∝ scale², read two frames late) climbs back to 1.0; a GPU at
  1.3× the budget settles under it and holds for the last 10 judgements.
- `the_gpu_times_its_frames_where_it_can` (gpu.rs): on this machine's RADV (Strix Halo) three frames
  at 1.0/0.5/1.0 came back as 122 µs / 51 µs / 106 µs with their scales.
- `cargo test -p game-render --lib` all 136 pass; `cargo check -p openmx` builds.

## Not verified / not 1:1

- Not run on the Steam Deck, nor in a windowed 2P ride here: the climb under Fifo is shown by the unit
  test, not by a live session's `render scale:` lines.
- The wasm32 build was not compiled (no wasm target in this toolchain); the timer uses only portable wgpu
  API, and WebGL2 has no timestamps so it takes the fallback.
- The fallback under VSync still cannot see room to climb (its wall time holds the display's wait); only
  timed adapters climb freely.
- Rendering only; nothing touches the simulation.
