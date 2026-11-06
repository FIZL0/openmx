# 1 — iphone frame rate (READY)

Task P324. 2026-11-05.

## What I did

- Read the user's 21:20 phone log (`research/p324/2026-11-05-2120-openmx-log.txt`, build 3555, A17 Pro,
  2202x1179 at render scale 0.75, far, 4x filtering, low shadows, reduced models). Each "frozen" stretch, with
  its mean fps and memory at start and end:

  | seconds  | fps  | memory MB   | what it was |
  |----------|------|-------------|-------------|
  | 1–68     | 60.0 | 1231→948    | the resume offer at launch |
  | 95–150   | 60.0 | 1023→963    | the pause; the pad was out (touch pad 1P) and came back at ~150 s |
  | 163–179  | 41.3 | 980→1124    | the pause, controller connected: +176 MB in its first second |
  | 207–223  | 41.6 | 1013→1113   | the same, +137 MB |
  | 495–613  | 40.8 | 1059→1184   | the same (an app switch at 495 s, no trip to the background) |
  | 650–688  | 58.3 | 1030→997    | the pause after a trip to the background (models given back and rebuilt), pad out |
  | 715–731  | 60.0 | 1030→1007   | a pause after that, controller connected again |

  So the 40 fps pause is not every pause. Each one came between the controller connecting at ~150 s and the
  app's trip to the background at 649 s. Metal's own count (`GPU N MB`, `currentAllocatedSize`) does not move
  while the footprint grows 140–180 MB, so the growth is CPU memory or memory Metal does not count, and it goes
  back within two seconds of the pause closing. Triangles a frame under the slow pauses (260k) are fewer than
  under the 60 fps ones (339k–394k), so the 3D scene is not what costs.
- Riding: 59.78 fps on average, and 186 of 365 riding seconds under 59.5 fps. 59.0 is 60 frames over 61
  refreshes, so it is about one frame a second waiting for a second refresh, and it is real: the line's
  window is wall time from line to line. It does not follow the ground stream (seconds with a `world ground:
  … built` or `world paint:` line: 11 of 27 under 59.5; seconds without one: 186 of 365) or the triangles
  (each 50k band from 250k to 450k misses about half its seconds).
- Step 1: the phone's line a second now says where the frame goes (`main.rs`, `stages.rs`). On iOS and
  Android, without `--timings`, each second also prints
  `     ms a frame: <each stage ≥ 0.05 ms> , all N; worst N ms (<two dearest stages>), N over 16.7; widest gap N
  ms, waited N ms a frame, N sprites, gpu N ms a frame (worst N)`:
  - the stages are the desktop's `--timings` stages (`Stages::brief`): sim, stream, ground, scene, hud, present
    and the rest;
  - `waited` is the present stage's wait on the surface: the acquire plus the present on the phone
    (`GpuWindow::waited`). The rest of the present stage is the renderer's own encoding;
  - `widest gap` is the longest time between two frames' ends that second, the stall a rider sees;
  - `gpu` is V122's timestamps, when the adapter has them (`gpu not timed here` when not);
  - `sprites` is the most sprites a frame drew over the views (the HUD and the pause).
  On the desktop `--timings` gets the same tail after its worst frame (`FrameSecond`).
- Step 4, the desktop: release build, `--menu --screen pause` on world_mxu_free, on a private headless weston
  with the GL renderer (1650x884, 60 Hz, Radeon 8060S over Vulkan). The settings were set to the phone's look
  (render scale 0.75, far, 4x filtering, low shadows, reduced models). A fifo pad pressed Start and held the
  gas. Both before and after this change it holds 60.0 in the ride and in the pause:

  | | fps | CPU work a frame | waited | gpu a frame (worst) | sprites | widest gap |
  |---|---|---|---|---|---|---|
  | ride  | 60.0 | ~1.2 ms (sim 0.02–0.05, stream 0.11, ground 0.18, hud 0.55) | 15.1–15.6 ms | 0.59–0.82 ms (1.03–1.51) | 477–480 | 17.2–18.8 ms |
  | pause | 60.0 | ~1.2 ms (hud 0.55) | 15.3–16.0 ms | 0.54–0.79 ms (0.95–1.41) | 826–842 | 17.1–18.1 ms |

  Before the change (the same runs without the `waited`/`sprites` fields, earlier today): ride gpu 1.1–1.36
  ms, pause 1.31 ms, hud 0.45 vs 0.56 ms. The desktop's RSS does not jump when the pause opens: 656→660 MB
  with the sound off, 678→682 MB with it on (through a stand-in `pw-cat`).
- Read and ruled out as the pause's +150 MB / 8 ms, on the desktop and by reading the code:
  - the Tricks page's stage (it loads once and is never freed, but the memory goes back);
  - `set_images` (once a ride);
  - the sprite buffers (842 sprites is small);
  - the pause's music (`cue(false)` in a ride, so no song change);
  - the render-scale path (`over` is the Garage's only);
  - the depth texture (one size);
  - the touch pad layer (it draws nothing with a controller connected);
  - rumble (only on a wreck);
  - `openmx_ios::pads()` (runs in an autorelease pool, once a frame a pad, riding or paused alike).
- Built the iPhone app with the change locally (`tools/ios-ipa.sh`, 4 min 38 s, exit 0). The unsigned IPA
  (12.9 MB, on top of 0b82dca3 with this change uncommitted) is in the s1 worktree's git-ignored
  `target/openmx-ios-p324.ipa`. Nothing was uploaded: this run publishes nothing beyond git, so the user
  signs and installs it, or a release carries it.

## What the original does

Not applicable: the frame pacing and the log line are the shell's own, not the original's.

## How it was proved

- The desktop's lines above, from the release build's `--timings`, before and after, in a phone-sized window on
  the GPU (two runs of ~30 s each: a pause, a ride, a pause).
- The phone line's code path (`PHONE && …` in the ride loop) compiled for aarch64-apple-ios (`tools/ios-ipa.sh`).
- `./kit check --all` for the rest.

## Not verified / not 1:1

- **Steps 2 and 3 are not done: no fix for the pause's 40 fps or the riding misses.** The desktop does not
  reproduce either one, since it holds 60 with ~1 ms of GPU a frame. The phone log names no cause: no pause
  screen, CPU stage or GPU time. Changing the sim's thread, the ground stream or the allocations blind would
  be a guess with no way to see it worked. The next phone log with this build says which.
- **No build was sent to the phone.** The IPA was built but not uploaded or installed (no publishing from an
  unattended run).
- **The phone line itself is unseen on a phone.** It compiled for iOS but has not run there. Android
  (`android_so.rs` is the same file) was not built.
- **Whether the A17's Metal adapter gives timestamps** (the `gpu` field) is not known. If it does not, the
  line says `gpu not timed here`, and `waited` is what tells the GPU's time.
- **The pause's +150 MB is unexplained.** Metal's own count does not move. The desktop's RSS does not move.
- **The 40 fps pauses all fell between the controller connecting (~150 s) and the background trip (649 s).**
  That pattern is from one log, and it may be chance.
- **The desktop's numbers are not the phone's.** Its GPU is far faster than an A17, and its window is 1650x884
  rather than 2202x1179.

## Next

The user, with the build that has this change (HUMAN.md "Where the iPhone's frame goes (P324)"): ride a few
minutes, open the pause for ~20 s twice (once with the controller connected), and send the log. Then the next
session (P324 again):

- if the slow pause's `waited` is high and its CPU stages are low, it is the GPU or the display;
- if `menu`, `hud` or `input` is high, it is the pause's own work on the CPU;
- the riding seconds under 59.5: `widest gap` ~33 ms names the stall, and the `worst …` names its stage.
