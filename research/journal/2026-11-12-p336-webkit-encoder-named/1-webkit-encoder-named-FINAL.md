# 1 — webkit encoder named (FINAL)

Task P336. 2026-11-12.

## What I did

The user's iPhone (iOS 18.7, Safari 27.0.1, WebGPU) ended every ride on its first frame with
`gpu: encoder state is invalid`, twice: once in the frame's scope (frame 414) and once as an uncaptured
error. Neither the console nor the frame's error scope said which command broke the encoder: WebKit
reports a pass's failing command only as the encoder being invalid when it is finished. Chrome does not
reproduce it at all (below), so the cause cannot be found here. What I did instead:

- **Each stage of a frame is an encoder and an error scope of its own on a browser's WebGPU**
  (`Renderer::stage`, game-render `gpu.rs`; switched on by the new `staged` field, set when the renderer
  has a `Watcher` and the backend is `BrowserWebGpu`, so desktop, WebGL2 and the phones' native apps
  still encode a frame in one encoder). The stages are the sun maps, the ground meshes, the frame's
  first timestamp, the views (low-scale path) and then the picture and overlay, or the views and overlay
  together, and the frame's last timestamp. Each is submitted inside a scope named after it. A broken
  encoder is now reported as e.g. `gpu: validation error in the ground meshes, frame 200: …`, and the
  ride's error says `(in the ground meshes, frame 200)`. The frame number comes from a `FRAME` static
  that `draw_frame` sets around the draw closure. Measured cost in Chrome: the sx01 race still shows
  60 fps on a phone-sized page.
- The two encoders the renderer makes outside a frame are watched too: the ground meshes' far target
  (`GpuMeshes::new` draws it in its own encoder when the ground is set, an "unwatched call" until now)
  and the ground's growing vertex page. I suspect the phone's second, uncaptured
  `encoder state is invalid` was the far target's encoder, because the frame's ground-meshes stage draws
  the same passes into the near and mid targets on the first ride frame. That is a guess, not a finding.
- `report()` appends the error's source chain where it adds words (`said`), skipping the browser's
  `<WebGPU Error>` placeholder.
- **A WebGPU validation error in a ride now restarts the page on WebGL2.** Before, the error went back to
  the menu as "Could not ride sx01: …", still on WebGPU, so the next ride broke the same way. Now
  `web_gpu.rs` starts it with `RIDE` ("webgpu failed in a ride: "); the menu's ride and the online ride
  in `main.rs` leave the shell with it on wasm; and `web/index.html`'s `fallBack` reloads on `?webgl2`
  when it sees it, as it already did for P257's front-end errors. The page log keeps the WebGPU run
  across the restart (P257b), so the stage's name reaches a saved phone log.
- `tests/web/gpu.mjs` has three new switches. `OPENMX_GPU_LIMITS=iphone` caps every `GPUSupportedLimits`
  getter at the iPhone's logged values. `OPENMX_GPU_BREAK=label,frames` breaks the first encoder with
  that label after that many frames: it pops a debug group that was never pushed, so the encoder is
  invalid at finish, as WebKit finds one. `OPENMX_GPU_FRAMES` watches until that many pictures have
  been shown.
- New test `web.rs::a_phone_race_on_sx01_rides_on_webgpu_and_a_broken_stage_is_named`.

opentennis: its web fallback (B244/B378) is already what index.html's `fallBack` copies. It has no
per-pass error scopes, so nothing was taken from it this time.

## Repro in headless chromium

- `--ride --track sx01` on a phone page (844×390 at 3:1, touch) under the iPhone's limits: about 2100
  frames on WebGPU, no validation error.
- `--ride --track sx01 --race --racers 8`, same page: about 2400 frames, no validation error.
- The limits make no difference in Chrome. The device gets what `request_watched` asks for
  (`downlevel_webgl2_defaults` raised to the adapter's 2D size and array layers), and the iPhone's
  adapter is at least that on every limit. So the phone's failure is a WebKit validation rule that
  Chrome's Dawn does not have, not a limit.

## How it was proved

- `web.rs::a_phone_race_on_sx01_rides_on_webgpu_and_a_broken_stage_is_named`, first run: the race on a
  phone page under the iPhone's limits must show at least 1400 pictures on WebGPU (it runs to 1500)
  with no `validation error` in the worker console.
- Same test, second run: with the `the ground meshes` encoder broken at frame 120, the console must
  name `the ground meshes, frame N`, the shell must leave with `webgpu failed in a ride: gpu: `, and the
  page must start again on WebGL2 and draw the race lit.
- By hand: the same break printed
  `gpu: validation error in the ground meshes, frame 200: PopDebugGroup called when no debug groups are currently pushed.`,
  and the restarted page then rode on webgl2 at 60 fps.

## Not verified / not 1:1

- **The actual cause on the iPhone is not found or fixed.** No WebKit WebGPU runs on this machine: WebKit's
  WebGPU is Metal-only, and Chromium/Dawn accepts every call the ride makes. Finding the call needs a new
  log from the user's phone on a build with this change. The log will name the stage, and the ride will
  then go on in WebGL2. Filed as a follow-up.
- Even staged, WebKit may still give only "encoder state is invalid" inside the named stage. Within a
  pass (e.g. "the views and overlay", which holds every view and HUD draw), the failing draw is not
  narrowed down further.
- The second, uncaptured error being the ground meshes' far-target encoder is a guess (see above).
- The test's race never reaches the shootoff throw (P332). The rider sits on the start with no
  throttle, so the race start and the gate are covered, not the throw. Driving a rider round sx01 in the
  page needs a tape or touch script that the gpu.mjs driver does not have.
- The test's frame count is the worker's pictures-a-second summed, not an exact frame counter.
- On WebGPU the staged frame is several submits instead of one. Writes that `queue.write_buffer` makes
  while a later stage is being encoded now land after the earlier stages were submitted. I checked each
  stage: no earlier stage reads what a later one writes (the ground meshes' rects, the upscale's place).
  No test catches a regression there, apart from the frame looking right in Chrome.

## Next

P336a: read the phone log the new build saves (Save log after a ride restarts on WebGL2), and fix the
call in the stage it names.
