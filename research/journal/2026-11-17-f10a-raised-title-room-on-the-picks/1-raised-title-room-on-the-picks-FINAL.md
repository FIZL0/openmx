# 1 — raised title room on the picks (FINAL)

Task F10a. 2026-11-17.

## What I did

- With web/touch.js's pad up, where the title and the hints would share the band's top past half
  their size (F10's `SHARED_SHRINK`), the Tracks and Race picks now stop their cards/rows short of
  where the raised title goes, so F10's "up a row over the hint tab" applies to them too.
  - Tracks: the cards stop above the raised title (6 cards → 4 at 667x375 and 1180x820).
  - Race: all 9 rows kept, fitted a little closer; the down arrow follows the new bottom.
- menu_screens.rs: F10's sum moved out of `Pen::hints_on` into `Pen::title_beside` (shrink, raised
  box), with `hint_widths`/`hint_spacing` and `HINT_LEAD`; new `above_the_raised_title` gives a screen
  its rows' bottom. `Pen::list` now calls `list_to` (a bottom argument); `cards` takes a bottom.
  The Tracks pick builds its hints before its cards to ask.
- Pad down, and every other size/screen: unchanged (the helper returns the old bottom).
- opentennis: no touch-pad title layout there; nothing taken.

## What the original does

Nothing: front-end layout is ours to design. No simulation, RNG or timing touched.

## How it was proved

- tests/menu.rs `the_title_stands_clear_of_the_touch_pads_stick` (ran with the disc): the floor on the
  title's and hints' size is now 0.5 at every size (was 0.5 on the P208 phones, 0.3 elsewhere). A
  throwaway print showed Tracks and Race at 667x375 and 1180x820 with the title at full size and the
  hints at 1.0 / 0.88, where they were ~0.3; least anywhere is now Pause at 0.504. The test still
  checks the raised title lies over no more rows/cards than with the pad down.
- Whole `-p openmx --test menu` via `./kit check`: 303 passed.

## Not verified / not 1:1

- Not looked at by eye (no raster probe, no real page): the geometry is checked by the test only.
- Tracks loses two cards with the pad up on those sizes; the list scrolls, but it was not tried by
  touch on a device.
- Race's down arrow is not a `Part`, so the test does not see it; by the sums its middle sits 0.7 em
  above the raised title's top (pad down: where it was).

## Next

Nothing.
