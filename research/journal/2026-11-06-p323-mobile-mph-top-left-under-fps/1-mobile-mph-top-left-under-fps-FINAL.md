# 1 — mobile mph top left under fps (FINAL)

Task P323. 2026-11-06.

## What I did

- **Which "fps line".** Nothing draws an fps or stats line at the top left on any platform. The Show
  FPS counter (`ride_hud::fps_sprites`) is drawn top right everywhere, and the phone's per-second
  stats line (P324) only goes to the log. A speedometer "under fps" at the top left therefore means
  moving the FPS line too. While the touch pad is up (web touch layout, iPhone, Android: every
  platform that calls `touch_room::set`), the Show FPS line now stands **top left, right of the score
  bar and level with its words**, and the speedometer and preload meter sit just under it, with
  their left edge on the FPS line's. With no pad shown (desktop, or a real pad on a phone), both stay
  exactly where they were.
- `ride_hud.rs`:
  - `to_the_top_right` (P304) is replaced by `to_the_top_left`.
  - `fps_by_the_score` is the one shared placement for the FPS line. It reserves the width of the
    widest text, "000 FPS", so the layout doesn't jump when the count changes. It also reserves the
    spot when Show FPS is off, so the meters don't move when the setting is toggled.
  - `fps_sprites_clear_of(…, pad, …)` is new, and `fps_sprites` now calls it with `None`.
  - `top_right` loses the `below` argument that P304 added: the meters no longer sit over that
    column.
- **Where the line can't fit beside the score bar.** On a tablet (1180x820) "000 FPS" right of the
  score bar would run into CAMERA. There the FPS line drops under CAMERA and PAUSE, right of the
  score bar and of CLUTCH. The meters take one of two places: just under the FPS line (kept left of
  CAMERA), or under CAMERA/PAUSE (allowed out to the face diamond and the view's middle). They take
  whichever holds them biggest, and shrink only when neither holds them whole. They always stay in
  the view's top-left quarter.
- `touch_room.rs`:
  - New: `under_the_top` (the bottom of CAMERA/PAUSE), `left_of_the_camera`, and `clutch` (CLUTCH's
    right edge and bottom, from touch.js's `left: l + .4u; top: t + 12vh`, 1.6u × 0.7u).
  - Removed: P304's now-unused `over_the_faces` and `right_of_the_top`, with their constants.
- `main.rs`: the GPU frame's Show FPS draw passes `touch_room::Room::of(frame)`. The CPU path
  (`fps_draw`) still passes no pad. The software HUD never took the pad (P208), so the two stay
  consistent.
- Split screen with the pad up: as in P304, only the view at the frame's bottom right moves its
  meters. If that view starts below the frame's top, they go right of that view's score bar, level
  with it, since there is no FPS line there.
- opentennis: nothing to take. It has no touch HUD.
- `overlay.rs` (named in the brief): no change. Its original-art HUD is not drawn since P170 (see
  P304's journal).

## What the original does

Nothing. The original has no touch pad, so this is front-end layout and ours to design.

## How it was proved

- `tests/ride_hud.rs::with_the_touch_pad_up_the_meters_sit_top_left_under_the_fps_line` replaces
  P304's top-right test. It covers 844x390, 932x430 and 1180x820 page windows, and a 2556x1179
  iPhone 15 Pro at 3x with 177 px island insets. Both speedometer styles are checked:
  - The FPS line is top right with no pad. With the pad it is in the top-left area, and under none
    of the ten pad controls (laid out from touch.js's own numbers in the test).
  - The meters move, and nothing else does. They land in the top-left quarter, start at or right of
    the FPS line's left edge and right of the score bar, and sit under the FPS line, within a fifth
    of the height.
  - None of their sprites is under a control.
  - With no pad, the HUD and the FPS line are identical to the plain `sprites`/`fps_sprites`, so
    desktop is unchanged.
- P208's `the_speedometer_and_the_radar_stand_clear_of_the_touch_pad` still passes unchanged.
- `touch_room` unit test: the new places on the 844x390 phone.
- **Picture.** A throwaway probe test (since removed) rastered the HUD and the FPS line with the
  software path, over dark boxes where touch.js's controls are:
  - 844x390, touch, digital: FPS beside the score bar, "14" + meter at about (165, 40) under it.
  - 844x390, touch, analog: the dial at full size under the FPS line.
  - 844x390, no pad: unchanged, FPS top right and meters bottom right.
  - 1180x820, touch, analog: the FPS line under CAMERA, the dial (×0.92) under it.

  None of them touches a control.

## Not verified / not 1:1

- **No real ride shot on a phone, browser or emulator.** The task's check asks for "a phone-layout
  ride shot". The shell has no flag to force the touch layout on the desktop, and an
  iPhone/Android/web run with the pad up was not set up in this session. What I looked at is the
  software-rastered HUD alone, over plain ground with the pad's boxes drawn in, not a world frame.
- **FPS line moved, not only the mph.** The user's words assume the fps line is already top left. It
  is drawn top right, so on the touch layout I moved it to top left as well, to make "mph under fps"
  hold. If the user meant "top left" and nothing more, the FPS line's move is the part to undo.
- **Web safe-area insets are not known to `touch_room`** (as in P304). On a browser phone with
  insets, the frame's top-left corner could sit under a notch. iOS lays the frame inside its kept
  insets, and the iPhone case in the test shows that.
- The iPhone `Room` sizes `u` from the inset room rather than the whole window (as in P304). That
  only matters on width-bound, iPad-like windows.
- The meter spot stays reserved under the FPS line when Show FPS is off. That leaves a small gap
  beside the score bar. I chose a stable layout over a tighter one.
- In a split view with the pad up, only the bottom-right view's meters move (as in P304). The pad is
  only up with no controller connected, so that case is rare.

## Next

Nothing for P323. F11 (checking the touch HUD on an iPhone) should now look at this top-left
place.
