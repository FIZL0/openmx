# 1 — gpu in the browser (PART)

Task P48. 2026-10-20.

**Blocked on N4.** The brief assumes the browser's module is N4-part's: instantiated by hand with no
wasm-bindgen glue. wgpu cannot work that way, because its WebGPU and WebGL2 backends are web-sys calls
and need wasm-bindgen's generated JS. N4's second session (`task/N4`, 4a0422f) already moved the
browser build to wasm-bindgen. It took wasm-bindgen-cli 0.2.129 into `$TMPDIR/openmx-cross` and
rewrote `crates/openmx/src/web.rs`, `Cargo.toml`, `tools/web.sh`, `web/worker.js`, `web/index.html`
and `crates/openmx/tests/web.rs`. Those are exactly the files this task wires the GPU into.

The runner could not merge N4 (run.log 19:39: conflicts in Cargo.lock, crates/openmx/Cargo.toml and
plan/), and it is queued for the master. If I did the same switch on main here, it would conflict
with N4 in every one of those files. If I merged task/N4 into this branch, N4's conflicts would come
with mine. So this session did only the renderer half, which is all in game-render, and blocks
`--after N4`.

## What I did

- **`Renderer::request(adapter, format)`, async** (`crates/game-render/src/gpu.rs`). This is the old
  body of `Renderer::new`, with `request_device(..).await` in place of `pollster::block_on`.
  `Renderer::new` is now `pollster::block_on(Renderer::request(..))` and is the same for every
  desktop caller. `new` and `headless` are `cfg(not(wasm32))`, so a wasm32 build makes no pollster
  call. The browser will await `request` from a JS-driven future, through
  wasm-bindgen-futures' `future_to_promise` or an `async` export.
- **game-render's `webgl` feature**: `["gpu", "wgpu/webgl"]`. wgpu's defaults already include
  `webgpu`, so a wasm32 build with `webgl` has both backends. The page then picks between them at
  runtime, with `wgpu::util::new_instance_with_webgpu_detection` or
  `Backends::BROWSER_WEBGPU | Backends::GL`.
- **The shaders against WebGL2**: `gpu::tests::every_shader_translates_for_webgl2`. It parses and
  validates each shader with naga 30 (the same translator wgpu uses), then writes every entry point
  out as GLSL ES 3.00 with `is_webgl`, using the writer flags wgpu-hal's GLES device sets. The
  shaders covered are mesh, sky, place/blit, and the ground twice: bare, and painted with 15 layers
  plus the light map (WebGL2's 16 sampled textures per stage), with every blend shape a pass can
  ask for. A storage buffer in the output fails the test. As a control, a shader reading a
  `var<storage>` palette must fail to translate, and it does. naga is a dev-dependency only, at the
  version already in Cargo.lock.

## What came from opentennis

Nothing in this part. opentennis's `tools/web.sh` is the model for the browser half: wasm-bindgen
`--target web`, one build for WebGL2 and one for WebGPU, with the page picking which to load. That
half was already taken by N4. One thing to note for later is its B121 patch, a WebKit
`getMappedRange` on a lost device. It only matters if `Renderer::read` or a mapped-at-creation
buffer runs in Safari.

## What the original does

Nothing here touches the simulation. Rendering is ours (AGENT.md).

## How it was proved

- `./kit check -- -p game-render --lib -- every_shader_translates_for_webgl2`: passes, including
  the storage-buffer control.
- **game-render with `webgl` for wasm32-unknown-unknown builds.** This was run by hand with the
  scratch toolchain (`RUSTC=$TMPDIR/openmx-cross/rust-1.99.1/bin/rustc cargo build -p game-render
  --lib --release --target wasm32-unknown-unknown --features webgl`, into `target/web`), and it
  builds clean.
- `./kit check -- --workspace --no-fail-fast`: 853 passed. A plain `./kit check` failed twice, both
  times on `netplay::four_peers_and_eight_riders_on_a_bad_network` ("k waited 62 rounds"). That is
  the known load flake (memory: netplay-four-peers-test-flakes). It passed alone, and in the full
  no-fail-fast run. Nothing in this change reaches netplay.

## Not verified / not 1:1

- **Nothing has been drawn in a browser.** There is no WebGPU or WebGL2 frame, and no chromium test.
  Both wait on N4's wasm-bindgen build (see the top of this file).
- **The wasm32 build of the `gpu` feature is not in `./kit check`.** It was run by hand once. The
  gate belongs in `crates/openmx/tests/web.rs` via `tools/web.sh --check` once openmx turns the
  feature on for wasm32, and that file is N4's.
- **The shader check is naga's translation, not a WebGL2 driver.** A real browser's GLSL compiler,
  or ANGLE underneath it, could still refuse something naga writes. Only the chromium run will
  show that.
- **Bind-group and texture limits under WebGL2 are not exercised by the test.** The renderer already
  asks wgpu for `downlevel_webgl2_defaults` on the desktop (P44), so wgpu validates the layouts
  against those limits on every desktop run. But the WebGL2 backend's own limits (for example
  `max_texture_dimension_2d` under `using_resolution`) are only known in a browser.
- **The validator ran with `Capabilities::empty()`.** That is stricter than or equal to wgpu's
  downlevel WebGL2 set, as far as I can tell. I did not trace the exact capabilities wgpu-core
  derives for a WebGL2 adapter.
- **`Renderer::read` and `Renderer::finish` still block on `device.poll(wait)`.** On the web that is
  a no-op, so a read-back there would return before the map completes. The browser path should
  present to the canvas surface and never read back. A test that wants pixels from the page should
  take them from the canvas, not from `read`.

## Next

Once N4 is on main (the run reopens this task):

1. `crates/openmx/Cargo.toml`: give wasm32 `game-render` with `features = ["webgl"]` (the desktop
   keeps `gpu`), plus wasm-bindgen-futures at the version in N4's Cargo.lock.
2. `web.rs`: an `async` wasm-bindgen export (`openmx_gpu_start(canvas: OffscreenCanvas)`) that does
   the following:
   - make the instance with `wgpu::util::new_instance_with_webgpu_detection`;
   - make the surface with `SurfaceTarget::OffscreenCanvas`;
   - `request_adapter(compatible_surface)` then `Renderer::request(&adapter, surface format)`;
   - upload the start line's assets and ground (P44's `main.rs` / `gpuwin.rs` show the order:
     `set_scene`, `set_sky`, `set_ground`, `upload_all`);
   - render a `Scene3` with one `View` and the overlay patches from `overlay::Layer` (compile
     overlay.rs into the lib as ground.rs is).

   The software path stays as the fallback when no adapter is granted.
3. `worker.js`: hand the OffscreenCanvas to the GPU start, and keep the 2D context only for the
   software fallback. A canvas cannot have both a `2d` and a `webgpu`/`webgl2` context, so pick
   before the first `getContext`.
4. Test: chromium over CDP (`crates/openmx/tests/web/page.mjs`, which is N4's) loads the page with
   the disc in OPFS, waits for `started`, and reads the canvas back with
   `toDataURL`/`drawImage`. It asserts the frame is not all black and not the software frame
   (the page reports which backend it got). Run it with `--enable-unsafe-webgpu` and without, so
   that both WebGPU and WebGL2 are covered (memory: headless-chromium-drives-the-web-page).
