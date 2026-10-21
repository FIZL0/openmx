# 1 — the originals hud shows (FINAL)

Task V28. 2026-10-20.

## What I did

- **Why the HUD did not show.** A direct `--ride` (headless and windowed, GPU and software) already
  drew V26's HUD. Two things hid it on a normal launch:
  1. The settings file beside the disc. P36 wrote `overlay = on`/`off`; V26 read `on` as its new
     *debug* overlay, so anyone who had saved settings before V26 (the main checkout's
     `settings.txt` says `overlay = on`) got the debug text and never the original's HUD.
     `menu::Overlay::from_word` now reads both old words as `original`; `debug` is only `debug`.
  2. On the GPU the pause menu (and any menu over a ride) is a patch, and `write_overlay` writes each
     patch's rectangle whole, so the full-screen menu patch replaced the HUD's patches: pausing wiped
     the HUD. `layer::over` folds the earlier patches into the menu's so the HUD is under the menu,
     as on the software renderer.
  Art::load failing used to print to stderr and ride on without a HUD; it now stops the ride with
  the reason (`the original's HUD: …`). The disc here loads it; the GPU patch path, the software
  path, 2 and 4 players and the biplane all draw it.
- **The rest of the original's riding HUD.** From PCSX2 frames of first_recording (bike) and the
  biplane state: a SCORE bar top left, the radar, the dial with odometer, and a preload meter — a
  dark upright capsule right of the dial. The biplane shows score, radar and dial only. Added: the
  score bar (Flash movie `Overlay_1p_Score.fsh`: tints and text colour read out of its bytes, label
  `eurostileboldbold_12`, number `eurostileboldbold_14`), the preload meter (capsule, tint from
  `ingame_preloadmeter.fsh`), odometer and meter only on the bike. `overlay::Gauges` carries what
  the view's rider shows; `main.rs::gauges` fills it from the seat.
- Tests: `tests/hud.rs` rides the shell itself (`--headless --size 640x448 --frames 2 --shot`),
  default overlay against `--overlay none`, and asserts over 10% of the speedometer's and the
  radar's boxes are drawn on — GPU and software, riding and paused. `tests/menu.rs`: `on`/`off`
  read back as `original`. `tests/overlay.rs`: score drawn, meter on the bike only, the plane with
  no odometer, `preload_fill` cases.

## What the original does

- Score bar and preload meter are Flash movies on the HUD layer; the speedometer's update binds the
  meter movie's `preloadMeter` variable (`./kit xref 0x436f78`, used from `./kit fn 0x2abcf0`).
  Three visibility setters sit beside it (`./kit fn 0x2ac190`, `0x2ac200`, `0x2ac2a0`). The
  "Preload Movie" popup (`./kit fn 0x29aad0`) is the tutorial tip, not the meter. Notes in
  research/oracle.txt under V28.
- Radar colour tables: AI marks `00 00 ff ff`, human `00 ff ff ff` (RGBA memory order, checked on
  screen); activity icons H/M/S/R in four colours. See the V26 lines.

## How it was proved

Looked right beside a screenshot: the port's 640x448 frame beside PCSX2's of first_recording's
state, the score bar and dial stacked at 4x and tuned by eye (label and number tops within a pixel).
`tests/hud.rs` (4 rides each test) proves the HUD is drawn on a default launch and under pause on
both renderers; it wrote its frames to `target/tmp/v28-*.png`, which were looked at.

## Not verified / not 1:1

- **The score is always 0.** The port has no stunt scoring; `Gauges::score` is wired but nothing
  feeds it.
- **Score bar and preload meter are drawn by hand, not played from their .fsh.** No Flash player in
  the port. The shapes, tints and colours are the movies'; positions and sizes are measured off a
  frame. The `SCORE` label draws about 9 px wider than the original's (the original's glyphs sit
  closer; the cause — a smaller text size or letter spacing in the movie — was not found).
- **The preload meter's fill is invented.** The meter was only ever seen empty: holding the d-pad
  for a preload broke PCSX2's screenshot hotkey, so a filled meter was never captured. Its colour
  (orange), inset and the formula (preload force / max × timer / time, only while firing) are
  guesses, and what the original feeds `preloadMeter` was not followed.
- **Radar activity icons and the red dots** on the original's radar are not ported (no activity
  data in the port).
- **Split screens** put the score at the view's top left and draw **no preload meter**: the
  original has per-split layouts (`speedometerhorizontal1` moves the meter by y −194) that were not
  followed, and at the dial's place the meter covered the split's speed number. Not compared
  against an original split frame.
- **Online** was not run separately: it rides through the same `ride()` and overlay code.
- The biplane's HUD (no odometer, no meter) matches one original frame; which visibility setter
  hides which part was not pinned.
- opentennis was not used: nothing in it bears on a Flash HUD.

## Next

Feed the score when stunt scoring is ported; capture a filled preload meter (a generated .p2m2
holding the preload button, screenshot from the app) to replace the guessed fill.
