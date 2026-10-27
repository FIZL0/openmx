# 1 — camera save and original page

Task P179. 2026-10-26.

## What I did

- **Save as New Preset from anywhere on the Camera tab.** `Menu::camera_to_save` (`menu_cameras.rs`)
  is the preset the save copies: the Preset row's when one is picked; on All Presets, the one the seat's
  rides start on, on the machine the seat starts on (`settings.machines[seat]`) — the name the camera
  button last left it on (`camera_sets[seat].start`), or, before it has moved, the disc's first preset
  in the cycle. The look copied is `camera_look` of that preset: its own numbers if it has some, else All
  Presets' one set as it frames that machine. The row is dimmed only when there is nothing to copy at
  all (no disc presets read and none of the player's). `save_camera` now goes through it.
- **The Original tab is a page of the Ride tab's.** `SETTINGS_TABS` is six tabs. A new
  `Screen::Original` (`--screen original`, title "Original Game") is an option page with the same rows
  as the old tab (master switch, Enhancements heading, Controls / Cameras / Markers). The Ride tab ends
  in an "Original Game" row (`Key::OriginalPage`, `Kind::Opens`) whose value is the master switch's
  Off / On / Mixed; Cross pushes the page, Circle pops back onto the Ride tab with the cursor on that
  row (the Settings cursor is untouched underneath). Works from the pause's Settings too.
  `Departure::tab()` for the non-graphics departures is now `TAB_RIDE`, meaning "through the Ride tab".
- Tests (`tests/menu.rs`): new `a_camera_preset_saves_from_all_presets_and_survives_a_restart` (save on
  All Presets by pad and keyboard; the typed name is in 1P's set over the start preset on All Presets'
  numbers; the camera button's walk reaches it; a start left on Freestyle Cam copies that preset and its
  own numbers; a biplane start copies the biplane's; written with `menu::save`, read back with
  `menu::load` into a new `Menu`, the Preset row lists all three and the bike's camera walk has both
  bike ones; with no presets the row is dimmed) and `the_original_game_page_is_a_page_of_the_ride_tab`.
  The helper `tab_to(.., "Original")` now goes Ride → row → Cross, so the P126/V57/V83/V84 tests drive
  the page the way a player does; the tab-walk test is six tabs; the every-row and every-frame-rate walks
  cover the Original page as a screen of its own (3990 runs still).
- Looked at `--screen settings` and `--screen original` shots at 1280x720 (software renderer): the Ride
  tab's last row reads "Original Game  Off ▸", the page has no tab strip and Back / Change hints.
- opentennis: nothing taken; its settings have no sub-page of a tab. The pattern is this repo's own Game
  Modifiers page (`open_modifiers`), copied for `open_original`.

## What the original does

Front-end only; the original has no such settings. Nothing ported.

## How it was proved

Unit tests through the pad reader in `crates/openmx/tests/menu.rs` (173 pass), shots by eye.

## Not verified / not 1:1

- "The preset in use" on All Presets is read from the menu's side only: the seat's Start On machine and
  the preset the camera button last left that machine's camera on. Under the pause on a ride that has
  since switched machines (Start On bike, riding the biplane), the save copies the Start On machine's
  preset, not the one on screen: the menu is not told which machine the seat is on now.
- An empty start word is taken to be the disc's first cycling preset. The test proves it for both files
  as `camera_file` reads them (`Race::new` starts on preset 0, the first in the cycle) — on a slot
  without the disc that is the stand-in's order, not the disc's. Under Mine Only the shell starts on the first of
  the player's own instead; the save then still copies the disc's first.
- "Survives a restart" is proved through `menu::save` / `menu::load` and a new `Menu`, plus the
  `laid` → `chase::seat_presets` path the shell uses; no real shell was restarted and ridden.

## Landing

`./kit done` ran with `--skip-audit`: the audit's seven leaks are all in `plan/DONE.md` (lines 121, 427: radar and biplane_tire_* names), already on main and not this task's; this diff adds no address. The gate passed (2440). Earlier runs failed `original_mode` (no DISPLAY exported; fixed with DISPLAY=:0) and `one_fps` / `fixed_rate` at load average 40 from another slot's gate; both passed once the load fell.

## Next

Nothing.
