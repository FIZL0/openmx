# 1 — ios home bar swipe held (FINAL)

Task P305. 2026-11-05.

## What I did

Why the first swipe still left in 0.10.3, going down the brief's list:

- **winit asking the wrong view controller / sys.rs replacing it**: no. The fork's `WinitUIWindow` sets its
  `WinitViewController` as `rootViewController` (third_party/winit/src/platform_impl/ios/window.rs:85), that
  class overrides `preferredScreenEdgesDeferringSystemGestures` and `prefersHomeIndicatorAutoHidden`
  (view_controller.rs:53-67), and nothing in openmx-ios/sys.rs sets a root view controller or makes a window.
- **set before the view controller is in place**: yes in part. `WinitViewController::new` sends
  `setNeedsUpdateOfScreenEdgesDeferringSystemGestures` before `setView` and before the window holds it, so that
  ask is a no-op; UIKit should still read the property when the VC becomes root, but nothing asks again after
  a turn. opentennis's only fix in this area (6815b2ce, "re-ask to hide the home indicator after a turn") re-sends
  both `setNeedsUpdateOf…` after each turn. Ported: `openmx_ios::defer_gestures` sends them to the window's root
  view controller, and `Host::look` calls it every look until the VC reports, then after each turn.
- **the indicator hidden**: the likeliest cause. The window asked for both `prefersHomeIndicatorAutoHidden = YES`
  and every edge deferred (P240, copied from opentennis's B623). UIKit's widely reported behaviour (Apple dev
  forums / Stack Overflow, Unity's "Hide home button" vs "Defer system gestures on edges") is that while the
  indicator is auto-hidden the bottom edge's deferral is ignored and a swipe up goes home at once; the games that
  "lock the home bar until you do it a couple of times" keep the indicator up, dimmed, and defer the bottom edge.
  So `with_prefers_home_indicator_hidden(false)` now, all edges still deferred, in rides and menus alike (it is
  the window's setting, there is no per-screen switch).

The log now says, after launch and whenever it changes, what the view controller UIKit asks reports, e.g.
`ios: system gestures deferred on bottom, top, left, right, home indicator shown` — and appends
`: a first swipe up leaves` when the bottom edge is not deferred or the indicator is hidden, and
`(asked of another view controller than the window's)` if the VC that answers is not winit's (`Gestures`,
crates/openmx-ios/src/gestures.rs, built and tested on every platform).

opentennis: has the same two settings (crates/hst-base/src/widescreen.rs:239-241) and 6815b2ce's re-ask; no fix
past that to copy. Not filed there (memory: shared iOS fixes follow opentennis, no tasks in its plan unasked) —
if the phone confirms this, opentennis wants the same one-line change.

The fps spikes / 0 triangles after each resign in the user's log are P302's (the loop now rests while away);
not touched here.

## What the original does

Not applicable: a platform behaviour of the port's iPhone shell.

## How it was proved

- `tools/ios-build.sh check -p openmx --bin openmx` (aarch64-apple-ios): builds, no warnings in the changed files.
- `./kit check -- -p openmx-ios`: 17 passed, incl. new `the_line_says_which_edges_and_whether_the_home_bar_holds`.

## Not verified / not 1:1

- Not run on an iPhone: no device on this machine. That one swipe up now stays in the game, and what the new
  `ios: system gestures deferred on …` line reads, need the user's next build's log. If it reads
  `home indicator shown` with the bottom deferred and the first swipe still leaves, the cause is elsewhere.
- That iOS ignores the bottom edge's deferral while `prefersHomeIndicatorAutoHidden` is YES is from developer
  reports, not Apple's documentation, and not tested here; it is the reason for the change, unproven.
- The home indicator is now always shown (dimmed while deferred) instead of fading out, in rides too — the price
  of the held swipe.
- Whether the turn re-ask matters on its own is not separable without a device.

## Next

The user's next iPhone log: look for the `ios: system gestures deferred on` line, and swipe up once mid-ride.
