# 1 — phone world hitches (FINAL)

Task P342. 2026-11-14.

## What I did

Read both phone logs (build 4179, world_mxu) frame by frame, sorted their slow frames by what led them, and
found three causes, none of them the tile's ground chunks or paint:

1. **Back from the app switcher: 440–460 ms frames, led by present.** P298a gives the ride's models back
   when the app goes away (`away: gave back 875 models, 556 materials and 498 textures`). The first frame in
   front again uploads every one of them inside `Renderer::render`: Mips on the CPU, buffers, bind groups.
   The present stage is the whole GPU frame, so it shows there, while `waited` (the window's acquire and
   present) stays at 11–13 ms. It is CPU work in the draw, not a GPU wait. Fix: `give_back_models` sets an
   upload budget (`LAZY_UPLOADS_A_FRAME`, 4 ms). Each frame after it resolves props and plant parts until
   the budget is spent, and draws the rest next frame (draws already skip a prop with no model). The first
   frame that finds everything up lifts the budget. `forget_assets` clears it, so a new ride's first frame
   still uploads everything it draws. Actors are never held back.
2. **A jump to a marker: ~160 ms frames, led by stream.** `world ground: waited 141 ms for mxu_free09`
   (5 of these in the logs, 137–152 ms). `GroundStream::update` blocked on the builder whenever the piece
   underfoot was not held at full detail, even when its coarse piece was held and drawable. It now waits
   only when nothing is held there. Otherwise the coarse ground is drawn until the fine piece arrives (then
   all of it goes at once, as for any piece a rider is on). Collision does not come from this ground, so
   only the picture is coarse for those frames.
3. **A tile with cars dressed mid-ride: free06 took 25–35 ms in one stream frame on the desktop.** Phone
   cost unknown: the phone logs have no such frame. `world_dress::take` read the tile's `.toy`, parsed
   every car's clip (`Traffic::load`: 25.7 ms) and built their wheels (7.3 ms) on the ride's thread. Both
   now run in `world_dress::build` on the dresser's thread. Only a tile with a car that presamples the
   ground (`PresampleTerrain`, which needs the ride's ground) still loads in `take` as before. Same 26 cars
   on clips, same ride hash (`0x2ad405e930e4260a`) before and after.

And so the next phone log names what is left:

- The per-second line splits present into **upload** (models uploaded in the draw), **submit** (the
  frame's `queue.submit`) and **wait** (the window's acquire and present; in a headless run, `finish`).
  `present` is now the encoding alone. The line also says `N models uploaded in the draw` when there were any.
- A `video: auto|chosen, <preset or Custom> (rows…)` line at ride start says which rows the ride runs on.

The 576 ms frame at 114 s of the 11:55 log came 4 s after a return from the switcher, with nothing logged
around it and `waited` 8.2 ms a frame. That makes it CPU in the draw, but I could not name it: the desktop
never had such a frame. P347 asks for the phone log that will.

The task's other guesses do not hold up. Pipelines are made when a material is made, which for a mid-ride
tile happens in the stream stage's pre-upload, and a give-back keeps them. Ground chunks are already
spread (256 a frame, 5.5 ms at worst on the desktop). Paint pages are spread (4 MB a frame, 1.5 ms slowest).

### Desktop timing (release, 2556x1179, world_mxu, Cross held, phone reach flags)

`--world-reach 3000,4500 --world-fine 1500,2200 --world-lean`: what a phone holds (video.rs world_reach).
AMD Radeon 8060S, Vulkan, headless (the wait is the GPU's `finish`). Other slots' tests were running on
the same machine during some runs (see Not verified).

- Before, the user's rows, 3600 frames: 60 fps throughout, one frame over 17 ms, 57 ms, led by stream
  (49.8). That was the dressing of nat18/nat12 mid-ride.
- After, same rows: worst 23.7 ms (stream). A 3100-frame Phone-preset ride before the dress change had a
  30.2 ms frame (stream 25.3) at free06's dressing. After it, nothing at free06, and the worst frame was a
  21 ms GPU wait at 28 s with nothing streaming.
- The desktop does not reproduce the phone's present-led frames at all.

### Defaults: what each row costs (wait = GPU ms a frame, 55 s of riding each, Phone preset as the base)

| row | GPU wait | vs Phone | triangles |
|---|---|---|---|
| Phone preset (Far, x4, shadows on low, reduced, 0.75) | 3.26 / 3.25 (two runs) | — | 237k |
| user's rows (Farther, 16x, shadows high, reduced, 0.75) | 4.75 | +46% | 486k |
| shadows off | 2.90 | −11% | 237k |
| shadow quality high | 3.98 | +22% | 237k |
| graphics farther | 4.06 | +25%, and ground +0.3 ms CPU | 485k |
| render scale 1 | 4.91 | +51% | 237k |
| render scale 0.5 | 2.04 | −37% | 237k |
| texture filtering 16x | 3.31 | ±0 | 237k |
| plants original | 3.29 | ±0 | 237k |
| detail full | 3.28 | ±0 | 289k |

CPU work wandered with the machine's load (the two Phone runs: 6.52 and 5.64 ms), but the GPU wait did
not, so the rows are compared by it.

**Decision: the fresh iPhone and Android default stays the Phone preset, and no code changes for it.** The
task assumed "the phone builds start with the desktop's". They do not. A fresh settings.txt has no `video`
line, so `video_auto` is on, and `video::pick(&Device::here())` gives every touch device (`cfg(ios/android)`)
the Phone preset: draw Far, filtering x4, shadows on at Low, detail reduced, scale 0.75, limit 60. An
iPhone 15 Pro (iPhone16,x) is 0 steps below it. A saved choice already stays: a file with a `video` line,
or hand-picked rows, is not touched.

The user's logs show the rows were **chosen by hand**: filtering 16x, shadows on at High, graphics Farther,
reduced, 0.75. No preset is that look, and no `auto graphics:` line was printed, so Auto was off. Those
rows cost about 46% more GPU than the Phone preset on the desktop, and twice the triangles (486k against
237k; the phone's 460–490k seconds at 47–54 fps match that). Scaled from the phone's own `waited` at those
rows (8–15 ms a frame before throttling), the Phone preset comes to roughly 6–10 ms, inside 16.7. That is
an estimate, not a phone measurement.

What each Phone default buys over the user's rows:
- **Far over Farther**: half the triangles, ~0.8 ms GPU, ~0.3 ms of ground CPU. The biggest single row,
  and the one the 16:05 log's 47–54 fps stretches ride on.
- **Shadows Low over High**: ~0.7 ms GPU (−18%). Off would save another 0.36 ms, but it costs the bike's
  shadow, so Low stays.
- **Scale 0.75**: already the user's. 1.0 would cost +51%; 0.5 saves 37%, and Auto's Tune steps there
  itself on a weaker phone.
- **Filtering x4 over 16x, detail reduced, plants**: nothing measurable on the desktop GPU. x4 stays
  because anisotropic taps cost memory bandwidth on a phone GPU more than here (not measured).

## What the original does

Nothing: a port-side frame-pacing task. No retail behaviour involved.

## How it was proved

- Desktop release rides as above. Logs in the session scratchpad (r1 before, r2 after, row-*.log rows,
  probe*.log temporary stage probes, after-dress.log).
- `game-render/tests/give_back.rs::many_models_given_back_come_back_over_frames`: 400 models with 128 px
  textures given back. The first frame back uploads fewer than all, every frame brings some back, all are
  back in the end, and the picture is the one before to the byte.
- `worlds.rs::the_fine_over_coarse_underfoot_is_not_waited_for`: coarse held underfoot, builder answering
  300 ms late. `update` returns before then with no "waited" line. With nothing held, it still waits.
- free06's dressing: the same 26 cars on clips and the same ride hash before and after.

## Not verified / not 1:1

- **No phone log after the change.** Every phone number above is from build 4179, before it. That the
  resume frames now stay near 4 ms of uploads, and that a marker jump no longer stops the ride, is shown on
  the desktop and in tests only. P347 asks for the log.
- **The 576 ms frame at 114 s (11:55 log) is not explained.** It is CPU in the draw (`waited` 8.2 ms), 4 s
  after a resume, with nothing logged around it. The desktop never reproduced it. The new upload, submit and
  wait stages are there to name it.
- **The desktop does not reproduce the phone's present-led frames at all.** Its GPU and CPU are several
  times the phone's, so its slow frames are the stream ones only.
- **The phone cost of a defaults row is estimated, not measured.** The table is the desktop GPU's.
  Filtering's cost on a tile-based phone GPU especially is not known.
- **Some desktop runs shared the machine** with other slots' tests (the first game-render check waited for
  a slot for 50 minutes). CPU stage times in those runs wander by ~15%; GPU waits were steady (3.26 against
  3.25 ms for the same row).
- **`put_mips` for a mid-ride tile is not budgeted.** Its levels are made on a thread, but all of them are
  written in the frame they arrive in. Not seen to cost a frame here (free06's tile: 2 textures).
- **A ground piece a rider is on is still handed over whole** (400 chunks in a frame, 5.5 ms on the
  desktop for 256). After a marker jump onto a coarse piece, its fine piece now arrives a few frames later in
  one go, rather than with the wait. Not measured on the phone.
- **Thermal throttling**: the 16:05 log reached "serious". Its 36–44 fps stretches late in the ride are the
  phone slowing itself, at the user's hand-picked rows. Nothing here changes that but the rows.
- **A tile whose car presamples the ground still reads its traffic on the ride's thread.** I did not check
  which world tiles have one; free06's 23 cars do not.

## Next

Land it. Then P347: a phone log from a build with this in it.
