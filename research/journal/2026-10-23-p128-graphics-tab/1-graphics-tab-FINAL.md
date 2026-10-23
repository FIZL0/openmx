# 1 — graphics tab (FINAL)

Task P128. 2026-10-23.

## What I did

- Settings has six tabs now: Ride, Display, **Graphics**, Audio, Controls, Camera
  (`menu_pages.rs` SETTINGS_TABS, TAB_GRAPHICS = 2; Audio/Controls/Camera moved up one). Graphics sits
  beside Display so L1/R1 go Display → Graphics → Audio.
- The final groups on the Graphics tab (`Menu::graphics_rows`), each a `Row::heading` the cursor steps over:
  - **World**: Draw Distance (V32's Original Fog / Whole Map: the fog and the far clip are one setting).
  - **Performance**: Render Scale, VSync, Frame Limit.
- **Display kept**, for the HUD only: HUD, Speedometer, Minimap, HUD Scale. Not folded into Graphics
  because those four are what is drawn over the ride, not how the world is drawn, and keeping them apart
  leaves Graphics as the one place P126's Original-mode graphics sub-switches go.
- **Ride** already had no HUD / Speedometer / Draw Distance rows on this main (P118 had taken them out;
  ride_rows is Start On, Wrecks, Machines at Bays, Game Modifiers), so nothing to drop there. The new test
  asserts no tab but Graphics has any graphics row, so a doubled row cannot come back silently.
- `--screen graphics` (P123's words): a new `Screen::Graphics`, handled like Controls/Audio/Camera — a
  word that opens Settings on its tab over the main menu, never a screen on the stack.
- The tab strip (`menu_screens.rs` Pen::tabs) shrinks its words to fit the page's strip when they would
  run past it: six tabs at 1.3 em ran "Camera" and R1 under the side panel. Garage's three keep 1.3 em
  (checked by `--shot` at 960x540: Settings' Graphics tab and the Garage).
- Settings fields and settings.txt words are untouched: only where the rows live moved.
- opentennis: no settings page with graphics groups to take; the heading rows are P118's own.

## What the original does

Nothing to port: the menus are ours (AGENT.md). The original has no graphics options.

## How it was proved

`crates/openmx/tests/menu.rs`:
- `the_graphics_tab_has_each_graphics_setting_once_under_its_group` (new): R1 from Display and L1 from Audio
  land on Graphics; its rows are exactly DrawDistance under World, then RenderScale, VSync, FrameLimit under
  Performance, each once; no other tab has any of them; the cursor opens on Draw Distance past the heading
  and the d-pad steps over the Performance heading; Right on Draw Distance sets `Settings::graphics` to
  Original, marks changed, and save/load reads it back; `Screen::from_word("graphics")` + `open` lands on
  the tab.
- Existing tests moved to the Graphics tab (draw distance, render scale, vsync/frame limit rows) and the
  L1/R1 walk updated for six tabs; `every_row_of_every_settings_tab_and_page_changes_by_pad` and P109's
  walked screens pick the new tab up from SETTINGS_TABS.
- Looked at: `openmx --headless --menu --screen graphics --shot` (headings, tab strip fits).

## Not verified / not 1:1

- No plant/tree detail or fade rows under World: V55 and V56 landed without a setting of their own
  (game-render eco.rs has no switch to bind), so there is nothing to put there yet. P126 (Original mode)
  adds those sub-switches; its brief already says they live on this tab.
- The shrunk tab words were looked at at 960x540 only, not at 4K or in split/online menus; the scale is
  in the page's em, so it should hold, but it was not shot.

## Next

Nothing for this task. P126 adds its graphics sub-switches to `graphics_rows` under World.
