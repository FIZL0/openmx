# 1 — world frame time (PART)

Task MD28. 2026-10-25. P131 folded in.

## What I did

Measured first, then fixed what the stopwatch named, one stage at a time.

**How it was measured.** `openmx --timings` gained a `stream` stage (the tiles' and the ground's
streaming), a `ground` share split out of `present` (the GPU renderer's `ground_for`), and for every
frame over 16.7 ms a `slow frame, ms: …` line with each stage and the tick, plus the second's and the
session's worst frame. Every run is the same 60 s full-throttle tape from the start line (1800 frames,
crossing joins on both worlds), headless offscreen at 1920x1080 against the user's real mods:

```
DISPLAY=:0 WAYLAND_DISPLAY=wayland-1 ./target/release/openmx --headless --size 1920x1080 \
  --mods ~/repos/openmx/mods --track world_mxu --timings --play gas.tape --frames 1800
```

The window could not be used: on this locked desktop the window's present is throttled to about one a
second, so its `present` is the compositor, not the port. `--world-reach 1500,2500` was also ridden to
force many more tile and dress loads mid-ride. `pace` is the wait for the 60 Hz tick, so idle time.

**Before (HEAD ecd23368), GPU, ms a frame over the session:**

| map | sim | stream | hud | present | worst frame | frames over 16.7 |
|---|---|---|---|---|---|---|
| free06 (one disc map) | 0.31 | 0.01 | 2.23 | 1.49 | 14.91 | 0 |
| world_mxu_free (5 tiles, 50 joins) | 0.31 | 0.03 | 2.99 | 2.11 | 30.28 (hud 23.79) | 10 |
| world_mxu (27 tiles, 626 joins) | 0.30 | 0.95 | 2.62 | 5.43 | 205.83 (stream 152.17) | 13 |

What the slow frames on world_mxu were, from the slow-frame lines and timers put in for the
measuring (removed again):

- **Crossing a join, 43-45 ms of `stream`**: the simulation's tiles read synchronously on the main
  thread when a tile came within reach (~35 ms for a national's), and up to 47 ground pieces added in
  the frame.
- **A ground piece added, 3.4-8 ms each** (P131): hashing its chunks ~0.9 ms, its LOD errors and
  vertex write ~3 ms, and the regrid over every held piece 1.5-2.5 ms, which grew with pieces x chunks.
- **`ground_for` rebuilt the whole view's index list (5.7 M indices, 23 MB) nearly every frame**, as
  any chunk changing level anywhere rebuilt all of it: 2-2.8 ms of CPU a frame.
- **The parked biplane** stood by the start since P113 and counted as a rider for streaming, so a tile
  near it but far from the bike was let go and read again: ~3 ms a tick in the tile streaming.
- **Every dressed tile's traffic cars posed and drawn every frame**, however far: 4-13 ms of `hud`
  (the props draw is in that stage).
- **A tile dressed mid-ride uploaded all its models and textures in one frame**: a 57 ms `present`
  frame; one billboard model's mip chain alone took 16-17 ms.
- **Tick 1, 206 ms**: `Tiled::every_shape` building every tile's collision on the first wreck check
  (63-132 ms), the tiles near the start read on the first tick (27 ms), and 47 pieces dropped on the
  first frame (~9 ms) because the start had built the parked plane's pieces too.

**The fixes, each with what it bought** (world_mxu GPU unless said):

1. *Tiles asked for are not let go before the next stream* (game-sim tiles.rs `asked`): the plane's
   far tile no longer re-read each tick, 3.2 ms → 0.05 ms in the streaming of that tick.
2. *Only cars within 1500 ft posed every frame, the rest every fourth* and none on a tile not drawn yet
   (world_dress `cars_near`): `hud` 2.62 → 1.64 ms a frame.
3. *The ground's index list kept per piece* (gpu.rs `GroundView.parts`): a piece's list is rebuilt
   only when one of its own chunks changes level. `ground` ~2.06 → ~0.9 ms a frame.
4. *A piece's hash and LOD errors made on the builder thread* (`GroundPiece::of`), and *the regrid
   linear* in the pieces: one piece's add 5-8 ms → 0.3-0.7 ms, the regrid 2 ms → 0.2 ms.
5. *Before the ride the tiles in reach are loaded and every collision shape is built*
   (`Tiled::load_near` + `every_shape` behind the loading screen): the 27 ms tile read and the
   63 ms wreck check on tick 1 gone.
6. *A frame hands the GPU at most 256 chunks* (`worlds::CHUNKS_A_FRAME`; a piece the rider is on
   goes whole): a national's 1444-chunk piece, 5-10 ms in one frame → under ~1 ms a frame over six.
7. *The start builds the pieces near a flown plane only* (`garage.plane_flown()`): the 47 drops on the
   first frame (~9 ms) gone, and the pieces built before the ride 128 → 81 (1.14 s of loading).
8. *A tile dressed mid-ride uploads 2 ms of its models a frame, its mips made on a thread*
   (`worlds::UPLOADS_A_FRAME`, `Renderer::mips_wanted`/`put_mips`): the 57 ms `present` frame gone.
9. *The simulation's tiles read ahead on a thread* (worlds.rs `Ahead`, the `Source` trait's
   `want`/`ready`): the 7.6 ms tile-read frame at a join gone; the main thread waits only for a tile
   the rider is on that is not read yet (it never did in these rides).

**After, GPU, ms a frame over the session:**

| map | sim | stream | ground | hud | present | worst frame | frames over 16.7 |
|---|---|---|---|---|---|---|---|
| free06 | 0.31 | 0.02 | 0.04 | 1.96 | 1.66 | 9.49 | 0 |
| world_mxu_free | 0.31 | 0.02 | 0.18 | 1.53 | 1.94 | 13.87 | 0 |
| world_mxu | 0.31 | 0.09 | 0.99 | 1.52 | 3.38 | 26.07 (tick 1) | 1 |
| world_mxu, reach 1500,2500 | 0.31 | 0.09 | 0.06 | 1.34 | 1.94 | 13.59 | 0 |

(`ground` is a share taken out of `present` from here on; before, it was inside it.) The one frame
over on world_mxu is tick 1, before the rider moves: `ground` 7.67 ms building the first view's index
lists and `present` 15.13 ms, the first GPU frame. Riding across the joins no frame is over 16.7 ms on
either world. A world_mxu_free run made while another build ran had three frames over (23-32 ms of
`hud` at ticks 217 and 610 and tick 1); the same run on a quiet machine had none, so those are the
machine's.

**Software renderer**, 900 frames of the same tape, HEAD → after, ms a frame:

| map | sky | ground | scene | hud | anim | all |
|---|---|---|---|---|---|---|
| free06 | 3.61 → 3.64 | 44.32 → 46.66 | 3.04 → 3.08 | 4.01 → 4.02 | 0.13 → 0.12 | 55.66 → 58.12 |
| world_mxu | 4.40 → 3.82 | 14.54 → 16.51 | 3.55 → 4.03 | 3.58 → 3.61 | 1.42 → 0.07 | 27.78 → 28.44 |

At 1080p the software renderer is over 16.7 ms on a disc map too, and a world is cheaper than free06
(the start's view holds less ground). The world's own cost there was the far cars (`anim` 1.42 →
0.07); its ground rasterising is the software renderer's, not the world's (V65 and the software path).

## What the original does

Nothing to port: the original has no streamed worlds. This is the port's own frame budget.

## How it was proved

- crates/openmx/tests/world_streaming.rs `a_streamed_ride_writes_its_ground_a_few_chunks_a_frame`:
  the MD17 test world ridden 540 ticks with `--world-chunks 8`; the shell's new
  `world ground: at most N chunks written in a frame of the ride (A in F frames, budget B)` line says
  no frame wrote more than 8 chunks, the pieces built mid-ride took more frames than there were pieces
  (the 13-chunk joins two each: 34 chunks in 6 frames for 4 pieces), the rider never waited for a
  piece, and the ride ends where the unbudgeted ride does.
- crates/game-sim/tests/tiles.rs `a_far_tile_asked_every_tick_is_read_once`, and the streaming test
  streams twice so a tile asked and in reach is held.
- The --timings tables above, before and after, on this machine.

## Not verified / not 1:1

- **Headless offscreen, not the window.** The window's present is throttled by the locked desktop, so
  every number is the offscreen renderer's; a window adds its swapchain present, unmeasured.
- **The first frame of a world is 26 ms** (tick 1: the first index build and the first GPU frame). It
  is before the rider moves, so not a frame riding across a join; filed as a follow-up.
- **The test's budget is chunks, not milliseconds.** Milliseconds on 5 busy slots are not a stable
  assert; 256 chunks was ~1 ms a frame on this machine's measurements only.
- **The other slots' load moves the numbers** by a few ms a frame; one world_mxu_free run under a build
  had three frames over (see above). The tables are from quiet runs (load average under ~1.5).
- **A piece split across frames draws partly for those frames** (a national's ground arrives over up to
  six frames, while still at least 200 ft away under the desktop reach), not seen to show in a run.
- **A tile dressed mid-ride appears once all its uploads are done** (a few frames later than before).
- **Far traffic cars pose at 15 Hz** past 1500 ft (every fourth frame).
- **The parked biplane's position is still the start's, not offset into the world's tiles** when it is
  not flown; it no longer drives streaming, but it is not where a world's tile puts it. Follow-up.
- **The software renderer is over 16.7 ms on worlds and disc maps alike at 1080p**; not this task's.
- **world_mxu_free before was measured in an earlier run of the same tape** (hd1 in my notes), not
  interleaved with the after run.

## Next

The work is done and committed on task/MD28, pushed by hand, **not ticked**: `./kit check --all`
is red only on main's own S119 lane (replay.rs `the_captured_backwards_turns_never_wreck` and
`the_slip_cases_replay_from_the_save_state`, "turn: the path parts at 75, not after 82"), which fails
the same on bare HEAD ecd23368 in a scratch worktree, so `./kit done` refuses. Every other test in
the gate passed (the two reds my stage-line change made, stages.rs's own test and one_fps.rs's parser,
were fixed and rerun green). Once S119 lands, rerun `./kit done MD28 "<msg>"` from this branch and
`./kit journal finish` this file. P131 is ticked in this commit (folded in, measured above).

Follow-ups filed: P149 (warm a world's first ground index lists during the load) and MD32 (put a
parked plane where its world tile puts it).
