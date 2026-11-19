# 1 — phone put-back submit spikes

Task P377. 2026-11-18.

## What I did

The phone log (`context/notes/phone-174722-openmx-log.txt`, 0.18.0 build 5260, sx01 race) has two slow
frames after the wreck and put-back at 72 s: 29.6 ms at 73 s (submit 14.7) and 48.5 ms at 76 s (submit 32.9).

- **The submit stage is all of the frame's Metal encoding.** In wgpu 30 the commands are encoded into the
  driver's in `encoder.finish()`, which the shell times with `queue.submit` as "submit". So a slow submit is
  driver CPU work on what the frame drew, not a GPU wait.
- **It is not new GPU memory.** The log's GPU figure (`currentAllocatedSize`) stayed at 164 MB through both
  frames; the +5 MB is the app's footprint.
- **It is not a tune.** No `auto graphics` or render-scale line; the phone ran fixed at 0.75. Overlay patches
  are empty since P170.
- **The desktop does not reproduce it.** Headless sx01 races (`--race --racers 4 --laps 3 --video-preset
  phone --size 1280x720`, a tape with Cross held and the stick hard over every 8 s; 8–9 wrecks and put-backs
  a run) never had a submit over ~0.9 ms on a quiet machine. A temporary wgpu `counters` probe showed no
  texture, buffer or pipeline made mid-ride beyond small ground index growth.
- **What a put-back does that the first picture did not:** I counted each pipeline's first draw
  (`Drawn::first_drawn`). After the warm frame (P150), the ride first drew `additive, ghost; additive, no
  depth, ghost` (the see-through rider and bike after a put-back) and the glows, 2–3 ticks in. Those are the
  only first draws in a ride. Metal compiles a pipeline state's variant lazily, and a first draw with a new
  pipeline and blend state is the known costly one on Apple GPUs, so this is the likely cause. Not proven:
  see below.

Fix:
- `Renderer::warm_pipelines` draws one triangle, scissored to one pixel, with **every** model pipeline (all
  12: blend × depth write × ghost), the particles' and the glows'. The warm frame calls it right after its
  `render`, behind the loading screen.
- `gpu: first drawn at tick N, submit X ms: <pipelines>`: printed when a frame draws with a pipeline for the
  first time. With the fix, a ride prints none.
- `gpu: slow submit at tick N: X ms, draws, triangles, ground parts written, models uploaded, memory`:
  printed for any submit over 5 ms, so the next phone log says what that frame handed over.

### Desktop timing (release, AMD 8060S Vulkan, headless, sx01 race as above)

- Before: first draws at ticks 2–3 (ghost additive, glows); none at the put-backs.
- After, run 1: no first draws. One 7.64 ms submit at tick 755 with nothing new (183 draws, 0 uploads),
  load average 19–37 from other slots' work.
- After, run 2: 8 put-backs, no first draws, no slow submit, worst submit 4.41 ms (load average 22).
- The warm frame took 12–13 ms against 8 before, on the busy machine.

## What the original does

Nothing: port-side frame pacing. No retail behaviour involved.

## How it was proved

- `game-render/tests/warm_pipelines.rs::a_warmed_renderer_draws_a_ghost_with_no_first_draw`: a fresh
  renderer's ghost frame names a ghost pipeline as first drawn (`opaque, ghost`). After `warm_pipelines` it
  names none, and its picture is the unwarmed one to the byte. Checked that it really ran on the GPU (a
  temporary panic printed the cold list).
- The desktop races above. Logs in the session scratchpad (run5 before, run8 and run9 after).

## Not verified / not 1:1

- **The phone hitch is not reproduced here.** The desktop's submit never came near 15 ms, before or after.
  That pipeline first draws are what the phone paid for is a reasoned guess, not a measurement.
- **The timing does not fit cleanly.** The ghost first draws happen 2–3 ticks into the ride, which on the
  phone would be the very first put-back's frame at 23 s, not 73 s. Earlier put-backs (23/26/29 s) had
  submits of only 6.0/3.7/4.9 ms. So something else may cost the 73 s and 76 s frames, perhaps a variant
  Metal makes per blend constant or per render target, which the warm cannot reach. The new `slow submit`
  line is there to name it.
- **No phone log after the change.** The brief's phone check (worst under 25 ms a second through a wreck) is
  not done. P379 asks for it.
- **Ghost shadows in the sun-map pass** are not counted by the first-drawn log; their pipelines are separate.
- **The +5 MB of footprint** at the hitches is not explained.
- **The desktop's 7.64 ms submit** in one run came on a machine at load 19–37 with nothing new drawn. I read
  it as the machine, since the rerun had none, but did not prove it.

## Next

Land it. Then P379: a phone log from a build with it.
