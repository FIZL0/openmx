# 1 — dynamic aspect (FINAL)

Task V24. 2026-10-20.

## What I did

- **The frame fills the window** (`--aspect fill`, the default): `crates/openmx/src/aspect.rs`'s
  `Aspect::frame` is the window's own size over `--scale`. `--aspect 4:3` is the shell as it was:
  `fit_frame` (moved from window.rs into aspect.rs, unchanged) and bars. A resize mid-ride re-makes
  the frame and the views, as before.
- **Each view draws at its own shape.** The field of view is `game_render::world::view_fov` of the
  view's own pixel shape (new beside `vertical_fov`, which is unchanged); `Camera::chase` uses it too.
  The split views were already drawn into frames of their own size (V22), so with the frame no longer
  forced 4:3 nothing is squashed — the test reads the angles back out of the projection to prove it.
- **The disc has its own widescreen mode, and the port follows it** instead of the brief's default
  Hor+ (see below). So the field-of-view rule did not change inside the shapes the original drew:
  the preset's horizontal angle is held, and a wider screen sees less up and down (Vert-, as the
  original's own 16:9 mode). Outside 2:3..32:9 — shapes the original never drew — the short side's
  angle holds at the range's edge and the long side widens: Hor+ past 32:9 (a 21:9 window's stacked
  strip), and a portrait window narrower than 2:3 holds 2:3's vertical angle and narrows sideways
  instead of opening into a fish-eye.
- **The 16:9 display**: the frame stands for whichever of the original's two display aspects its
  shape is nearer (geometric middle, ≈1.54: 16:10 and 21:9 are wide, 3:2 and portrait are 4:3).
  On the 16:9 display the cameras read the presets' wide copies (`Preset1w` for the full screen and
  the stacked split, `Preset1x` side by side, `Preset1z` — not on the disc, so the full screen's —
  for the port's quarters and eighths), and the stacked split's short-viewport pitch trim is off,
  as in the original. A resize across the middle reloads every camera's presets for the new display
  and re-sets the current preset (main.rs `set_display`), the original's own reload when a split
  changes. Seen live: the tiling WM shrank a 1920x1080 window to 936x1018 two frames in and the
  shell printed `display: 4:3` and reloaded the `h` copies.
- **The HUD** is pinned to its view's bottom-left corner by `aspect::corner` (`hud::panel_at`): its
  own size, never stretched, in every view of every shape. A view too short for it keeps the top
  (the numbers) and clips the bottom — before, it clipped the top.
- opentennis (`crates/hst-base/src/widescreen.rs`) does Hor+ in Bevy and keeps a 4:3 HUD box with
  pinned edge pieces; nothing there to copy for a software rasteriser beyond the idea of pinning to
  the view's edges and an Auto/4:3 choice. Its 16:9/16:10/21:9 fixed choices were not taken: the
  brief asks for `4:3|fill`.

## What the original does

- Video modes 480i, 480p, 480p widescreen, 720p, 1080i, PAL, PAL60; the display setter files 3–5
  under display aspect 3 (16:9) and the rest under 2 (4:3) (`./kit fn 0x00312410`), and computes the
  per-aspect shape terms as `(render h / display h) × {1, 4/3, 16/9}` (its last three lines).
- The view arranger hands each view the shape term of the current display aspect (or 16:9 if the
  wide flag in the viewport-mode object is set) through the viewport's vtable +200
  (`./kit fn 0x0020aaa0`, lines 43–47).
- The viewport's shape term is `(h/w) / (aspect × fbh/fbw)` (`./kit fn 0x00112670`), and the
  projection is built from the preset's *horizontal* angle and `1 / term` (oracle.txt,
  viewport_build_projection / frustum_half_angles). So in both displays the horizontal angle is the
  preset's and the vertical angle follows the displayed shape of the view.
- The camera loader's suffix choice by split mode and display aspect is R9's (oracle.txt,
  camera_load_presets and the R9 section), and the short-viewport skip at aspect 3 is in the race
  camera's aim (oracle.txt, `short_viewport` note).

## How it was proved

- `crates/openmx/tests/aspect.rs` (7 tests): frame sizes for 4:3 (1440x1080), 16:9, 21:9 and a
  portrait 1080x1920 window at fill and 4:3; each view's size alone, stacked, side by side and in
  quarters; the horizontal and vertical angles **measured off `Camera::view_projection`** for every
  one of those views (square pixels in every view; 85° across inside 2:3..32:9; the held vertical
  past each edge; 68.997° down at 4:3, the save state's number); the display classification; the
  loader's suffixes; the short-viewport trim; the four corners in every view.
- `hud.rs` `the_panel_is_pinned_to_each_views_own_corner`: draws the overlay into every view of
  those windows and checks the changed pixels are exactly the panel's rectangle at the bottom-left.
- Looked at headless shots at 1920x1080, 640x480, 2560x1080 split 2 and 1080x1920 split 4: no
  squash, HUD in each view's corner. A windowed run on :0 resized itself mid-ride and swapped
  displays as above.
- Simulation untouched: the files changed are the shell, the HUD and the renderer's field of view;
  `./kit check` passes the replay tests.

## Not verified / not 1:1

- **The original's 16:9 mode was not run.** That it keeps the horizontal angle is read from the
  decompilation (the shape term and the projection), not from a widescreen save state or screenshot;
  no 16:9 state exists. The aspect-table values are computed at runtime (the image holds zeros at
  them), so the 4/3 and 16/9 factors are from the decompiled setter, not a RAM read.
- **The 4:3/16:9 split at ≈1.54 is the port's choice.** The original only knows its video modes;
  a 16:10 or 3:2 window has no original counterpart.
- **Outside 2:3..32:9 the field of view is the port's own** (Hor+ past 32:9, held vertical below
  2:3); the original never drew those shapes.
- **`Preset1z` fallback**: the original leaves a missing section's record as it was (whatever was
  loaded before); the port falls back to the full screen's copy, as V22 already does for `q`.
- **A display swap clears the player's camera nudges and re-sets the preset**, which is what the
  original's split-change reload does; whether the original ever reloads on a *display* change
  mid-race (it cannot change video mode mid-race) is moot — the swap is the port's.
- **The wide flag** in the original's viewport-mode object (forcing 16:9 regardless of video mode)
  has no setting here.
- **P36's settings and front-end layout at any shape: not done** — P36 (the front end) does not
  exist yet, so there is no settings screen to put `--aspect` in and no screens to lay out. P36 should
  carry `Aspect` as a persisted setting and lay its screens out with `aspect::corner`.
- The overlay is the shell's own debug panel; the original's HUD is not ported, so its anchoring is
  not compared with anything.

## Next

Nothing for V24 itself. P36 should add `--aspect` to its settings file and lay out with `aspect::corner`.
