# 1 — graphics tab (FINAL)

Task P128. 2026-10-23. Third attempt. The first (106943b) was cut before P126 landed; the second (wip/P128,
243feba) carried it onto main past P126, but its merge was left for a human: the fix session joined the
conflicts in menu_pages.rs (adding P129's Markers to `Departure::tab`), then the tests after the merge failed
in game-render's `ecosystem` and openmx's `one_fps` while three other check runs were going, and the repair
session ran out of time. Neither test reads the menus. This attempt applies wip/P128's code diff onto
today's main (49944589) and fixes what main gained since: MD25's 360 Style switch (`Departure::ThreeSixty`)
goes on the Original tab, and MD9's World Sky row, which the merge put under World, is in the test's World
group. The whole gate then ran on this branch (see `./kit done`).

## What I did

- Settings has seven tabs: Ride, Display, **Graphics**, Audio, Controls, Camera, Original
  (`menu_pages.rs` SETTINGS_TABS; TAB_GRAPHICS = 2, Audio/Controls/Camera/Original moved up one).
  Graphics sits beside Display, so L1/R1 go Display → Graphics → Audio.
- The final groups on the Graphics tab (`Menu::graphics_rows`), each a `Row::heading` the cursor steps over:
  - **World**: Draw Distance (V32's fog and far clip, one setting), Plant Detail (V55's Smooth / Original),
    World Sky (MD9's Global / Per Map sky and light for combined worlds; it was on Display).
  - **Picture**: Picture Shape (V24's Fill the Window / 4:3).
  - **Performance**: Render Scale, VSync, Frame Limit.
- **P126's graphics sub-switches moved here** from the Original tab, as the brief says: the fog switch *is*
  Draw Distance (same `Settings::graphics`, same two words in the same order), so the Original tab's
  "Fog and Draw Distance" row is gone rather than doubled; Plant Detail and Picture Shape are the same
  `Key::Departure` rows, built by the new shared `Menu::departure_row`. The Original tab keeps the master
  switch and the Controls, Cameras, Markers (P129) and 360 Style (MD25) sub-switches, and its help now says the fog, plants and picture shape
  are on Graphics. The master switch still sets all seven (`Departure::ALL` is untouched).
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
- The new test steps the d-pad three rows (Draw Distance, Plant Detail, World Sky) down to Picture Shape,
  over the Picture heading, and back.
- Existing draw distance / render scale / vsync tests moved to the Graphics tab; the L1/R1 walk is seven
  tabs round from either end.

## Not verified / not 1:1

- The menus are ours (AGENT.md), so there is nothing to match against the original; what follows is only
  what was not checked.
- The second attempt's merge failures (`ecosystem`, `one_fps`) were not reproduced on its merge; they are
  judged unrelated because neither test touches the menus, and this branch's own full gate is the check.
- This branch's first `./kit done` gate failed one assertion in `one_fps`: sky 8.5 ms against its 8.0 ms
  budget, with a machine load average of 42 from the other slots. Run alone right after, it passed. The
  headless ride it times draws no menu, so it was judged a load red and `done` was run again. One sample;
  I did not take a timing on main to compare against.

- No tree (V56) row under World: V56's foliage fix landed without a switch of its own (no `Departure`
  or settings field to bind), so there is nothing to put there; the plants row is V55's.
- The shrunk tab words were looked at at one window size only (the --shot default), not at 4K or in a
  split-screen pause; the scale is in the page's em, so it should hold, but it was not shot.

## Next

Nothing for this task.
