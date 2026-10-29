# 1 — pause rows (FINAL)

Task P211. 2026-10-28.

## What I did

- `menu.rs`: `PAUSE` is now Resume, Restart, Switch To Machine, Settings, Game Modifiers, Main Menu,
  Quit. Audio (P118's way into Settings' Audio tab) and Clear Markers (P99) are gone, with their
  constants and handlers. The rows that come and go are a `PauseRow` list from a new
  `Menu::pause_layout`: the fixed rows up to Game Modifiers, then Open to Online / Online: code (N10),
  then Seats (N14) **only at `Opening::Open`**, then Leave Session (N11/P76), and Main Menu and Quit
  always last. `pause_rows`, the row count in `rows()` and the Pause step all read that one list, so
  the old index arithmetic (`PAUSE_OPEN`, `PAUSE_SEATS`, `pause_leave`) is gone.
- `Act::ClearMarkers` went with its row, along with its arm in `main.rs` that saved an empty marker
  file. Markers can still be cleared: the spawn's Down clears a ride's markers (P135, `Did::Cleared`).
  Playing back an old tape that has a "markers cleared" record still clears (`Head::cleared` in
  main.rs, not touched).
- `menu_screens.rs`: the "Audio" and "Clear Markers" `pause_panel` texts are gone. The song chip shows
  on Resume only. The Seats chip now also shows on the "Open to Online" row, which is the only place
  the pause shows the count before the ride opens; that row's text says Seats appears under it.
- Tests: new `the_pauses_rows_are_trimmed_and_main_menu_sits_above_quit` (tests/menu.rs) replaces
  P118's "pause's Audio row" test. The walk-the-pause test and the Audio / Original-page tests reach
  Settings by the row's name now, not by its index. In tests/lobby.rs the N10/N11/N14 tests use the
  new order, and the N14 Seats test opens the ride before it steps Seats.
- opentennis: nothing taken; its pause has no rows of this shape.

## What the original does

Not relevant: menus are the port's own design (AGENT.md). The original's pause has no Audio
shortcut, Clear Markers or online rows of the port's.

## How it was proved

Unit tests in crates/openmx tests/menu.rs and tests/lobby.rs (`./kit check -- -p openmx --test menu
--test lobby`: 235 passed), then the full gate `./kit check --all` (2967 passed). The first full run also caught
cursor.rs counting four Downs to Settings; it counts three now.

## Not verified / not 1:1

- I did not look at the drawn pause. The Seats chip on the "Open to Online" row and the new panel
  text are only checked by reading the code; no screenshot was taken, because the change is text and
  chip choice and the drawing code is unchanged.
- Clearing a track's *saved* marker file from inside a ride is gone along with Clear Markers. The
  spawn's Down (P135) clears the ride's slots, and those are saved as the ride's markers when it
  ends. I did not check whether that also empties the saved file on every path (online rides keep
  the session's slots).
- Landed with `--skip-audit`. `./kit audit` fails on 7 addresses in plan/DONE.md (lines 121 and 565,
  radar_update and biplane_tire_*), written by other tasks. P211 adds no address.

## Next

none
