# 1 — splash new version top edge (FINAL)

Task P355. 2026-11-17.

## What I did

- The splash's New version card (`new_version`, menu_screens.rs) now sits at the frame's top edge
  (`top() + 1.2`), not the stage's (`top().max(0.0) + 1.2`), the same as the build label beside it (B32).
- New `Part::Update` reports the card's box, so the layout test can find it.
- New test `the_splash_new_version_card_sits_on_the_top_edge` (tests/menu.rs): at 16:9, 16:10, a phone held
  both ways and 21:9 the gap over the card is under a fifth of its height and the card stays on the frame.

## What the original does

Nothing to port: the menus are ours to design. The card comes from opentennis's B581.

## How it was proved

- The new test passes, and does not skip (it needs the disc's menu art; run with `--nocapture`, no
  `skipped:` line).
- With the old `top().max(0.0)` put back it fails: at 1280x800 the card floated 59.2 px under the top edge.

## Not verified / not 1:1

- Not looked at on screen or on a phone: only the drawn boxes were checked, not a rendered picture, as the
  card only shows when a newer build is out and no run here forces one on screen.

## Next

none
