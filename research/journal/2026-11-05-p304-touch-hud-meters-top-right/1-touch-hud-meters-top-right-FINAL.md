# 1 — touch hud meters top right (FINAL)

Task P304. 2026-11-05.

## What I did

- The ride HUD that is actually drawn is `ride_hud.rs` (P121); `overlay.rs`'s original-art HUD is no
  longer an `--overlay` choice (P170), so the brief's `overlay.rs` needed no change. The move is in
  `ride_hud::sprites_clear_of`. It already took the page's touch pad as `Option<Room>` (P208), which
  pushed the speedometer left of the faces and up over MODIFIER. That push is replaced:
  `to_the_top_right` moves the speedometer (digital or dial) and the preload meter into the view's top
  right, their top level with the score bar. It does this only for the view whose bottom-right corner
  is the frame's, the one the pad covers, as P208 did. A top split view keeps its places, and P208's
  test still holds that.
- Where in the top right: the pad's face diamond goes a long way up. On an 844x390 phone △'s top is
  68 px from the top, so the meters (74 px tall there) do not fit over it at the right edge. There are
  three candidate places: the right edge over △, left of △ over □, and left of □ in the top half.
  Each must also stay right of PAUSE along the top. The meters go to whichever place takes them
  biggest: full size where one fits, otherwise shrunk to fit the best one. The top-right column (marker
  menu, camera toast, join cue) starts below the meters while they are up there.
- Results from a throwaway probe (removed): digital is full size everywhere. It goes left of △ on
  844x390, 932x430 and the iPhone, and at the right edge on 1180x820. The analog dial is taller and
  shrinks: ×0.85 on 844x390, ×0.95 on 1180x820, ×0.63 on the iPhone 15 Pro (frame 734x393).
- `touch_room.rs`: added `over_the_faces`, `over_the_square`, `left_of_the_faces(down)` and
  `right_of_the_top` (all touch.js numbers, in its `u`, with the same 0.15 u slop + 0.2 u gap). Removed
  `over_the_pedals` and its two constants, since their only user was the old speedometer push.
- iPhone: `ios_gpu.rs`'s `GpuWindow::poll` now sets `touch_room` while `ios::TOUCH.shown`, with the
  room inside the kept insets as the "window". The pad is laid out from those same edges
  (`touch_pad.rs`), and the frame is centred in that room as `Room` assumes. A real controller hides
  the pad, `touch_room` goes `None`, and the HUD is drawn exactly where it was. Side effect: the iPhone
  now also gets P208's other placements, the radar right of the stick and the menus' hints clear of the
  pad. That is the rest of F11. I left F11 open because none of it was checked on a device.
- `cargo check --target aarch64-apple-ios` (tools/ios-build.sh) builds.
- opentennis: nothing to take. It has no touch HUD.

## What the original does

Nothing. The original has no touch pad, so this is front-end layout, ours to design (AGENT.md).

## How it was proved

- `tests/ride_hud.rs::with_the_touch_pad_up_the_meters_sit_top_right_clear_of_its_buttons` runs on
  844x390, 932x430 and 1180x820 page windows, and on a 2556x1179 iPhone at 3x with 177 px island
  insets kept each side. It covers both speedometer styles. With no pad, the HUD is identical to
  `sprites` and the meter is in the bottom-right quarter. With the pad, the sprites that move are the
  meters, and their box lies in the top-right quarter of the frame. No moved sprite overlaps any of the
  pad's ten controls (the seven bottom-right ones, the stick base, and CLUTCH, CAMERA and PAUSE, laid
  out from touch.js's own numbers in the test). The marker menu starts below them.
- P208's `the_speedometer_and_the_radar_stand_clear_of_the_touch_pad` still passes unchanged.
- `touch_room` unit test: the new places on the 844x390 phone.

## Not verified / not 1:1

- Not looked at on a phone or in a browser. The no-overlap proof is geometric: sprite boxes against
  touch.js's button boxes, written out again in the test. Nobody has looked at a real screen.
- The iPhone's `Room` sizes `u` from the inset room's width rather than the whole window's. It is the
  same whenever `u` is height-bound, as on every landscape iPhone (the test asserts that for its case).
  On an iPad-shaped window that is width-bound, the places would be a little off.
- The analog dial is shrunk on phones (to ×0.63 on the iPhone 15 Pro) because it does not fit the gap
  between PAUSE and the face diamond at full size. Whether that is still readable is a judgement call
  for a person on a device.
- In a split screen with the touch pad up, only the frame's bottom-right view moves its meters, and
  its top right may still be under the face buttons. Not handled: the touch pad is shown only with no
  controller connected, or while kept for a joined player, so a split with it up is rare.
- The marker menu and camera toast in the top-right column were already under the pad's faces before
  this change (P208 did not move them). They now start below the meters, which puts them further into
  the faces. Not changed: out of scope.
- The iPhone's new radar and hint placements (F11's remaining parts) come with this change but were not
  checked on a device. F11 is left open.

## Next

Nothing for P304. F11: check the radar and the hints on an iPhone and close it.
