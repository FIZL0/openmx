# 2 — iphone frame rate (FINAL)

Task P324. 2026-11-21.

## What I did

- Drove the iPhone 15 Pro on omarchy-desktop's USB (`tools/ui-15.sh`, lock P324, given back). Pushed the disc
  to Documents, rode two rides with the touch pad, and pulled `openmx-log.txt` after each. Build 5260 (bd79f93),
  native sim, Phone preset (draw far, 4x, low shadows, reduced models, scale 0.75), no controller.
  - `research/p324/2026-11-21-phone-dallas-log.txt`: Dallas, a ride and a pause.
  - `research/p324/2026-11-21-phone-world-log.txt`: the same, then world_mxu_free (the freeride maps):
    ~220 s of riding with a 25 s pause in the middle.
- What the logs say:

  | | fps | CPU work a frame | gpu a frame | triangles | worst frames |
  |---|---|---|---|---|---|
  | Dallas ride | 59.9–60.1 | ~5 ms (sim 0.8, hud 0.6, submit 1.5, present 2.2) | 5–8 ms | 160–295k | 19–20 ms |
  | Dallas pause | 60.0 | ~5 ms (hud 0.9) | — | — | 19–20 ms |
  | world ride, standing | 60.0 | ~5 ms | — | 230k | 19 ms |
  | world ride, 20–75 mph | 57–59.3, about 1 s in 2 | ~5 ms (sim 0.8, stream 0.09, ground 0.3) | 12.2–14.7 ms | 250–310k | 29–41 ms, all `wait` |
  | world pause | 59.9–60.1 | ~5 ms | — | 250k | 19 ms |
  | world, first frame after the pause | 54.0 | sim 114.6 ms in one frame | — | — | 117.7 ms |

  "gpu" is the few seconds Metal timed a frame (it mostly prints "no frame timed").
- So, on this build and with no controller:
  - **The pause holds 60** on both, so the 40 fps pause of 2026-11-05 did not come back. That log's pauses
    all fell between a controller connecting and a trip to the background. An agent cannot connect a pad.
  - **The riding misses are the GPU on the world.** The CPU's work is ~5 ms and then waits ~11 ms on the
    surface. The world's frames take 13–14.7 ms of GPU against Dallas's 5–8 ms at a like triangle count, so a
    heavier frame now and then runs past the refresh. The misses grow with speed (more ground and props come
    into view). The ground stream (0.09 + 0.3 ms) and the sim (0.8 ms) are small, so spreading the ground's
    builds or moving the sim to the worker would not win a frame. Filed V139 to find which pass costs it.
  - Auto graphics timed the world while the rider stood at the start ("60 fps holds at Phone" at 6 s). It
    also drops the slowest 5% and steps down only under 93% of 60, so 58 fps never moves it. V139 names that.
  - **The one big hitch was the pause's.** On the pause's first frame `Tiled::stream` dropped mxu_free06 and
    mxu_free11 ("world: mxu_free06 dropped" at 153 s). They were past the drop reach and held only because the
    world's cars on them asked about their ground every tick, and that frame asked nothing. The first tick
    back read both again on the ride's thread, a 115 ms frame (179 s).
- The fix (game-sim `tiles.rs`): a far tile stays until 60 `stream` calls have passed with nothing asking
  about it (`ASKED_KEPT`, about a second of frames), not one. The pause runs no stream calls, so the tiles
  are still there when the ride goes on. The stitch's pass over the whole world, which streams after each
  piece to keep its memory down, keeps the old rule (`kept_for(0)`).

## What the original does

Not applicable: frame pacing and the world's tile streaming are the port's own.

## How it was proved

- The two phone logs above, line by line (the stage, worst-frame and gpu fields of the once-a-second line).
- game-sim `tests/tiles.rs`: `streaming_never_changes_an_answer` and `a_far_tile_asked_every_tick_is_read_once`
  now check that a far tile stays through `ASKED_KEPT` streams with nothing asking and goes at the next.
- `./kit check` for the rest.

## Not verified / not 1:1

- **The fix is not checked on the phone.** That needs a new signed IPA installed on it, and this run builds
  none. The next phone log riding world_mxu_free and pausing should show no "dropped" line at the pause and
  no 100 ms frame after it.
- **No desktop before/after timing for this change.** It changes no frame's cost, only when a far tile is
  let go, and the desktop's pause cannot be driven by a tape (the tape stops with the ticks). The unit tests
  cover the rule.
- **The riding misses on the world are not fixed.** Which GPU pass costs the extra ~7 ms is not known:
  Metal times only whole frames, and the desktop does it in ~1 ms. Filed as V139.
- **The 40 fps pause with a controller connected is not reproduced.** No pad could be connected by an agent.
- The memory still swings ~100 MB from one second to the next while riding (630↔790 MB on Dallas,
  950↔1150 MB on the world). It costs no frame on these logs; not looked into.

## Next

- Build and install a new IPA, ride world_mxu_free, pause and go on: no hitch after the pause.
- V139 for the world's GPU time.
