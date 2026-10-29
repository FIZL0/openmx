# P48 — gpu in the browser

2026-10-20. Task: `./kit plan P48`.

Putting the wgpu renderer into the browser's build. The renderer half that stands alone is done: the
device is now requested by an async `Renderer::request` that a browser can drive, with no pollster on
wasm32. A test writes every shader out as WebGL2's GLSL ES 3.00 and passes. game-render builds for
wasm32 with WebGPU and WebGL2. The browser half (`web.rs`, `worker.js`, `tools/web.sh`, the chromium
test) is blocked on N4. N4 replaced N4-part's hand-instantiated module with wasm-bindgen in the same
four files, and the runner could not merge it (conflicts). That work is queued for the human. Details
in `1-gpu-in-the-browser-PART.md`.

2026-10-28, part 2 (`2-gpu-in-the-browser-FINAL.md`): N4 and P196 had landed. The page now draws on
WebGPU, or on WebGL2 (`?webgl2`, or when WebGPU is missing or fails before its first frame), with
software as the last fallback. It does so through the shell's own `GpuWindow` (`web_gpu.rs`) on the
worker's OffscreenCanvas. web.sh runs wasm-opt. Headless chromium on this machine's GPU rides at
60 fps on both APIs (`the_browser_rides_on_the_gpu`, in the gate). Fixed along the way, in
game-render: a 24-byte sprite uniform, `Instant` on wasm32, and the sky's square six-layer array,
which wgpu's GL backend took for a cube map (black sky).
