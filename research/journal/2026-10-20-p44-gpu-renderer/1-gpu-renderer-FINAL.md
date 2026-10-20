# 1 — gpu renderer (FINAL)

Task P44. 2026-10-20.

## What I did

**Measured first.** `--fps` (`crates/openmx/src/perf.rs`) prints a line once a second. It gives the
frame rate, the average and worst frame, how many frames went over 16.7 ms and over 33 ms, and each
stage's average and worst time: sim, anim, skin, scene, draw and present. Any frame over two refreshes
also gets a line of its own. This is opentennis' `HST_FPS` idea (its z1-perf and b163-lag-spikes
entries): the one frame a rider feels shows up as a line, not as a dip in an average. Every run below
is release, rides `free06` on a generated 900-frame pad tape (full gas, the stick swinging), and was
measured on this machine (AMD Radeon 8060S, RADV, Vulkan) with the other slots loading it (load
10–13).

| run | renderer | fps | frame (avg) | sim | anim | skin | scene | draw | present |
|---|---|---|---|---|---|---|---|---|---|
| 1 player, 1920×1080, headless | software (before) | 1.6 | ~620 ms | 0.15 | 0.03 | 0.09 | 0.02 | 607 | — |
| 4 players, 1920×1080, headless | software (before) | 1.1 | ~920 ms | 0.60 | 0.07 | 0.42 | 0.11 | 915 | — |
| 1 player, 640×448, headless | software (before) | 9.7 | ~103 ms | | | | | 102 | — |
| 1 player, 1920×1080, headless | **gpu** | 59.6 (paced) | 0.9 ms | 0.20 | 0.01 | 0.01 | 0.01 | 0.20 | 0.45 |
| 4 players, 1920×1080, headless | **gpu** | 59.7 (paced) | 1.6 ms | 0.61 | 0.02 | 0.02 | 0.03 | 0.30 | 0.63 |
| 1 player, 1920×1080 window on :0 | **gpu** | 59.8 | 0.8–2.3 ms | 0.21 | 0.03 | 0.01 | 0.02 | 0.35 | 0.19 |
| 4 players, 1920×1080 window on :0 | **gpu** | 59.6 | | 1.11 | 0.11 | 0.04 | 0.08 | 0.73 | 0.30 |
| 1 player, 640×448 window | software (fallback) | 9.0 | | 0.17 | 0.03 | 0.10 | 0.02 | 107 | 1.1 |

Times are milliseconds. The loop paces to 60 Hz, so the fps column sits at the cap, and the headroom
is in the frame column. On the GPU, headless "present" is the wait for the queue to finish. In a window
it is acquiring the surface plus the present (Mailbox). Each run had one slow frame (36–48 ms), at
5.0 s. That is the `--shot` at frame 300: a readback plus a PNG write.

**The renderer** (`gpu.rs`; the design is in the module comment):
- Every model, material and texture, the scene's props and the ground are uploaded once when a ride
  starts: 137 models, 59 materials, 58 textures and 576 ground chunks of 32×32 cells, in 0.3–0.5 s.
- Skinning is on the GPU. A pose is the node matrices, kept in a palette texture rather than a storage
  buffer, so WebGL2 can take it. `rider::Body::palette` gives the body's pose with the hands and feet
  held, and the chassis' pose is its `world`. Because of this the CPU no longer re-makes each rider's
  triangles every frame (`skin_into_at`), and no longer re-skins the chassis (`skin_into`). Each player
  also no longer needs its own `@N` copy of the bike and body models: the copies are skipped when the
  GPU draws.
- Props are culled per view against the frustum and the fog, batched by model and level of detail,
  and drawn instanced, one call per material. Opaque runs go first, then the blended ones.
- One render pass draws every split view (V22), each view as a viewport with its own camera uniform.
- The ground is drawn by chunk, culled by bounding sphere, and its paint (`Splat`) is generated into
  the ground shader.
- The HUD and the pause menu are still drawn by the CPU code they always used
  (`crates/openmx/src/overlay.rs`). Each is drawn twice, into a black frame and a white frame, and
  becomes a patch: multiply by (white − black), then add black. The GPU lays the patch over the picture.

**The shell:**
- `Screen` (`crates/openmx/src/screen.rs`) is either the winit+wgpu window (`gpuwin.rs`) or the old
  X11 window. The GPU window falls back to the X11 window, saying so, if no GPU can be had.
- With no window (`--headless`), the GPU draws offscreen. `--shot` reads the frame back.
- The surface prefers Bgra8Unorm/Rgba8Unorm, because a float surface (the first format RADV lists is
  Rgba16Float) is linear and washes the colours out. It prefers Mailbox, then Immediate, then Fifo.

## What the original does

Nothing in this task is the original's. Rendering is ours (AGENT.md).

## How it was proved

- **Matches the rasteriser:** `gpu::tests::the_gpu_draws_a_known_scene_as_the_rasteriser_does`
  (game-render, run by `./kit check`) draws a known scene both ways at 160×120: lit and prelit-glass
  quads, two placements and fog. It asserts the same triangle count, fewer than 3% of pixels off by
  more than 6, and more than 1000 pixels that are not fog. Run directly on this machine: 2 of 19200
  pixels differ by more than 6.
- **The simulation did not move:** `ride hash 0x8dff4da60bbb95de` from 600 frames of the tape, with
  `--renderer gpu` and with `--renderer software`, and again from the 4-player GPU run.
- **By eye:**
  - The `--shot` at frame 300 from each renderer, side by side: the same picture, except that the GPU
    also draws the far hill, which the rasteriser's 44-cell ground radius leaves out.
  - With `--hud`, the HUD overlay looks pixel-for-pixel the same.
  - The 4-view 1080p shot shows four differently posed riders, all skinned from one shared model.
- `./kit check`: TESTS PASSED, including `crates/openmx/tests/ride.rs`' four-player headless shot,
  which now runs on the GPU.

## Not verified / not 1:1

- **Texture filtering.** Textures are linear-filtered with mip chains; the rasteriser samples nearest.
  This is a choice, by eye.
- **The ground is drawn out to the fog**, culled by chunk, rather than to `GROUND_RADIUS` round the
  rider. This is why the GPU's triangle count is about twice the software one (48k against 24k a frame
  for one view).
- **The ground's light** is interpolated per pixel from the vertex normals, not computed per cell.
  Looked at once, at 640×448 only.
- **The overlay's multiply-and-add is exact** for anything that writes a colour over a pixel, darkens
  it or mixes with it. It is **not** exact for additive HUD pixels that saturate at 255 over a bright
  scene: over white they clip, so the multiply comes out too low. Not seen in the HUD shot, which has
  no additive parts over bright sky.
- **The in-ride pause menu** on the GPU path (`front.menu.screen()` drawn as a full-frame patch) was
  built but never seen: a headless run cannot pause, and keys cannot be sent to the window on this
  desktop. The front-end menus outside a ride go through `present` (a CPU frame blitted onto the
  surface), and a 5 s `--menu` run on :0 opened and drew with no error. Its picture was not looked at,
  because a screenshot of a window cannot be taken here.
- **Draw order.** Blended runs are drawn after all opaque runs within a batch, and are not sorted back
  to front between models. Two overlapping translucent props can mix in the wrong order.
- **The far plane is 100 000.** The rasteriser clips nothing beyond near. Nothing in free06 reaches it.
- **The windowed picture** was only measured, never seen. Only offscreen frames (`--shot`) were
  looked at.
- **Steady 60 with four views** was measured with this machine's GPU while the other slots loaded its
  CPU. No other GPU and no phone was tried.
- **Not in the browser at all.** The wasm build leaves the `gpu` feature off, so web.rs still presents
  a software frame. This is split off as **P47**.
- **The GPU test's skip.** `the_gpu_draws_a_known_scene_as_the_rasteriser_does` prints `skipped` and
  passes where no adapter exists. A `./kit check` pass does not prove it drew (a pass leaves no log).
  It did draw when run directly here.

## Next

P47: the browser through WebGPU/WebGL2.
