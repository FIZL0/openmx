# 1 — mx world load (FINAL)

Task P260. 2026-11-02.

## What I found

- **Installed path** (Mods on, `~/repos/openmx/mods/tracks/world_mxu` shadowing the disc world): it
  loads. `--track world_mxu --mods ~/repos/openmx/mods --headless` rode in a few seconds. From the front
  end (`--menu --screen pause`, a scratch store with the user's settings.txt and their mods folder
  linked, on a private weston with llvmpipe) the build was 4.65 s and the ride started. The tile reads
  are not the hang.
- **Disc-built path, cold cache** (Mods off, or no installed world): `disc_worlds::world` read the 27
  maps and then ran `stitch::build::build_tiles` as a single call. Headless, that was 132 s; from the
  menu on llvmpipe, 158 s. Neither ever went to `Progress`. The loading screen sat on "Scene" at 0 %
  for two minutes, which looks hung. The user's `mods/cache/stitch` has never had a built world_mxu in
  it. Back or close then went through `loading::on_worker`: it set the cancel flag and called
  `handle.join()` on the window's thread. The build only looks at the flag at `progress.reach`, which
  comes after the stitch, so the window stopped pumping events until the stitch was done. That is the
  "can't cancel or close" freeze. A shell killed meanwhile left its `<key>.part-<pid>` folder (216 MB
  for world_mxu) in the cache for good.
- Where the stitch spends its time: 15 s in `Tile::trim_cut` over 27 tiles, on one thread. Then 20–30
  widen / relief / solve passes of 1–3 s each (`plan_widened` ~1.6 s, a solve ~3 s).

## What I did

- `stitch::build::build_watched(tiles, out, p, copy_tiles, watch)`. `build_tiles` is now
  `build_watched` with a watcher that does nothing. It calls `watch(Step)` at the start, at each tile's
  cut and seam gate and ring line, at each plan pass, at each relief re-check, and after each solved
  piece (`Solving { pass, done, of }`). The watcher's error stops the build. Inside `solve` an error
  already stops every thread at its next piece.
- `trim_cut` runs side by side, one scoped thread per tile, the same way the relief already did. The
  output is unchanged: the menu's cold world_mxu `mod.json` is byte-equal to the earlier build's, and
  so are all its joins (`cmp`/`diff -rq`). A cold world_mxu is now 92 s headless.
- `disc_worlds::world_watched(…, watch)` reports "Reading the maps n of 27", then "Joining the maps"
  (one count per tile made), then "Joining the maps, pass N" (with the pieces, while solving), then
  "Reading the world n of m". A stopped build removes its `.part-<pid>` folder. Before a build,
  `forget_dead_parts` removes any `.part-<pid>` whose process has no `/proc/<pid>`.
- `worlds::find_watched` / `read_watched`: an installed world reports "Reading the world n of m" (each
  tile, and every 32 joins) and answers a cancel. `find` and `read` keep their old signatures.
- `loading::Progress::doing(what, n, of)`: the line under the bar shows `what` in place of the stage's
  word until the next `reach`. `done()` never goes back, so a count that restarts each pass does not
  pull the bar back.
- `loading::on_worker_winding`: after it stops the build, it keeps calling `wait(Some(&mut leave))`
  until the thread has ended. `load_in_front` uses this so the window draws the pick it went back to
  and hears a close in the meantime, which turns a Back into a Quit. After a close it only pumps
  events. The line is now `loading: stopped in X s, the load's thread is gone (N running)`.
  `on_worker` itself is unchanged for its callers.
- Test hooks: `OPENMX_LOAD_CANCEL=<frames>:<line>` waits besides for the line under the bar, and
  `OPENMX_LOAD_CLOSE` is the same for the close button. A close prints
  `loading: the window was closed at <line>`.

I looked in opentennis (`hst-base/src/jobs.rs`, `hst-play/src/preload.rs`) for a cancellable load.
Its loads are bevy async-pool jobs, so there was nothing to take. This builds on P127's worker.

## How it was proved

- From the menu on a private weston, cold world_mxu off the disc:
  - Back at "Joining the maps 7 of 27": stopped in 0.02 s.
  - Close at "Joining the maps, pass 2": stopped in 1.87 s (one unwatched `plan_widened`), and the
    shell exited.
  - Ridden through: built in 142 s and riding at 158 s on llvmpipe. Headless it was 92 s, down from
    132 s.
  - The `.part` of a run killed by `timeout` was gone on the next run.
- tests/loading.rs (all through the shell, the disc and an empty mods folder):
  - `back_while_a_worlds_joins_build_returns_to_the_pick_promptly`: world_mxu_free, Back at "Joining
    the maps, pass…". It is back on Tracks with the thread stopped in < 5 s (0.08 s measured), no ride
    has run, nothing is left in the cache, and a planted dead-pid `.part` is gone.
  - `closing_the_window_while_a_worlds_joins_build_ends_the_shell_promptly`: the same with the close.
    The shell has no `--seconds`/`--frames`, so only the close can end it (0.09 s measured).
  - `a_stage_says_what_it_is_doing_and_the_bar_never_goes_back`.
- tests/disc_worlds.rs:
  - Every first ride from cold (world_mxu_free, world_mxu, world_mxu_nat) is held to `COLD` = 480 s.
    The second ride and the installed `--seams` ride are held to `WARM` = 240 s.
  - New: `the_installed_mx_unleashed_worlds_ride_promptly` rides the main checkout's installed
    world_mxu and world_mxu_free with no `--seams`, each within `INSTALLED` = 60 s.
  - Measured (test build, the four tests side by side, plus a private weston):
    - world_mxu_free cold 14.0 s, then 20.4 s.
    - world_mxu cold 145.8 s, world_mxu_nat cold 128.6 s.
    - Installed with `--seams`: world_mxu 87.7 s, world_mxu_free 14.5–20.2 s. That 87.7 s is why
      `WARM` is not 90.
    - Installed plain, alone: world_mxu 8.5 s, world_mxu_free 3.9 s.

## Not verified / not 1:1

- The user's own machine and GPU. My reproduction ran on llvmpipe in a headless weston. The installed
  world_mxu loaded there in 5 s, so if the user's freeze was on the installed path with their GPU, I
  have not seen it. I did not find a hang on that path. Its reads and checks are now cancellable like
  the rest, and there is no automated cancel test for them: the whole read is under a second, so a
  hook cannot catch it reliably.
- The build still takes 92 s cold for world_mxu (release). That time is the stitcher's widen and solve
  passes, and I did not change its algorithm. The load now reports each pass and answers a cancel, but
  it is not faster than that.
- A stopped build can take up to one `plan_widened` (~1.6 s for world_mxu) to end, because that step
  has no watch inside it. The window keeps answering during that time.
- `forget_dead_parts` only works where `/proc` exists (Linux). Elsewhere a killed shell's `.part`
  folder stays.
- Not 1:1: nothing here touches the simulation. The original has no worlds.
