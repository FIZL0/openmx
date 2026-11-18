# 1 — hints clear of the stick (FINAL)

Task F10b. 2026-11-17.

## What I did

- With web/touch.js's pad up, the hint tab's room now starts right of the stick's resting base, not
  at the frame's left (menu_screens.rs `Pen::hint_shrink`, used by `hints_on`). P208 only kept the
  hints left of the pedals.
- That alone left the Controllers hints at ~0.35 and Race's at ~0.39 of their size on 667x375 and
  1180x820 (F10a's floor is half). So a hint word may carry a short form after `|`
  (`Pen::hint_words`): used only while the pad is up and the long words would shrink past half.
  - Controllers: Done / Off / Join, and Move is left out (empty short form). The stick still moves a
    card, and a touch off the pad drags it (P214's drag).
  - Race: Supercross / National → Series.
  - Every other screen and size, and pad down: the long words, as before.
- Least hint/title size anywhere with the pad up is now Pause at 0.504 (F10a's); Controllers ≥ 0.6.
- opentennis: no touch-pad hint layout there; nothing taken.

## What the original does

Nothing: front-end layout is ours to design. No simulation, RNG or timing touched.

## How it was proved

- tests/menu.rs `the_hints_and_the_band_stand_clear_of_the_touch_pad` now also asserts the hints miss
  the stick. Before the fix it failed: Tracks at 1180x820, hints [170.8, 635.8, 418.0, 26.3] under the
  stick [41.3, 563.9, 214.8, 214.8].
- `the_title_stands_clear_of_the_touch_pads_stick` (half-size floor) failed at 0.349 with only the
  room change; passes with the short words.
- Whole `-p openmx --test menu` via `./kit check`, with the disc: 303 passed.

## Not verified / not 1:1

- Not looked at by eye (no raster probe, no real tablet): geometry is checked by the tests only.
- The test's stick box is its own copy of touch.js's sizes, as in P208/F10; not checked against a
  live page.
- Dragging a Controllers card by touch on a real tablet was not tried; Move's hint is gone there on
  the strength of P214's drag and the stick.

## Next

Nothing.
