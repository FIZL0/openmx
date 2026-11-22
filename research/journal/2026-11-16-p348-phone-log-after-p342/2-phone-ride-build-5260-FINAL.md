# 2 — phone ride on build 5260 (FINAL)

Task P348. 2026-11-21.

## What I did

- The iPhone 15 Pro on omarchy-desktop already had openMX 0.18.0 build 5260. The log's first line names
  `bd79f93` (`Release v0.18.0`, 2026-11-18 16:58); the brief named `4fce2110e` (the same day, 15:43). Both
  have P342 (`ecd76633f` is an ancestor of each), so I installed nothing.
- Drove it under `tools/ui-15.sh lock P348` (lock given back after). Killed the app for a fresh log,
  then title → Free Ride → MX Unleashed: WORLD (world_mxu, last ridden) → ride.
- Rode it with two fingers at once: GAS held and the touch stick steered now and then, in one WebDriverAgent
  `/actions` call with two touch pointers (`ui-15.sh swipe` is one finger and its call times out past
  60 s). About 6 minutes of riding, up to 76.8 mph.
- Two trips away: Home, 15 s and 10 s away, then `launch` back. Each time the app gave back its models and
  ground and came back paused; I resumed and rode on.
- Pulled `Documents/openmx-log.txt` with `ios fsync` (kept in `context/p348/ride-5260.txt`, 2146 lines,
  469 per-second lines).

## What the log says

- **`video:`** `auto, Phone (draw far, filtering 4, shadows on, shadow quality low, detail reduced, scale 75,
  limit 60)`. The rows are auto: the Phone preset.
- **Joins:** 38 joins and 8 tiles (nat17, nat13, nat18, nat04, nat16, nat06, nat21, nat03) loaded during the
  ride. No `world ground: waited` line at all.
- **While riding, no frame over 50 ms.** The slowest is 39.9 ms. 462 of the 469 seconds have a worst frame
  led by **wait** (the window's acquire and present): 30–40 ms is one or two missed refreshes
  (`pictures 16.6-33.4 ms apart`), with the GPU at 11–16 ms a frame. Every second while riding reads
  ~60 fps.
- **The 7 present-led seconds all sit at going away or coming back**, each worst frame 104–106 ms of
  present (and two of 15 s and 8.8 s while the app was suspended):

  | line | when | worst | led by |
  |---|---|---|---|
  | 1462 | WillResignActive (1st trip) | 111.95 ms | present 105.76, sim 3.63 |
  | 1471 | in the background | 15060.89 ms | present 15059.07 |
  | 1488 | first second back | 116.06 ms | present 104.76, input 7.69 |
  | 1877 | WillResignActive (2nd trip) | 117.26 ms | present 106.26, input 9.55 |
  | 1885 | going to the background | 127.01 ms | present 104.11, input 20.84 |
  | 1889 | in the background | 8827.42 ms | present 8821.99 |
  | 1905 | first second back | 107.26 ms | present 105.46, hud 1.14 |

- **What those frames spend their time on: the away rest, not drawing.** `GpuWindow::draw` in
  `ios_gpu.rs` first calls `rest()`, which waits on `rest_while_away(AWAY_REST)`: up to 100 ms while the app
  is away, returning early when it comes back. That wait runs inside the draw, before the frame's stage
  marks split the rest out, so `main.rs` bills it to present. Hence ~100 ms + a few ms on every frame at
  resign and in the background (`pictures 100.1-133.5 ms apart` there), and on the first frame back (a rest
  that ended when the app came back, then that frame's draw). The 15 s and 8.8 s frames are the same wait
  with the app suspended by iOS. None of them is the 576 ms frame P342 saw; that kind of frame did not
  happen in this log.
- **Back from away: the uploads are small, but the first frame's is bounded, not read.** The first second back uploaded
  `28 models uploaded in the draw` both times, at `upload 1.03` and `1.28` ms a frame on average. In the
  first frame back, upload is not one of the two leading stages, so it took under 7.69 ms (1st trip) and
  under 1.14 ms (2nd trip). The largest per-second upload anywhere in the log is 3.11 ms a frame. The
  second after each return is back to 60 fps (worst 18–21 ms, wait-led) while the ground builds again
  (`stream` 1.6–1.9 ms a frame).

## What the original does

Not applicable: frame times of the phone app, not the original's behaviour.

## How it was proved

- The log above, from a build with P342, read line by line (`grep worst … (stage …)` over all 469 seconds).
- The rest billed to present: read in `crates/openmx/src/ios_gpu.rs` (`draw` → `rest` → `AWAY_REST`
  100 ms) and `crates/openmx-ios/src/rest.rs` (`wait_timeout_while`), and matched to the log by the 104–106 ms
  sizes and by where they fall (only between `WillResignActive` and the first second back).

## Not verified / not 1:1

- **The jump to a marker was not done.** The touch pad has no d-pad, and the markers are only on the d-pad
  (Left/Right select, Up held rides back). No pad is on the phone. So the P342 marker-jump fix (no wait for
  fine ground over coarse) is not shown on the phone; this log has no `world ground: waited` line, but no
  jump was made either. Filed as P398 (a touch control for the markers).
- **The away trips were Home and back, not the app switcher itself.** Both send WillResignActive and then
  DidEnterBackground, and the app gave back models and ground the same way P342's switcher logs did. I tried
  the switcher too: a WebDriverAgent touch swiped up from the home indicator and held (twice, since the app
  defers the bottom edge). It never opened; the ride just went on. A switcher peek that never reaches the
  background (only WillResignActive) is therefore not in this log.
- **The first frame back is over the 4 ms budget on trip 1, possibly.** Its upload is only known to be under
  7.69 ms (the second leading stage); trip 2's is under 1.14 ms. The budget is checked before each model, so
  one model's upload past 4 ms is allowed by design. Reading the exact figure needs the per-frame upload in
  the worst frame's line, which the log does not give.
- **The rest's share of each 105 ms frame is inferred, not measured.** The per-second line has no stage for
  it, so `present` holds rest plus the frame's real encoding. Splitting it out needs a change in `ios_gpu.rs`
  and a new signed build on the phone; filed as P397 rather than shipped unseen (iOS-only code is not
  built by the gate on this machine).
- **The first frame back's upload is bounded, not read.** Only the two leading stages of the worst frame
  are logged, so its upload is known only to be under the second stage (7.69 ms and 1.14 ms).
- **The first ~48 s of the log is the bike standing** (the first one-finger gas hold timed out); riding
  covers about 6 minutes after it.
