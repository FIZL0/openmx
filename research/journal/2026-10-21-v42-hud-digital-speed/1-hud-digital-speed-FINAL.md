# 1 — hud digital speed (FINAL)

Task V42. 2026-10-21.

## What I did

- `crates/openmx/src/overlay.rs`: the speed is the original's digital readout (yearlingdashboldoblique_55,
  colour 204,204,204) by default, with the preload meter beside it, in every view. A new `Place` (Full,
  Top, Bottom, Side) says which of the disc's speedometer layouts a view takes, from the view's size
  against the window's: `speedometer.lyt` for one view, `speedometerhorizontal1/2.lyt` for the top and
  bottom of a stacked split (and the rows of a quad or more), `speedometervertical1free.lyt` for a
  side-by-side split. `Drawn::place` reports it. The readout is right-justified with its right edge at the
  layout's X (the last glyph's box edge, its negative gap to a next pen not counted). That proved true
  against the original (below). The odometer keeps V26's "last character starts at X" rule. `Gauges` has
  a `speedometer` style. Analog draws today's dial, needle and odometer, but only in a single full view
  for one rider. Splits are always digital, as the original's code forces with two players or more. The
  digital layout has no odometer: speedometer1.lyr's "Speedometer Digital1" container holds the text alone,
  and `SpeedometerBackground_digital` is in no container. The plane shows the speed in the chosen style with
  no meter and no odometer.
- `crates/openmx/src/menu.rs`: `Speedometer { Digital (default), Analog }`, and a "Speedometer" row in
  Settings under Render scale (row 9; Game Modifiers stays last). Left, Right and Cross cycle it. It is
  saved as `speedometer = digital|analog` in settings.txt, and an unknown word reads as the default.
- `crates/openmx/src/main.rs`: `--speedometer digital|analog`. When it is unset, a ride launched from the
  menu takes the setting, and a direct launch takes the default, the same as `--overlay`. A Settings change
  during a ride applies on the next frame (`front.menu.changed()`, like the overlay mode). Both renderers
  get the style through `gauges()`.
- Tests: `tests/overlay.rs` has the new `a_fresh_settings_file_rides_with_the_readout_and_the_meter`. A
  fresh `Settings::parse("")` draws the readout ending at the layout's X and the meter beside it (its
  pixel bounds within 2 px of the layout-derived place) in one 640x448 view, in a 640x224 top and bottom
  view and in both 320x448 side views. It also checks the plane: readout alone, or the dial with no
  odometer. `the_views_draw_the_riders` covers analog and digital, and that a split asking for analog gets
  the readout and meter. The GPU-vs-CPU test runs analog, digital, quad and side-by-side. `tests/menu.rs`
  has the new `the_speedometer_row_switches_the_dial_and_is_saved`, which drives the row by pad
  (Right/Right/Left/Cross), checks `take_changed`, the saved text and a save/load round trip. `tests/hud.rs`
  (V28's shell launch test) now looks for the readout and meter's box by default, plus one
  `--speedometer analog` launch for the dial's box.
- opentennis: nothing new taken. The Settings row follows menu.rs's existing rows, which already follow
  opentennis's settings store.
- Disk housekeeping: my temp quota was full. I deleted an old V25 session's 5.2 GB cargo target dir from
  this slot's Claude temp folder. It was rebuildable output, untouched since 2026-10-20.

## What the original does

See research/oracle.txt "V42". `./kit fn` on speedo_mode and speedo_update shows the mode word (1 =
digital) and the players byte that forces digital at 2 or more. The mode is applied when options are
applied, not per frame: a PINE write of the word changed nothing on screen. The original's pause page
Player Options > Speedometer Display (Digital/Analog) switches it. I switched it to Analog and accepted, then
to Digital and accepted.

## How it was proved

- Looked right beside a screenshot: PCSX2, freestyle (state slot 7), digital set through the original's own
  Player Options. The original shows the readout "51" with ink at x 539–587, y 344–375 (640x448), the meter
  at 599,314, and no dial, odometer or plate. I rendered the port with the same text, "51": ink at 535–586,
  y 343–377, with the meter at the same place. The first try, with the odometer's last-character rule, put
  the digits over the meter.
- The split placements are proved only as "what the layouts say under my reading of them" (the unit test).
  See below.
- `./kit check`: passed.

## Not verified / not 1:1

- **Split placements are not compared to the original.** I never rode a two-player split in the original.
  The readout and meter places in the Top, Bottom and Side views come from the layouts, read as "text in
  the view's own frame, the Flash meter offset from its movie's place on the whole screen". The radar
  layouts' coordinates support the text half of that, and both vertical views put the meter next to the
  text, which suggests the reading is right. The meter's movie place (599,326) is inferred from V28's
  measured 599,314 minus the layout's -12.
- **Three or more views** use the horizontal layouts by their row (top or bottom). That is the port's own
  choice: the original has no free-ride split of more than two.
- **Right-justification differs between the two texts.** The readout's right edge at X is proved by the
  frame above. The odometer keeps V26's last-character rule, which fits the original's ghost and miles
  sitting together but was not re-checked. The engine's general rule is unknown.
- **Glyph rasterisation:** our "51" is 51 px of ink wide against the original's 48, measured from the
  PCSX2 window (593x445) scaled to 640x448, so 1–3 px of either is scaling. The original's digits show no
  dark outline at that scale; ours keep the font atlas's.
- **The meter's fill** is still V28's guess (not touched here). The original frame was taken with it empty.
- **Analog in a split is ignored** (always digital), following the original's code. The original's menus
  were not checked for whether they grey the option out.
- **Applied live** is proved by code path only (`front.menu.changed()` re-reads the setting each frame,
  like the overlay mode). No test rides, changes the setting mid-ride and looks.
- Screenshots of the original are not committed (game data). They were in this session's scratchpad.

## Next

Nothing.
