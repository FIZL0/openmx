# 1 — hud redo (FINAL)

Task P121. 2026-10-23.

## What I did

- `crates/openmx/src/ride_hud.rs`, new: the ride HUD of `docs/hud-concept.html` as `ink` sprites
  (rounds, rings, discs, bars, capsules and SDF type), one call for the GPU (`sprites`) and the same
  sprites rastered by `ink::raster` for the software path (`draw`), so the two are one picture.
  Everything is placed in the disc's 640x448 and scaled by `min(w/640, h/448)` (times the HUD scale)
  to each view, each piece held to its corner as the disc's layouts hold them:
  - score bar at the original's SCORE_BAR (14,25 168x20.5, darker label end to x 82), grey SCORE and
    the banked total (`rider.score.total`, grouped with commas), an orange ring and orange-to-grey
    number for 0.7 s when the total goes up; MODIFIED and the rider tag under it.
  - trick pop-up at 320,373: the chain's names (latest three, `…` before) joined by `+` over the
    points and `xN` from two tricks on, orange ringed dark; the newest name and the points pop in from
    2.0 to 1.0 with alpha over 0.3 s; an ended combo plays out 1.0 → 2.0 while fading over 0.3 s. A
    landing first holds `+banked` for 1 s. No LANDED / BAILED word.
  - speedometer at `speedometer.lyt`'s places (Place::Full): Digital (default) the DSEG7 Classic
    Bold Italic number with its right at x 588 and its top 2 px under the meter's top, no unlit
    segments; Analog the dial on the original's needle angles (white dash ticks and numbers 0–100,
    orange needle, grey hub, `%07.1f` odometer capsule under it). The preload meter (V53 fill) right
    of either on the bike, filling from the bottom. A split view is digital, as the original's is.
  - radar at the original's 87,363, r 52: dark disc, the two axes and N/E/S/W outside the rim
    turning with the heading (`overlay::radar_turn`), zone letters in their activity colours,
    machines and bikes as coloured rounded squares, P88 markers as numbered slot-colour discs, other
    riders as the original's human/AI dot colours, rim chevrons for anything clamped, the player's
    arrow fixed up.
  - prompts low and small at the bottom's middle: the machine switch's `offer_text` question with a
    black Circle "Accept" pill; a challenge's "Hold to start" pill with a dashed ring filling.
  - marker menu top right, replacing `hud::draw_markers` under the ride HUD: dark panel, MARKERS and
    the machine, the five slot discs (filled = set, the picked one grown and ringed), the other
    players' pills with names, the label ("Marker 2 empty" / "to Ann P2"), the hold bar over HOLD_MS
    (green once fired) and the d-pad legend. Same MarkerShown rules as the bar, per machine (P107's
    set is the one `shown_markers` already picks).
  - camera toast under the marker column for 1.5 s on a camera change (not the ride's first camera).
- `menu_art.rs`: `ink::digits()`, a second SDF atlas of DSEG7 (`assets/fonts/DSEG7Classic-
  BoldItalic.ttf`, OFL, licence beside it) over `0123456789.-: `; `Type::of` builds either atlas;
  `Type::top` (a glyph's top) for placing type by its cap height.
- `menu.rs`: `Overlay::Hud` (word `hud`, label HUD) is the default and first in `ALL`
  (Hud, Original, Debug, None). Migration: a settings file with no `hud` line saying `original`
  reads as Hud once; `hud = 1` is written beside `original` only (as other one-case lines are), so a
  file saved since keeps Original and every other file round-trips word for word.
- `menu_pages.rs`: the HUD row's help and tips name the four; the Speedometer row dims only under
  Debug and None.
- `main.rs`: `--overlay hud|original|debug|none`; the two atlases go after the original's HUD
  images (menu base moved past them); a per-view `RideHud::with` steps the odometer, updates the
  view's `ride_hud::Tracker` from `seat.sim.rider.score` with `Stunts::display_name`, and builds the
  `Shown`; GPU and software paths draw it when the overlay is Hud and the debug HUD is not on.
  Original and Debug keep `hud::draw_markers`; `rig.ride_hud.note_camera` on each camera change.
- Tests: `tests/ride_hud.rs` (new, 9 tests + the mounted modules' own); `tests/menu.rs` three
  assertions moved for the new default (one more Right to reach Debug; pre-V26 on/off now land on Hud).

## What the original does

The stunt overlay's HUD listener answers a combo's end — event 2 landed, 3 bailed — the same way:
it sets the Stunt movie's `Mode` to 1 and plays it, which pops the orange text out. Nothing turns it
green or red and no word is drawn; a landed combo of two or more first shows "Stunt Combo Total".
So `LAND_COLOUR == BAIL_COLOUR == ORANGE`, and what tells them apart in the port is the `+banked`
line a landing holds a second. Recipes in `research/oracle.txt`'s P121 block (`./kit fn` on the
listener's boot and tick, `./kit data` on the "Mode" string).

## How it was proved

- `./kit check`: TESTS PASSED (the gate's count is shared, so not quoted).
- `tests/ride_hud.rs`: every piece at its layout place at 1920x1080 and 3840x2160 (score bar, meter,
  readout right edge and top, radar centre/radius/compass, pop-up centred low, prompt low/small,
  marker menu right edge); a split view's pieces at the view's own scale and corners; landed and
  bailed chains in their colours with no word, with orange pixels counted in the rastered frame, the
  hold and play-out timings; the pop-in; Digital is DSEG7 (the digits atlas' sprites) left of the
  meter and Analog the dial at the original's pivot with the odometer; the marker menu through all
  five slots and two players as `Markers::at` steps, label and grown slot each time, the hold bar at
  0/¼/½/1/over HOLD_MS; the camera toast; the default and migration; text edges at 4K (score number
  and DSEG7 readout: under 10% of edge pixels lack both a full and an empty neighbour); and the shell
  run with each `--overlay` on both renderers, each drawing something different from `none` and
  the ride HUD's score bar and meter where the layout puts them.
- Looked beside the concept and PCSX2: `context/shots/P121/` (git-ignored; copy from slot s3):
  `ride-shot-1080p.png` is the shell's own `--shot` (taped ride on free06, marker menu open after a
  d-pad Right); `ride-composed-1080p.png` is `ride_hud::draw` with a two-trick combo, the menu and
  the switch prompt over that ride's clean (`--overlay none`) frame; `concept.png` is
  `docs/hud-concept.html` in headless chromium; `pcsx2.png` is V27's PCSX2 frame of the original;
  `side-by-side.png` the three (concept · port · original).

## Not verified / not 1:1

- Land/bail colour is from reading the listener's code, not from frames of a captured bail; the
  original's "Stunt Combo Total" line is not drawn (the port shows `+banked` instead).
- The shell shot has no combo: a taped free06 ride (gas, P30's backflip stick schedule, both stick
  signs, a held wheelie) never opened the port's score card in the shell, so the combo and prompt in
  the composed shot are drawn by the same code but fed by hand. Whether the shell's rider scores at
  all on a real jump was not looked into (the old `gauges()` comment says the port keeps no score).
- The PCSX2 picture is V27's old frame (no combo, no prompt); no new PCSX2 capture was taken.
- HUD scale and Minimap settings do not exist yet (V52 is not done): `Shown::scale` is 1.0 and
  `Shown::minimap` true from main.rs; both are honoured by `ride_hud` once wired.
- The challenge prompt is drawn and tested only through `Shown::challenge`; no activity is ported,
  so main.rs passes `None`.
- The port has no AI riders; every other rider is a human-coloured dot.
- Split views use `speedometer.lyt`'s Full places scaled to the view, not the original's
  `speedometerhorizontal*.lyt` places.
- The concept's camera toast shoulder chip is left out; the player's radar arrow is an isosceles
  arrowhead (no notch); machines on the radar are coloured squares without their icons.
- Original keeps the old `hud::draw_markers` bar; only Hud has the new marker menu.
- The concept's "Biplane parked here" machine card is not used: the brief's question and Accept
  pill (P103's `offer_text`) are.
- "Follows presses": the test steps `Markers::at` (what Left/Right change) rather than riding pad
  presses through `markers::frame`; tests/markers.rs already holds the press-to-`at` part.

## Next

Nothing for this task; P129 follows up the shell ride that never scores.
