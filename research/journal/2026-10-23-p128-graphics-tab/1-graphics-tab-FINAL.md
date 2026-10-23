# 1 — graphics tab (FINAL)

Task P128. 2026-10-23. Second attempt: the first (wip/P128, 106943b) was cut before P126 landed and its
merge was left for a human only because the fix session could not reach the API. This one is that work
carried onto main with P126's Original tab in it.

## What I did

- Settings has seven tabs: Ride, Display, **Graphics**, Audio, Controls, Camera, Original
  (`menu_pages.rs` SETTINGS_TABS; TAB_GRAPHICS = 2, Audio/Controls/Camera/Original moved up one).
  Graphics sits beside Display, so L1/R1 go Display → Graphics → Audio.
- The final groups on the Graphics tab (`Menu::graphics_rows`), each a `Row::heading` the cursor steps over:
  - **World**: Draw Distance (V32's fog and far clip, one setting), Plant Detail (V55's Smooth / Original).
  - **Picture**: Picture Shape (V24's Fill the Window / 4:3).
  - **Performance**: Render Scale, VSync, Frame Limit.
- **P126's graphics sub-switches moved here** from the Original tab, as the brief says: the fog switch *is*
  Draw Distance (same `Settings::graphics`, same two words in the same order), so the Original tab's
  "Fog and Draw Distance" row is gone rather than doubled; Plant Detail and Picture Shape are the same
  `Key::Departure` rows, built by the new shared `Menu::departure_row`. The Original tab keeps the master
  switch and the Controls and Cameras sub-switches, and its help now says the fog, plants and picture shape
  are on Graphics. The master switch still sets all five (`Departure::ALL` is untouched).
  `Departure::tab()` and `Departure::key()` say where each switch's row is.
- **Display kept**, for the HUD only: HUD, Speedometer, Minimap, HUD Scale. Not folded into Graphics:
  those four are what is drawn over the ride, not how the world is drawn.
- **Ride** already had no HUD / Speedometer / Draw Distance rows on this main (P118 took them out), so
  nothing to drop. The new test asserts no tab but Graphics has a graphics row, so a doubled row cannot
  come back silently.
- `--screen graphics` (P123's words): `Screen::Graphics`, handled like Controls/Audio/Camera — opens
  Settings on its tab, never a screen on the stack.
- The tab strip (`menu_screens.rs` Pen::tabs) shrinks its words to fit when they would run past the
  page's strip. Seven tabs looked at by `openmx --menu --screen graphics --shot`: all fit between L1 and R1.
- Settings fields and settings.txt words are untouched: only where the rows live moved.
- opentennis: no settings page with graphics groups to take; the heading rows are P118's own.

## What the original does

Nothing to port: the menus are ours (AGENT.md). The original has no graphics options.

## How it was proved

`crates/openmx/tests/menu.rs` (`./kit check -- -p openmx --test menu`: passed, then the full gate):
- `the_graphics_tab_has_each_graphics_setting_once_under_its_group` (new): R1 from Display and L1 from
  Audio land on Graphics; its rows are exactly the three groups' keys, each once under its heading; the
  departures with `tab() == Graphics` are exactly the graphics ones on it; no other tab has any of them;
  the cursor opens on Draw Distance past the heading and the d-pad steps over headings; Right on Draw
  Distance sets `Settings::graphics`, marks changed, and save/load reads it back; `--screen graphics`
  opens the tab.
- P126's `original_on_sets_every_sub_switch_and_one_turned_back_leaves_the_rest` now turns each sub-switch
  back on the tab its row is on (Graphics for fog/plants/picture) and returns to Original for the master.
- Existing draw distance / render scale / vsync tests moved to the Graphics tab; the L1/R1 walk is seven
  tabs round from either end.

## Not verified / not 1:1

- No tree (V56) row under World: V56's foliage fix landed without a switch of its own (no `Departure`
  or settings field to bind), so there is nothing to put there; the plants row is V55's.
- The shrunk tab words were looked at at one window size only (the --shot default), not at 4K or in a
  split-screen pause; the scale is in the page's em, so it should hold, but it was not shot.

## Next

Nothing for this task.
