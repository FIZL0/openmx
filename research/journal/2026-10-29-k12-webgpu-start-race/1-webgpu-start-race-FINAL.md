# 1 — webgpu start race (FINAL)

Task K12. 2026-10-29.

## What I did

P38's gate saw `the_browser_rides_on_the_gpu` red with gpu.mjs (webgpu) reporting
`the page did not start drawing: {"disc":"no","state":"failed"}`. The gate log was gone by the time I looked, so
the page's error text is lost, and I reasoned from the state:

- `state: failed` is set only when the worker posts `{ error }` and `fallBack` declines. It declines because
  `drawing` is still `''`: no `gpu` attribute, so the module never reached `web_gpu::finish`'s `using()`. A WebGPU
  adapter or surface failure does not end like this. `prepare` falls back to WebGL2 and then to software, and both
  of those say `gpu`. So the error came before the renderer, in the worker's `start`. With a picked File (gpu.mjs
  turns OPFS off), `openDisc` cannot fail and `loadFiles` swallows its own errors. That leaves `game()`, the
  dynamic import of the glue and its wasm fetch.
- That fetch has a known saboteur here (K7, memory "chromium-brings-its-own-extensions"). This machine's chromium
  force-installs a policy extension from the web store into every fresh profile a few seconds after start, and an
  install mid-load aborted the worker's module fetch in K7. page.mjs got `--disable-extensions` for it; gpu.mjs was
  still on K9's list. Under load ~50 the page's load stretches into that window. Every gpu.mjs ride is a fresh
  profile, which explains why the third ride (the phone's) can fail after two good ones.

The changes:

- `crates/openmx/tests/web/gpu.mjs`: chromium starts with page.mjs's
  `--disable-extensions --disable-component-extensions-with-background-pages --disable-background-networking`. Its
  failure message now adds the page's `#status` text, which holds the worker's error, so a future red says why.
- `web/worker.js` `game()`: a module load that fails (import or wasm fetch) is tried twice more, a second apart,
  before the page hears of it. A failed `import()` stays failed for its address, so the retry imports the glue as
  `openmx_web.js?retry=N`. The glue keeps its wasm only once instantiated, so its own fetch just runs again. The
  service worker matches the pair by pathname, so a `?retry=` address is still served as the module. This also
  helps a real player on a network that drops a connection.

touch.mjs, chromium.mjs and shots.mjs still lack the flags. They stay K9's.

## What the original does

Nothing: test and page tooling, no game behaviour.

## How it was proved

- Four ordinary phone-webgpu rides of gpu.mjs (load ~35) all passed before the change. Three more with the
  scratch server holding pkg/ for 3, 6 and 9 s also passed. **The red was not reproduced.** The extension does
  install into each scratch profile (`Default/Extensions/jlnfnnolkbjieggibinobhkjdfbpcohn`).
- The retry, against a scratch copy of gpu.mjs whose server cuts off the first response mid-body: once for
  `openmx_web.js`, the import fails with "Failed to fetch dynamically imported module" and is asked again. Once for
  `openmx_web_bg.wasm`, it fails with "Failed to fetch" and is asked again. Both runs then rode on webgpu at 54–60
  fps with the screenshot lit. An instrumented worker and page showed that neither the worker's
  `unhandledrejection` nor the page's `{ error }` fired. The "Uncaught (in promise)" CDP prints in those runs comes
  before the cut and is the page's own.
- `./kit check -- -p openmx --test web`: 7 passed.

## Not verified / not 1:1

- The cause is inferred, not seen: the failing run's log (with the page's error text) was overwritten before K12
  started, and the red never reproduced here, neither at load ~35 nor with the module's fetch delayed into the
  extension's install window. If it was something else that fails before the renderer (e.g. the wasm's memory
  refused under memory pressure), the retry still covers it, but the extension flags would not have been the fix.
  The next red now prints the worker's error.
- Not run at the gate's load ~50 with three chromiums at once: there is no way to make that load on demand without
  taking the machine from the other slots.

## Next

Nothing for K12. K9 still owes the same flags to touch.mjs, chromium.mjs and shots.mjs.
