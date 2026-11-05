# 1 — ios loop rests while away (FINAL)

Task P302. 2026-11-04.

## What I did

- `crates/openmx-ios/src/rest.rs`: `Rest`, a mutex-held "away" flag plus a condvar. `go()` sets the
  flag, `come_back()` clears it and wakes any waiter, and `rest(most)` waits while away until the app
  comes back or `most` runs out. It is plain logic built on every platform, so the desktop tests cover it
  (no wait when not away, a wait for the full `most` when away, and a prompt wake on coming back).
- `sys.rs`'s lifecycle block (P298's `on_lifecycle`) calls `go()` on WillResignActive and
  DidEnterBackground and `come_back()` on DidBecomeActive. It exposes `rest_while_away` and `is_away`.
- `ios_gpu.rs`: `GpuWindow::draw`, which every loop's picture goes through (the menus, the loading
  screen, the ride, and `present`/`present_again`), now rests first while the app is away: at most
  `AWAY_REST` (100 ms), and it draws nothing. The loops keep polling, answering P298's away and running
  any owed ticks ten times a second, and DidBecomeActive ends the rest at once. One log line goes out on
  the first rest (`ios: away, the frame loop rests`) and one on the way back (`ios: back, the frame
  loop rested N s`).
- Why `draw` and not each loop: the spin was `draw` returning `None` with nothing to wait on (no drawable
  while inactive). One place covers all three loops, and no shell loop in `main.rs` changed, which keeps
  merges clean.
- The brief also says the ride kept going through WillResignActive in 0.10.3 (57 mph at 12 s). That was
  0.10.3 itself: P298's pause (`cff3fd7b`) is not an ancestor of `Release v0.10.3` (`55619594`), so it
  comes with the next release. Nothing was changed for it here.
- Gate fix outside the task: `disc_worlds`'s `world_mxu_free_is_built_off_the_disc_as_openmx_mods_builds_it`
  failed on main as it stands ("built from cold: its start file is stale"). MD95's one-row layout and
  P299's kept start files each landed while the other was still in progress. As the test's message says,
  I copied the build's kept `start.json` over `crates/stitch/worlds/world_mxu_free.start.json`.
- From opentennis: nothing to take. Its iOS loop is Bevy's/winit's, and `hst-base/src/ios.rs` has no
  inactive pacing.

## What the original does

Not applicable (the iOS shell's frame loop; the PS2 game has no app lifecycle).

## How it was proved

- `rest.rs`'s three unit tests (desktop, in the gate).
- `tools/ios-build.sh check -p openmx --bin openmx` builds for `aarch64-apple-ios` with no errors or
  warnings in the changed files.

## Not verified / not 1:1

- **Not run on a phone.** No iPhone is reachable from here. So it is unconfirmed that the fps log reads
  about 10 instead of 3700-13556 while in the app switcher or Control Center, and that the picture is
  back the moment the app is active. The next 0.10.x log from the user shows it: look for the
  `ios: away, the frame loop rests` / `ios: back, … rested N s` lines and for the fps around them.
- **Ticks while away come in bursts:** an owed tick backlog of about 3 ticks per 100 ms rest, run through
  before the next draw (N23a's backlog). An offline ride is paused there (P298), so nothing shows. An
  online ride keeps its ticks, but they come in bursts instead of at 30 Hz. Not checked against a live
  netplay session on a phone.
- **Behind Control Center the picture holds still:** the frame drawn before the app went inactive stays
  up, as it does in other games. Untested whether iOS would otherwise have given drawables there.

## Next

The user's next iPhone log (see Not verified).
