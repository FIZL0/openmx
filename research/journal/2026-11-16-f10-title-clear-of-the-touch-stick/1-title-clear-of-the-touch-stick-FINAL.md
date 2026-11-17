# 1 — title clear of the touch stick (FINAL)

Task F10. 2026-11-16.

## What I did

- With web/touch.js's pad up, the screen's big slanted title (menu_screens.rs `Pen::title`) now starts
  right of the stick's resting base (the same `clear_of_the_stick` P208 gave the band's line).
- The title is held back while the pad is up and drawn with the hints (`Pen::hints_on`), so the two can
  share the band's top between the stick and the pedals:
  - both fit: unchanged sizes, the title right of the stick, the hints left of the pedals;
  - else both shrink by the same factor, half an em apart;
  - past half size (`SHARED_SHRINK`), the title goes up a row over the hint tab at full size (or as big
    as fits up to the pedals), and the hints keep P208's layout. It does that only when the raised
    title lies over none of the screen's rows, pills or cards drawn so far; else they share anyway.
- A screen with a title and no hints draws the held title in `Pen::finish` (called from menu.rs's
  layout after the screen).
- New `menu::Part::Title` (the title's box with its slant and shadow), for tests.
- With the pad down nothing changes: the title is drawn at once where it was.
- No change to web/touch.js was needed: touch_room.rs already reads its numbers.
- opentennis: its touch menus solve a different problem (menu scale); nothing taken.

## What the original does

Nothing: front-end layout is ours to design. No simulation, RNG or timing touched.

## How it was proved

- tests/menu.rs `the_title_stands_clear_of_the_touch_pads_stick` (needs the disc; ran with it): every
  `Screen::ALL` at 844x390, 667x375, 932x430 and 1180x820 with the pad laid out from touch.js's own
  numbers. With the pad up the title is in the frame, off the stick and every right-hand control, off
  the hints, and over no more rows/pills/cards than with the pad down. On the P208 phones (844x390,
  932x430) the title and the hints stay ≥ half their size; anywhere ≥ 0.3. With the pad down the
  title does lie on the stick (so the test is not vacuous).
- Whole `cargo test -p openmx --test menu` via `./kit check`: 299 passed, 1 ignored.
- By eye: a throwaway probe rastered Settings, Assign, Tracks and Pause at the three sizes, pad up and
  down, with the pad's boxes drawn on (PNGs in the session scratchpad, not kept). At 844x390
  "Settings" sits right of the stick beside smaller hints; at 667x375 it goes up a row over the tab.

## Not verified / not 1:1

- Not looked at on the real page (headless chromium with the wasm build): tests/web/gpu.mjs, which P208
  used, is gone, and the layout code is the same the test rasters.
- Tracks and Race at 667x375 and 1180x820: the raised title would cover their cards/rows, so title and
  hints share the band's top at about 0.3 of their size; small. Filed as F10a.
- On a tablet (1180x820) the Controllers screen's long hints run left under the stick: P208 kept the
  hints clear of the right-hand controls only. Not this task's; filed as F10b.
- The raised title on Controllers at 1180x820 touches the bottom edge of the 5P/6P seat boxes, which are
  not `Part`s, so the test does not see them.
- The overlap check sees only rows, pills and cards pushed as `Part`s before the hints; other drawn
  things (panels, stills, text) are not checked against a raised title.
- Safe-area insets: still unknown to the shell, as in P208.
- The iPhone app's own pad sets the same room (touch_room::set), so it gets this too, but not run there.

## Next

Nothing; follow-ups filed.
