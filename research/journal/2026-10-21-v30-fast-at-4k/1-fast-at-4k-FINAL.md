# 1 — fast at 4k (FINAL)

Task V30. 2026-10-21.

## What I did

**Measured first.** `openmx --headless --size 3840x2160 --players N --seconds 8 --fps --no-sound`, release
build, AMD Radeon 8060S over Vulkan. The startup line says `renderer: gpu`, so it had not fallen back to
software. The time was all in P47's `draw` stage, the CPU side of the overlay:

| 3840×2160 | before | after |
|---|---|---|
| 1 player, original HUD | 4.9 fps, draw 195.76 ms, present 3.13 ms | 59.7 fps, draw 0.68 ms, present 1.47 ms |
| 4 players, original HUD | 3.1 fps, draw 301.27 ms, present 3.53 ms | 59.7 fps, draw 0.69 ms, present 1.77 ms |
| 1 player, `--overlay none` | 59.7 fps, draw 0.74 ms | (unchanged) |
| 1 player, `--hud` (debug HUD) | 37.6 fps, draw 24.33 ms | 59.5 fps, draw 2.44 ms |
| 4 players, `--hud` | 35.7 fps, draw 24.39 ms | 59.5 fps, draw 3.11 ms |
| 1 player, the pause (`--menu --screen pause`) | 3.1 fps, draw 302.48 ms | 59.8 fps, draw 0.18 ms |
| 4 players, the pause | — | 59.8 fps, draw 0.41 ms |
| front end in a window, the splash (`--menu --timings`) | 82.87 ms a frame | 0.33 ms |
| front end, main / settings | — | 0.48 / 0.75 ms |
| windowed ride, 1 / 4 players, render scale auto | — | 59.7 / 59.8 fps, draw 0.27 / 0.24 ms, present 0.13 / 0.08 ms |

The 60 fps ceiling is the ride loop's own 60 Hz pacing (main.rs, "Pace to 60 Hz"). It is not the
renderer. A whole frame now costs about 2 ms, and the new test draws a 4K frame of a known scene and a HUD
in 1.09 ms on the GPU. Drawing above 60 Hz with the sim at 30 Hz is P52's job (the fixed-rate loop, on
its own branch). I did not touch the pacing, so the two merge cleanly.

**Where the time went:**

1. **The original's HUD** (V26) was drawn by the CPU at window size twice (over black and over white),
   then picked apart into a full-frame `Patch` and uploaded every frame. At 4K that is 2 × 25 MB to draw,
   50 MB to scan, and up to 25 MB to upload.
2. **The pause** did the same, over the whole frame.
3. **The debug HUD**'s `Layer` cleared both 4K frames and both depth buffers every frame.
4. **`Patch::of`** scanned every byte with an early-exit `all`, which does not vectorise.
5. **The front end** painted every screen on the CPU at window size every frame. The painter's `fill`
   was a per-pixel blend. Its blit cache was FIFO and thrashed on the Tracks screen.

**What changed:**

- **GPU sprites** (`crates/game-render/src/gpu/sprite.rs`): quads with an image (or none), a texel
  clamp, a tint and a shape. The shapes are plain, disc, capsule and needle (the HUD's dials and bars),
  plus glow (additive, for the menus' highlight). The images are uploaded once with
  `Renderer::set_images`, and one instanced draw is made per run of the same image.
  - `overlay.rs` draws the original's HUD as sprites when the GPU is on, from the same layout code as the
    software path (`Pen` with `Ink::Frame` / `Ink::Sprites`).
  - `menu.rs`'s `Painter` gets a sprite mode (`Art::sprites`). Every screen's `fill` and `blit` become
    sprites, so the front end and the in-ride pause cost what a small window does. The pause's pictures
    are registered after the HUD's (the HUD's indices start at 0).
  - The software renderer and `--shot` keep the CPU path unchanged.
- **Render scale** (`crates/game-render/src/gpu/scale.rs`, `Renderer::render_scale`): the views are
  drawn into a smaller texture, which is stretched bilinearly over the frame. The overlay and sprites are
  then laid on at full size. Each view's rectangle is scaled and rounded so the split-screen edges still
  meet.
  - `--render-scale auto|F` selects it. Auto is the default: `AutoScale` averages 30 frames' draw time
    against 80% of the display's refresh (`GpuWindow::refresh`, from winit; 60 Hz where unknown). It
    lowers the scale in sixteenths, down to 0.5, and raises it again when there is room.
  - At 4K on this machine auto stays at 1.0, because the frames are far under budget.
- **Present:** the GPU window already preferred Mailbox, then Immediate, then Fifo, with frame latency 1
  (P44). V30 adds no blocking present. The front end on the GPU now draws sprites straight into the
  surface. The X-window path re-sends the last frame (`present_again`, `blit_again`) when the menu's look
  has not changed (`Menu::look`) instead of repainting it.
- **The debug HUD** (still CPU patches): `Layer` puts back only the rectangle the last drawing touched.
  `Patch::of` folds each row with OR/AND (which vectorises) and only searches columns outside the
  rectangle found so far. The renderer skips the overlay upload when the patches are unchanged
  (`Overlay::kept`).
- **The menu painter's CPU path**, used by the X window, the software renderer and `--shot`: `fill` uses
  a per-channel lookup row, plus a row-doubling copy when the fill is opaque (bit-identical to before).
  The blit cache is now LRU.

## What the original does

Nothing ported. This is the shell's renderer. The HUD's and the menus' layout and pictures are unchanged
(V26 and P36 code), only drawn by the GPU instead.

## How it was proved

- `crates/game-render/src/gpu.rs` tests:
  - `a_4k_frame_fits_in_a_120hz_frame` renders a 3840×2160 headless frame of `known_scene` with a HUD of
    every sprite kind. It times 30 frames after 3 warm-up frames, each waited on, and asserts under 8.33
    ms. It measured 1.09 ms.
  - `a_frame_at_half_scale_looks_like_the_full_frame` compares render scale 0.5 with 1.0 at 640×360. It
    asserts a mean max-channel difference under 1.0 (measured 0.37) and under 1% of pixels off by more
    than 40 (measured 0.238%, along the scene's edges). It also asserts that over 100 pixels changed, so
    the stretch really ran (measured 1.2%).
  - Both skip with a message where there is no GPU.
  - `an_overlay_finds_the_rectangle_the_plain_search_finds` checks the new `Patch::of` against a plain
    search over 500 random layers.
  - `every_shader_translates_for_webgl2` now also covers the sprite and upscale shaders.
- `crates/openmx/tests/overlay.rs`, `the_gpu_draws_the_hud_as_the_cpu_does`: the original's HUD as GPU
  sprites against the CPU drawing, at 1280×720 with 1 and 4 views. Mean difference 0.016 / 0.027, and
  0.0085% / 0.0029% of pixels off by more than 48. Asserted under 0.5 and 0.1%.
- `crates/openmx/tests/menu.rs`, `the_gpu_draws_every_screen_as_the_cpu_does`: every `Screen::ALL` at
  1280×896 and 1600×720. Means 0.01–0.65, at most 0.0625% of pixels off (Tracks). Asserted under 1.0 and
  0.2%.
- **Looked right beside a screenshot:**
  - `--shot` of the HUD (1 and 4 players) and the in-ride pause (2 players) with `--renderer gpu` beside
    `--renderer software`. For the pause, ImageMagick MAE is 0.6%, most of it the 3D's own rasteriser
    differences.
  - `--render-scale 0.5` beside 1.0.
  - The test frame (`GPU_TEST_PNG`).
- `./kit check`: TESTS PASSED.

## Not verified / not 1:1

- **No frame rate above 60 fps was observed.** The ride loop paces itself at 60 Hz, and the monitor
  here is 1920×1080@60 Hz. The 120/144 Hz claim rests on the frame times (about 2 ms a frame, 1.09 ms for
  the GPU's 4K frame), not on a measured rate. Uncapping the loop is P52's.
- **Sprites are not pixel-exact** against the CPU drawing. Glyph edges and dial anti-aliasing part by a
  few levels on under 0.1% of pixels (numbers above).
- **The render scale has no settings row or settings.txt key**, only `--render-scale`. A row next to
  V32's graphics preset is filed as V34.
- **Auto scale only runs in a GPU window.** `--headless` draws at the scale given, or 1.0 under auto,
  and an explicit `--render-scale` disables auto. The refresh falls back to 60 Hz where winit does not report
  it. On this machine it reported 60.
- **Auto never scaled down here**, because the frames were far under budget. Its hysteresis is covered by
  `scale.rs`'s unit tests, not by a slow machine.
- **The debug HUD (`--hud`) is still drawn on the CPU as patches.** It is faster (24 → 2.4–3.1 ms at
  4K) but still costs a few ms at 4K.
- **The X-window (non-GPU) front end still sends a CPU frame** when the look changes. It sends nothing
  new when the look is unchanged.
- **Present was not changed.** Mailbox was already P44's. Nothing in the loop waits on the GPU besides
  the surface acquire, which is reported as `present` (0.08–1.8 ms).

## Next

none — V34 (render scale setting) and P52 (draw above 60 Hz) carry what is left.
