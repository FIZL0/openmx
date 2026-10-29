# 2 — gpu in the browser: the page draws on WebGPU or WebGL2 (FINAL)

Task P48. 2026-10-28. This part follows part 1's "Next". By now N4 and P196 had landed. The browser
runs the desktop shell (`main.rs`, mounted in `web.rs`), so the GPU went into the shell's own
`GpuWindow` rather than into a separate start-line export.

## What I did

- **`crates/openmx/src/web_gpu.rs` is now a real `GpuWindow`** (it was a stub that never opened).
  `web.rs` swaps it in for `gpuwin` on wasm32. `prepare(canvas, webgl2)` is awaited from the async
  export `openmx_run`, so the adapter and device requests are driven by the worker's promise and
  pollster is never used. It works as follows:
  - **WebGPU** is tried only when `wgpu::util::is_browser_webgpu_supported()` says so.
    `Instance::new` with `BROWSER_WEBGPU` quietly falls back to wgpu-core when there is no
    `navigator.gpu`, so the check has to come first.
  - **WebGL2** is used otherwise, or with `?webgl2`. The canvas's `webgl2` context is taken first,
    with `alpha: false, antialias: false`, because a canvas keeps its first context, and wgpu-hal
    would otherwise make an alpha one that the page composites.
  - Either way the window draws on the worker's OffscreenCanvas. Surface configure sets the canvas
    size, which comes from the page (`web_host::view`). The surface is Fifo, latency 1, and is
    presented at the display's rate through the rAF wait the shell already does
    (`pace::shown`, then `web_host::next_frame`).
  - The HUD is drawn as overlay patches, as on the desktop. The draw is the same `Renderer::render`.
- **Fallback ladder (`web/index.html`)**: auto, then WebGPU, then WebGL2, then software. Four
  things fall back, all before the first GPU frame (`{drawn}`):
  - a module error before that frame,
  - a WebGPU uncaptured validation error (`on_uncaptured_error` into a static, checked on each draw),
  - a panic (the worker's `error` / `unhandledrejection`),
  - a failed `prepare`.

  `fallBack` reloads the page with `webgl2` and then `software` added to the query. The worker tells
  the page which API draws (`{gpu}`) and when the first frame is up (`{drawn}`). `{gpu}` is sent
  *before* `Renderer::request`, so a panic inside the request still knows what to fall back from.
- **`tools/web.sh` runs wasm-opt** with opentennis's flags: `-O2 --strip-debug` plus the six
  feature flags (bulk-memory, nontrapping-float-to-int, sign-ext, mutable-globals,
  reference-types, multivalue). It uses wasm-opt from PATH, or binaryen version_133 downloaded once
  under the fetch lock. `WEB_FAST=1` skips it. `--check` exits before it, because the gate's module
  is only read and run. The module goes from 16.5 MB to 12.0 MB.
- **`?args=`**: the page's query can pass shell arguments (`main.rs`'s `argv()`). The test uses
  `?args=--ride` to start on the start line.
- **Three renderer bugs that only a browser showed**, all in game-render:
  1. **The sprites' uniform was 24 bytes.** A WebGL2 adapter lacks
     `BUFFER_BINDINGS_NOT_16_BYTE_ALIGNED`, so wgpu refused the pipeline. The WGSL `Place` now has
     three pad floats. `every_shader_translates_for_webgl2` now checks every uniform's size and
     includes the sprite shader, with a 24-byte control that must be refused.
  2. **`std::time::Instant` panics on wasm32** ("time not implemented"). gpu.rs takes `web_time`'s
     on wasm32.
  3. **The sky was black on WebGL2.** wgpu-hal's GL backend picks a texture's target from its
     descriptor (`get_info_from_desc`): a square D2 texture whose layers are a multiple of 6 becomes
     `TEXTURE_CUBE_MAP`, and a 1-layer one becomes `TEXTURE_2D`. The sky's six square faces were
     therefore a cube, and sampling them through `texture_2d_array` gave black. I found it on the
     desktop with `WGPU_BACKEND=gl`, which shows the same black sky, so the browser was not needed
     to debug it. Constant-colour probes in the sky shader showed the pass drew, the fog was right,
     `blend` was (0,1,1), and only the sample was black. The new `array_layers` helper adds a spare
     layer to such counts and is used for the sky, the ground pages and `array_shape`.
     `array_textures_stay_arrays_on_gl` covers it.

## Decision: one pkg with both backends, not two

opentennis builds two pkgs, one for WebGPU and one for WebGL2, and the page loads one of them. wgpu
30 supports both backends in one build (`webgpu` is a default feature, plus `wgpu/webgl`), and the
choice is made at runtime by which instance is created. One pkg halves the gate's wasm32 build time
and keeps one module in the service worker's cache. What it costs is module size (both backends are
linked in), which wasm-opt partly wins back.

## What came from opentennis

- The wasm-opt step and its exact flags.
- The idea of a fallback ladder that drops to the next API on a failed first frame or a validation
  error.
- The two-pkg layout, which I did not take (see above).
- B121's WebKit `getMappedRange` patch is not applied. Nothing on the browser path maps a buffer.

## How it was proved

- `crates/openmx/tests/web.rs::the_browser_rides_on_the_gpu` is new and in the gate. It runs
  `tests/web/gpu.mjs`, which serves web/ with the `tools/web.sh --check` module as its pkg/. The
  page is opened in headless chromium on the machine's GPU (`--enable-unsafe-webgpu
  --enable-features=Vulkan --use-angle=vulkan --ignore-gpu-blocklist`) with `?args=--ride`, once as
  it comes and once with `?webgl2`. It rides 10 s each and asserts four things:
  - the API asked for draws, with no software fallback;
  - a first frame was drawn;
  - more than half the pixels are not black;
  - the median pictures a second (after 2 s) is ≥ 30.

  Measured through `./kit check -- -p openmx --test web the_browser_rides` (2026-10-28):
  - `webgpu: 60 fps (median of [59.7, 59.7, 60.0, 60.0, 60.0, 60.0, 60.3, 60.3]), 738279 of 738560 pixels lit`
  - `webgl2: 60 fps (median of [59.6, 59.7, 60.0, 60.0, 60.0, 60.0, 60.0, 60.3]), 738282 of 738560 pixels lit`

  With the wasm-opt'd web/pkg, by hand: WebGL2 ran at 55.8–60.8 fps and WebGPU at 58.7–60.7 fps
  over 12 s.
- The wasm32 build with the gpu feature is in the gate through `the_shell_builds_for_the_browser`
  (`tools/web.sh --check`). Its import and export lists were updated (`gpu`, `drawn`).
- Pictures checked by eye: WebGPU and hardware WebGL2 at the start line match each other, with the
  sky's clouds, the HUD's score pill, the map and the gear. After the sky fix, desktop
  `WGPU_BACKEND=gl` and Vulkan shots match too.
- The fallback was seen working by hand during development. SwiftShader's WebGL2 (default headless
  chromium, no GPU flags) drew the ride. `?software` drew the software picture.
- `./kit check -- -p game-render --lib gpu::`: 20 passed.

## Not verified / not 1:1

- **Only headless chromium on this machine.** WebGPU was only reached with
  `--enable-unsafe-webgpu` on Linux/Vulkan. Firefox, Safari (macOS and iOS), and WebGL2 inside a
  worker on iOS Safari have not been tried.
- **fps measured only at 60 Hz, headless.** Presentation follows rAF, so a 120 Hz display should
  draw at 120, but that is unmeasured. The window reports `refresh: None`, so the shell's pacer
  assumes its default.
- **Default headless chromium (SwiftShader WebGL2) rides at about 1.9 fps.** That is chromium
  without a GPU, and the gate passes the GPU flags. A real browser on a machine with no GPU would
  be that slow, and nothing tells the user why.
- **The fallback ladder has no automated test.** It was exercised by hand (a sprite-uniform panic
  fell back to software before the fix). The gate only covers the two GPU APIs drawing.
- **A fallback reload re-asks for the disc when OPFS is refused** (a private window), because the
  picked file does not survive a reload.
- **Render scale and resize on the web are not tested.** The size is followed on each `poll`, and
  reconfigure is on a size change, but no test resizes the page.
- **B121 (WebKit `getMappedRange` on a lost device) is not patched.** Nothing on the browser path
  maps a buffer, so it should not arise, but this is unproven in Safari.
- **The spare array layers cost memory on every backend.** That is one extra sky face per sky, and
  one extra page where a square page array's count is a multiple of six. They were not measured.
- **wasm-opt's effect on speed was not measured separately.** Both builds hit the 60 fps cap. Only
  the size change (16.5 to 12.0 MB) was measured.
