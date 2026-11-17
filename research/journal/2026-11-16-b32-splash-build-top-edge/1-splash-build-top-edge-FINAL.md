# 1 — splash build top edge (FINAL)

Task B32. 2026-11-16.

## What I did

- `menu_screens.rs` splash build label: its baseline hangs from the frame's top edge (`top()`), not
  `top().max(0)`, which clamped it to the stage's top. On a phone held upright (and on 16:10, by the
  letterbox's 59 px at 1280x800) it floated under the edge over the scene.
- Its x keeps `left().max(0)`: on a wide screen it stays over the stage's left, beside the scene, as before.
- `menu.rs`: new `Part::Build(Bounds)` so tests can find the label; `tests/menu.rs`'s exhaustive match
  takes it.
- Test `the_splash_build_label_sits_on_the_top_edge` (tests/menu.rs): at 1920x1080, 1280x800, 390x844,
  844x390 and 2520x1080 the gap over the label is under twice its height and it is inside the frame.
- opentennis: no splash build label found to copy; the fix mirrors P278's hint tab (`bottom()`).
- Filed P355: the New version card at the top right still uses `top().max(0)`, so it now sits lower than
  the label on a tall frame.

## What the original does

Menus are ours to design; the original has no build label.

## How it was proved

- The new test passes; with the old `top().max(0)` put back it fails ("at 1280x800: the build label …
  floats 59.2 px under the top edge").

## Not verified / not 1:1

- Not looked at on a real phone or in a screenshot: checked by the label's drawn bounds only.
- A phone's notch/status-bar safe area is not allowed for: the menus have no safe-area inset here, same as
  the hint tab of P278.

## Next

none
