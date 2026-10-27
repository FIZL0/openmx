# 1 — aspect ratio options (FINAL)

Task P176. 2026-10-26.

## What I did

- Took opentennis's Aspect Ratio choices (`crates/hst-base/src/widescreen.rs` `ASPECTS`: Auto, 4:3,
  16:9, 16:10, 21:9, Stretch). Only the list and the "largest centred rect of the ratio, bars round
  it" rule came over; opentennis's Bevy view rects, safe-area insets and `Bleed` copies have nothing
  to land on here, because the shell already draws the 3D, HUD, menus and every split view into one
  frame and the presenters (`window.rs`, `gpuwin.rs`, web) centre that frame with black round it.
- `aspect.rs`: `Aspect` gains `Wide` (16:9), `Monitor` (16:10), `Cinema` (21:9) and `Stretch`;
  `Aspect::ratio`, `Aspect::ALL`, and `fit_ratio` (the old 4:3 `fit_frame` math for any ratio;
  `fit_frame` is now `fit_ratio(.., (4, 3))`, same numbers). `--aspect` takes `auto`/`fill`, `4:3`,
  `16:9`, `16:10`, `21:9`, `stretch` (also `16x9` etc.).
- `Display::of` is unchanged: 4:3 still forces the television; every other choice goes by the frame's
  shape, so the fixed 16:9, 16:10 and 21:9 frames are all the wide display (and read the `w`/`x`/`z`
  presets), as the brief asks.
- Settings: `menu::Picture` has the same six (settings.txt words = `--aspect`'s; old `fill`/`4:3`
  files still read). The Display tab's new last row, Aspect Ratio (`Key::AspectRatio`), is now the
  Picture departure's switch, as Draw Distance is the fog's: `Departure::Picture` moved its tab to
  Display and its key to `AspectRatio`, and the Graphics tab's two-choice Picture Shape row is gone
  (one row for one setting). The Original switch still sets 4:3, and back to ours is Auto.
- Split screen: `Layout::rects` was already fed the frame, not the window, so the views follow the
  fixed frame (seen: 21:9 side by side, 960x822 each in a 1920x1080 window).

## What the original does

Nothing new: the original only has its 4:3 and 16:9 video modes (aspect.rs's module doc). The other
shapes are a presentation addition, off by default (Auto).

## How it was proved

- `tests/aspect.rs` `each_fixed_shape_is_the_largest_one_inside_the_window`: every fixed shape in
  the 4:3, 16:9, 21:9 and portrait windows, exact frame sizes, touching two window sides, its ratio to
  the pixel, its display, and the four split layouts tiling the fixed frame; every word parses.
- `tests/menu.rs` `the_display_tab_has_opentennis_aspect_ratios`: the row, its six labels, the pad
  stepping through all six, each saved and read back, the departure going to 4:3 and back to Auto.
  Two existing menu tests updated for the row's new place and six choices.
- Ran the shell on the 1920x1080 X display, 2 players side by side: `--aspect 21:9` drew 1920x822
  (16:9 display, views 960x822), `16:10` 1728x1080 (views 864x1080), `stretch` 1920x1080; a resize
  to 936x500 re-made the 21:9 frame at 936x401.

## Not verified / not 1:1

- Stretch is the same as Auto here. In opentennis it is Auto with the safe-area insets dropped
  (B624); the openmx shell has no safe area or UI margin, so there is nothing to drop. I did not
  make it a distorting stretch of the 4:3 picture, since opentennis's Stretch does not distort
  either. The row's help says it fills the window as Auto does.
- The bars were not checked by eye in a screenshot: `--shot` writes the frame, not the window,
  and grim grabs the lock screen here (memory). The frame sizes printed by the shell, and the
  presenters' existing centring (unchanged since V24's 4:3 bars), are the evidence.
- The web build was not run; it uses the same `Aspect::frame` path through `frame_size`, but I did
  not open it in chromium.
- Portrait windows letterbox at every fixed shape, as 4:3 already did. opentennis gives a portrait
  window the whole window at any ratio; I kept openmx's existing 4:3 rule rather than change it.

## Next

Nothing.
