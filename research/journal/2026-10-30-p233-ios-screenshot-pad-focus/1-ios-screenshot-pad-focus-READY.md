# 1 — ios screenshot pad focus (READY)

Task P233. 2026-10-30.

## What I did

Ported opentennis 9e7e38ae ("iOS: Tap screen to resume controller after a screenshot or a trip out of
the app", `crates/hst-base/src/ios.rs` `watch_focus` + `needs_tap.rs`), fetched 2026-10-30.

- **Notifications** — `crates/openmx-ios/src/sys.rs` `watch_focus()`: the same three,
  UIApplicationUserDidTakeScreenshotNotification, UIApplicationWillResignActiveNotification and
  UIApplicationDidBecomeActiveNotification, through `addObserverForName:object:queue:usingBlock:` on
  `[NSOperationQueue mainQueue]`. openmx-ios has no objc2/block2, so the block is a hand-laid clang
  global block literal (`_NSConcreteGlobalBlock`, `BLOCK_IS_GLOBAL`), leaked for the process; its body
  reads the notification's own `name` (one block serves all three). Each arrival logs
  `iOS <name> (<n> controllers)` to the log (`Documents/openmx-log.txt`), as opentennis's line, and
  stores `max(controllers+1)` in an atomic — opentennis's `NOTICE.fetch_max` — which the shell takes
  with `focus_notice()`. Called from `ios.rs` `main` right after `log_to`.
- **Flag** — `crates/openmx/src/hud.rs` `NeedsTap`: opentennis's rules verbatim (`notice(n)` sets only
  with n > 0, idempotent bool, never a toggle or count; `input(touched, pressed)` clears), plus
  `frame()` holding opentennis's per-frame order (input first, then the notice, so this frame's notice
  outlasts a press before it). `GpuWindow::poll` (`ios_gpu.rs`) runs it each frame and logs
  `controller needs a tap: true/false` on each change (opentennis logs it per notice).
- **Inputs that clear it** — a touch: `WindowEvent::Touch` with `TouchPhase::Started` sets
  `Shared.touched` (raw, before the touch pad, as opentennis reads raw `TouchInput`). A pad press:
  `PadNode::take` (`ios.rs`) flags any of a controller's 17 standard buttons going from up to down
  (triggers and d-pad included, as Bevy's `just_pressed`; never the touch pad, never the first read).
- **Overlay** — `ios_gpu.rs` `needs_tap_sprites`: drawn in `GpuWindow::draw` after the touch pad, on the
  whole surface (`[0, 0, size]`), so the dimmer covers the notch / island / home-bar strips outside the
  safe area; text centred on the whole window. Look is the port's own menus' (opentennis used its
  menus' too): the popup dimmer `[0,0,0,150]` (`menu_screens.rs` `popup`), heavy slanted type in CHALK
  with a dark drop shadow, 6% of the short side. Draw only: nothing paused, no input swallowed, the sim
  untouched.
- Release note line, as opentennis's.

## What the original does

Nothing — an iOS-only shell behaviour.

## How it was proved

- `hud.rs` `needs_tap_tests` (4 tests, `./kit check -- -p openmx --bin openmx -- needs_tap`: passed):
  opentennis's test (no controller → stays unset; screenshot with one → set; released buttons aren't
  input; touch clears; pad press clears), the double screenshot notice is one tap, launch's
  DidBecomeActive with no controller does nothing, a same-frame press doesn't eat the notice.
- iOS target: `tools/ios-build.sh` (release, aarch64-apple-ios) builds with no warning from the files
  touched. Desktop/web: `./kit check --all` in `./kit done`.

## Not verified / not 1:1

- **Which notifications a screenshot sends on openmx, on device**: not seen — needs the user's iPhone
  run (no device here, unattended). opentennis's device result (user 2026-10-30) is two
  UIApplicationUserDidTakeScreenshotNotification in a row; WillResignActive on leaving the app;
  DidBecomeActive at launch. Same observers here, so the same is expected, unproven.
- **The overlay on device** (covers notch and home-bar strips, one tap / one press clears it): only
  reasoned from the code (drawn on the whole surface) and the build; never seen on a phone.
- **The hand-made block literal**: compiled and linked, never run. It follows clang's global-block
  ABI (isa, flags, reserved, invoke, descriptor{reserved,size}); if Foundation ever wanted a
  signature (BLOCK_HAS_SIGNATURE) it would be missing — addObserverForName does not ask for one.
- **tvOS**: openmx has no tvOS target, so there is nothing to build; the code is `target_os = "ios"`.
- **Look**: the port's own menus' style, not opentennis's (its look is tennis's); text size 6% of the
  short side is a choice (opentennis: 5 vmin).
- **Pad press while the pad layer isn't reading**: a press is seen in `PadNode::take`, which runs when the
  pad layer reads a controller's node; a screen that opened no node for it would not clear the flag by a press
  (a touch still does).

## Next

-READY for the human: on the iPhone, with a controller connected, take a screenshot (side + volume),
check the overlay covers the whole screen incl. notch / home-bar strips and that one tap or one button
press clears it, then send Documents/openmx-log.txt's `iOS UIApplication…` lines to paste here.
