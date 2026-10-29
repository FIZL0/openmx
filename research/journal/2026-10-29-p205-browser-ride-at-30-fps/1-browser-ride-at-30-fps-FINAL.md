# 1 — browser ride at 30 fps (FINAL)

Task P205. 2026-10-29.

## What I did

Followed opentennis's B328 method: profile a ride in headless chromium, then fix what the profile shows, not the
list of guesses. Tools: the test's `crates/openmx/tests/web/gpu.mjs` with a worker CPU profile (CDP `Profiler` on
the worker's session, 5 s at 200 µs) and `--timings`, which now reaches the page's console (below).

**Baseline** (main at b94ce1b5, 1280×577, WebGPU on this machine's GPU): the page said 60 fps, but that number
counted `frame()` calls, not pictures. The ride waited for the page's next animation frame after every tick as
well as after every picture, so a 60 Hz page drew 30 pictures a second, and a slower page drew fewer still. Per drawn frame:
sim ~14 ms, the rAF wait ~13 ms, hud ~3.5 ms, present ~2 ms. In the worker profile the worker was 55 % idle, and
the sim was 72 % of the busy time. One call was 24 % of all samples (55 % of busy time): `Scene::of` →
`flight_search::wooded_of`, which refiled every prop and plant group into a fresh quadtree. It does that three
times a tick (the wheels' search, the sweep back, the rider's scenery contact), each time from a new
`bike_search()`. Native headless takes ~6.6 ms a tick for the same sim; wasm is ~2× that.

Fixes:

1. **A tick is not a picture** (`main.rs`, the ride loop): the loop awaits `pace::shown()` only when the last
   time round drew. After a tick that drew nothing, it goes straight on. → ~48 fps.
2. **The broad phase's filed tree is kept** (`flight_search.rs`, `Filing`): `broad_build::key` reads only
   the tree's box. So the filled tree is a pure function of (box, the ordered items (object, centre, half)).
   `world_of`/`wooded_of` now collect those items, and `Filing::file` stamps them as u32 bits.
   - On a stamp hit, it clones a kept tree (the last 8, per thread).
   - On a miss, it files the items as before and keeps the result.
   - The output is the same tree, bit for bit. → sim 2.4 ms per drawn frame (was 8.5–14), worker 80 % idle,
     **60 fps steady**.
3. `--timings` in the browser: a `timing()` helper prints to the page's console through `host::say` on wasm32
   (standard output goes nowhere there).

The rest of the brief:

- **The 640×448 cap** (`web_host::PIXELS`): the GPU never had it. P48's `GpuWindow` draws at the canvas's whole
  device-pixel size. Measured at a phone viewport (844×390 at DPR 3 = 2532×1170, touch emulated): WebGPU 60 fps;
  WebGL2 60 fps after its first second. The cap stays only on the software fallback: a single-thread rasteriser at
  a window's pixels draws ~1 picture a second. Its doc now says so.
  - The gate's `the_browser_rides_on_the_gpu` gained a third case, a phone's page on WebGPU. It asserts the
    first frame is 2532×1170 and the median is ≥ 30 fps.
- **Phone defaults** (B351/P53/B413, "low shadows by default on phones"):
  - `index.html` says `phone` when the primary pointer is coarse (or `?phone`).
  - `worker.js` passes it to the new `openmx_phone` export before `openmx_run`.
  - `menu::Settings::default` then takes the original's shadows instead of ours (`shadows_default`). `shadows_text`
    compares against that default, so a phone that picks Better keeps it.
  - A console probe (removed after use) showed `phone true` with `?phone` and `false` without.
  - Render scale was already Auto (V30 picks the scale from frame time), so it needed no phone default.
- **sw.js caching the wasm** (B470):
  - The glue and the wasm are cached per build, named by `version.txt`, and served from that cache first.
  - Once a build's pair is whole, older builds' caches go.
  - A build without a clean name (a dirty local build, `+`, or no version.txt) stays network-first as before.
  - Offline, a whole build's pair comes before stray copies, so the glue and the wasm stay one build's.
  - Checked in headless chromium against a counting server (scratch `swtest.mjs`):
    - first visit: 2 fetches; a repeat: 0
    - a new version.txt: 2 fetches, and the old cache is deleted
    - a `+` build: fetched every time
    - server down: the last whole pair is served.
- **A hidden tab** (B260/B299): already handled. `pace::Pacer::owed` drops any backlog past `CATCH_UP` (0.5 s),
  so a tab coming back runs the next tick now, not a burst of the ticks it missed. While hidden, the worker's
  rAF stops and the loop parks in `shown()`. No change.
- **B342–B345 (merge meshes, batch, re-uploads, culling)**: not done, because the profile does not ask for them.
  After the two fixes the worker is ~80 % idle at 60 fps on the GPU path. GPU submit/present (the "hud" stage,
  ~5.7 ms) is mostly waiting on the GPU, not CPU.

## What the original does

Nothing here is the original's: pacing, caching and the broad phase's memo are ours. The memo changes no result
(the tree is the one filing would build).

## How it was proved

- `crates/game-sim/tests/flight_candidates.rs`, `a_kept_tree_is_the_tree_filed_afresh` (free06's props), passes
  and does not skip. It checks three things:
  - a kept tree equals the tree filed afresh on a new thread (an empty cache)
  - the whole world equals it too
  - after one prop moves 1 ft, the kept answer is the fresh one.
- The full gate's bit-exact replays run the sim through the memo.
- `crates/openmx/tests/web.rs`, `the_browser_rides_on_the_gpu`: WebGPU, WebGL2, and the phone case, each ≥ 30 fps.
  It passed under a busy machine (load ~50, where the phone case gave 41–51 fps by hand).
- The fps numbers above are from `gpu.mjs`-style runs on this machine.

## Not verified / not 1:1

- **No real phone was measured.** The phone numbers are a phone-sized viewport on this desktop's GPU and CPU.
  CDP's `Emulation.setCPUThrottlingRate` is refused for workers ("only supported for pages"), so a phone's CPU
  could not be emulated. The ride's sim is ~2.4 ms per drawn frame here; a phone at 4–6× slower is still within a
  33 ms tick, but that is an estimate, not a measurement.
- **Safari and Firefox untried.** Only headless chromium was used, on Vulkan.
- **The shadows default was not seen in a picture.** The two shots at the start line look alike at that sun angle.
  The console probe proves only that the flag reaches `Settings::default`, and that Shadows::Original is the
  setting's own path.
- **Hidden tab not exercised in a browser.** The claim rests on `Pacer::owed`'s rule, not on a test that hid a
  page.
- **The memo's memory is unmeasured.** It holds 8 trees per thread, each a clone of a quadtree over a world's props
  and plant groups.
- **sw.js on a real deploy is untried.** It was checked only against a local server. The fetch of version.txt
  races a deploy that swaps the pair in between, which could cache a mismatched pair under one name until the
  next build. GitHub Pages swaps a whole site at once, so this was not guarded.
- **No Phone preset row.** The brief asks for a Phone preset that turns every speed-up on. Phones get low shadows
  by default, but there is no single Settings row for it. Filed as P215.

## Next

Nothing for this task. The follow-up (a Phone preset row, and a real phone measured) is P215.
