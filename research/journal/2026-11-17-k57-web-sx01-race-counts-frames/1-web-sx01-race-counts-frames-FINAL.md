# 1 — web sx01 race counts frames (FINAL)

Task K57. 2026-11-17.

## What I did

- The red in MD179's merge gate (gates/27e629a8…failed) was `a_phone_race_on_sx01_rides_on_webgpu_and_a_broken_stage_is_named`:
  "only 682.7 pictures shown", the page drawing 2–30 fps under the other slots' load.
- Cause: `tests/web/gpu.mjs` with OPENMX_GPU_FRAMES watched the page for **at most 120 s of wall clock**, summing the
  page's per-second fps samples. A page slowed below 12.5 fps could never reach 1500 in that window, so the test raced
  the machine's load, not anything in the shell.
- Fix: with FRAMES, gpu.mjs now serves worker.js behind `frames.mjs`'s WORKER counter (as gamepad.mjs does, K39) and
  watches until the worker has drawn FRAMES frames since the watch began. The only bound left is progress: a minute
  with no new frame ends the watch (a hung page still fails, with its count). It prints `shown`, the exact count.
- web.rs asserts `shown >= 1500` (was a fps-sample sum ≥ 1400, which also lost or doubled frames when a sample was
  late).
- Nothing taken from ../opentennis: its web tests have no frame-count watch (frames.mjs here is already the shared
  pattern).

## What the original does

Not applicable: a test-harness change, no game behaviour.

## How it was proved

- Alone: `./kit check -- -p openmx --test web a_phone_race_on_sx01` passes.
- A/B under a deterministic slowdown (SIGSTOP/SIGCONT of the driver's chromium tree, 60 ms of every 100 ms; the
  node driver itself left running, as the gate's wall clock is): HEAD's gpu.mjs stopped at 120 s with 346 pictures
  (the gate's red); the new one counted 1501 frames in 400 s (its fps samples summed to 1498, so the count agrees),
  exit 0. At 85 ms of 100 the old one again stopped short, and the new one was still counting at ~1 fps when the
  probe's own 20-minute cap cut it, which is the intended behaviour.

## Not verified / not 1:1

Everything this task left unproven or inexact goes here, each with its reason — or the single word `none`.
A reader must be able to tell from this section alone what they may not trust yet.

- Under heavy load the race now takes as long as 1500 frames take (400 s at ~4 fps): the test is slower in a busy
  gate instead of red. No upper bound but the one-minute stall; if a gate's own watch ever kills it for time, that
  is the next thing to look at.
- `the_browser_rides_on_the_gpu`'s 30 fps bar is a performance check by design (K41) and was not touched.
- Other tests in the web binary were not load-tested; MD179's gate failed only on the sx01 race.

## Next

Nothing.
