# 1 — splash prompt (FINAL)

Task P278. 2026-11-03.

## What I did

- Shot the splash before the change (`--menu --screen splash --shot`, headless, on a private weston) at
  1920x1080, 1280x800 and 780x1688. Measured the "Press" line in the 1080p shot with ImageMagick: the
  P's cap runs 601-649 px (mid 625) and the Enter keycap 594-653 px (mid 623.5), so the glyph is on the
  line, centred to within 1.5 px, and stands about 5 px over the caps above and below as a keycap should.
  The brief's guess (the glyph off its line) was not it.
- What does float is the hint tab. `hints` puts the tab's foot at `STAGE.1 - BAND`, the top of the black
  band the other screens draw along the bottom. The splash calls it with no band, so the tab hung 130 px
  over the bottom at 1080p, and on the phone it sat beside the logo, halfway down the screen.
- `hints` is now `hints_on(hints, STAGE.1 - BAND)`. The splash calls `hints_on(.., self.bottom())`, so
  its foot is on the frame's bottom edge at every window shape. Nothing else moves.
- Test `the_splash_hints_sit_on_the_bottom_edge` (tests/menu.rs): at 1920x1080, 1280x800, 390x844,
  844x390 and 2520x1080, the gap under the splash's `Part::Hints` box is under the box's own height, and
  the main menu's tab still sits on its band. It fails on the old code (1080p: 129.8 px over the edge)
  and passes on the new. `the_hints_and_the_band_stand_clear_of_the_touch_pad` still passes.
- After shots at the same three sizes: the tab is in the bottom right corner at each.

## How it was proved

The layout test above, and after shots at 16:9, 16:10 and a phone shape, looked at by eye (kept in the
session's scratch folder, not committed).

## Not verified / not 1:1

- Not run on a real Deck or phone. The 16:10 and phone shapes are headless windows of that size.
- The build label in the top left still sits at the stage's top (`top().max(0)`), not the frame's, so on
  a phone held upright it floats too. Not in this task's scope; filed as a follow-up.

## Next

none
