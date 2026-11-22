# 1 — gpu times on the phone (FINAL)

Task P346. 2026-11-22.

## What I did

- Read the P301b logs (`context/phone-logs/p301b/`, 0.18.0, which has the 11-17 change). They did
  carry GPU times now, but only about one second in ten had any, and then one frame: 362 of
  428 seconds in `ultra-2.txt` read `gpu no frame timed`.
- Added a count of the frames the timer loses, and why, to the line a second:
  `, 61 untimed (0 busy, 61 unusable, ticks A B)`. `busy`: every slot in flight. `unusable`: the
  two ticks read back give no time; `A B` is the last such pair as read.
- Built that (`tools/ios-ipa.sh`, ship + native sim), installed it on the iPhone 15 Pro over
  omarchy-desktop (`ui-15.sh install`, re-signed for `io.github.fizl0.openmx`) and raced sx01 by
  touch (gas held 100 s). Every frame was `unusable`, about 60 a second, none `busy`. The ticks are
  nanoseconds (36350 s: the phone's uptime). The end read **25 ms before the start**, every second
  (e.g. `36350078832166 36350053410375`).
- What that means: the copy-out of the timestamps (resolve) sat in the frame's own command buffer,
  right after its last pass. Metal does not order a counter sample buffer against a blit, so the
  resolve read the end slot before the last pass's end-of-fragment sample was written: it got the
  value that slot's last frame left there. Slots came round every second frame, so end − start =
  GPU time − 33.3 ms ≈ −25 ms, which `take` drops. A frame now and then read right, which is the
  odd `gpu 14 ms` second in the P301b logs.
- Fix (`crates/game-render/src/gpu/timer.rs`): the frame's command buffer no longer resolves. Its slot
  goes `DRAWING` at submit, `DRAWN` from `queue.on_submitted_work_done`, and `take` (once a frame)
  resolves the `DRAWN` slots in a small submit of its own, then maps them as before. The samples are
  written by then, since the GPU has finished the frame. 8 slots, not 4: a frame is held a frame
  longer.
- Second build, same sx01 ride: **every second timed, no untimed frames**: GPU 4–9 ms a frame on
  average, worst 13 ms, at a steady 60 fps (sx01 is a light track).
- Then the question itself, on the heavy world: Free Ride 10 of 10 (world_ultra), gas held in 50 s
  pieces, 350 s riding (`context/phone-logs/p346/ultra.txt`). Every second timed.

## What holds the drawable

**The GPU.** Of the 118 seconds with a frame of 28 ms or more on world_ultra, 117 had a GPU frame over a
refresh (16.7 ms): GPU 20–40 ms a frame in those seconds, the drawable wait about the GPU time less the
CPU's ~5 ms. The surface hands back a drawable only once the GPU has finished with one of the three.

Why the GPU gets slow — thermal, by these numbers (20 s means):

| seconds | fps | triangles | GPU ms a frame | GPU ms per million triangles | thermal |
|---|---|---|---|---|---|
| 0–79 | 60 | 160–183k | 7.8–9.4 | 46–52 | nominal → fair at 52 s |
| 80–139 | 41–49 | 131–176k | 19–24 | 111–185 | fair, serious from 123 s |
| 140–179 | 60 | 165–170k | 7.1–7.3 | 43 | serious |
| 180–279 | 54–60 | 139–205k | 9.5–15 | 47–100 | serious |
| 280–339 | 28–30 | 160–212k | 32–35 | 156–216 | serious |

- The same scene costs 2–4× the GPU time in the slow stretches: triangles and particles (100–250 live)
  are much the same as in the fast ones, at the same render scale (0.75, the Auto preset's Phone rung).
- The CPU slows with it: sim ~2 ms a frame when fast, ~5 ms when slow. A heavier scene would not slow
  the simulation; a clocked-down chip slows both.
- It comes in waves (slow 80–139 s, fast again 140–179 s, slow 280–339 s), the shape of the chip
  stepping its clocks down and up as it heats, with the thermal state at fair, then serious.
- The P301b `ultra-2.txt` fall to 27–30 fps from 419 s is the same thing: its rare timed seconds there
  read GPU 25–33 ms, sim doubled too.
- sx01 (light, 4–9 ms GPU) had one frame over 28 ms in 129 s, in the first second, with the GPU at 8 ms:
  a start-of-ride hitch, not the GPU. No stall of the user's 11-13 kind (a few seconds apart, GPU
  small) showed on either ride.

## What the original does

Not applicable: frame timing on the phone is ours (AGENT.md, rendering).

## How it was proved

- On the iPhone 15 Pro (A17 Pro, Metal), two builds of this branch, the same sx01 race by touch:
  before the fix 129 of 129 seconds `gpu no frame timed` with ~60 unusable frames each; after, 0 of 129.
  The logs: `context/phone-logs/p346/sx01-before.txt`, `sx01-after.txt`.
- `crates/game-render/tests/frame_time.rs` on the desktop GPU: each of the three ways of drawing a
  frame still comes back timed (7–8 of 8), with the frame's own render scale. gpu.rs's
  `the_gpu_times_its_frames_where_it_can` now takes one more look after the last frame, since its times
  come in a look later.
- `./kit check --all`: green but for whip_tricks' three 360 tests, which this diff does not touch. They are
  red with main's trick pack in the slot and skip without it; filed as K87.

## Not verified / not 1:1

- **The cause of the stale end is inferred.** That Metal does not order the counter samples against the
  resolve's blit is read from the numbers (end − start ≈ −25 ms ≈ GPU time − two refreshes, every
  frame) and from the fix working, not from Apple's documentation.
- **The desktop test cannot fail the old way.** On Vulkan the resolve in the same command buffer was
  fine, so frame_time.rs passes with either timer here; the phone log is the proof.
- **Thermal is inferred, not measured.** iOS gives only the thermal state, not clocks. That the slow
  stretches are the chip throttling rests on the same scene costing 2–4× and the sim slowing with it.
  Where the bike was in the world was not logged per second, so a heavier view (fill, alpha, shadow) in
  those stretches is not ruled out.
- **The user's 11-13 stalls were not reproduced.** Their log's few-second 28–55 ms frames with nothing on
  the CPU came on a different build and ride; neither ride here showed that pattern, so whether those
  were the GPU or the compositor is still open. With the timer fixed, any new phone log answers it: a
  stall second's `gpu … (worst N)` over 16.7 ms is the GPU.
- **No change or test in `crates/openmx/src/ios_gpu.rs`**, the task's file. The surface code there was
  not what held the drawable (the GPU was), and the fix is in the shared GPU timer. A desktop test cannot
  show Metal's ordering, so the proof is the phone logs, not an "ios present stalls" test.
- **Only the iPhone 15 Pro, two rides** (sx01 129 s, world_ultra 350 s), touch only, phone on USB power.

## Next

- The Auto preset sets the render scale once (75 on this phone) and the GPU-timed scale ladder does not
  run under it, so a throttled phone holds 75% at 30 fps. Letting the ladder step the scale down while
  the GPU is over budget would hold 60 fps longer: filed as P399.
