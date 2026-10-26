# 1 — screen words for tabs (FINAL)

Task P123. 2026-10-26.

## What I did

- `Screen::tab_from_word` (menu_pages.rs): a tab of Settings or the Garage by its name in lower case
  (`ride`, `display`, `original`, `rider`, `bike`, `gear`; the others too, but `Screen::from_word` wins).
- `Menu::open_tab(screen, tab)`: over the main menu, as the pad reaches it, cursor on the first row.
  For the Garage it opens the garage (every seat's pick) and then sets the tab.
- `--screen` in main.rs tries the screen words first, then the tab words; `Args::screen_tab` carries the
  tab. The tabs that were screens (graphics, audio, controls, camera) keep their own words and paths:
  `audio` still opens over the pause. The error and `--help` list the tab words.
- opentennis: nothing taken; its menus have no tabs of this kind.

## What the original does

Nothing to port: `--screen` is the shell's own front-end flag, and the menus are ours to design.

## How it was proved

- tests/menu.rs `every_settings_and_garage_tab_has_a_screen_word`: every name in `SETTINGS_TABS` and
  `GARAGE_TABS` opens its screen on that tab with the cursor on a selectable row, and Back goes to the
  main menu (not for Audio, which is over the pause).
- Live: a release build with `--menu --screen W --shot` for each of ride, display, original, rider, bike
  and gear on the GPU renderer, DISPLAY=:0. Every png showed its tab highlighted in the tab strip and that
  tab's rows. `--screen nope` prints the new error.

## Not verified / not 1:1

- No shell (binary) test runs `--screen <tab>`. The front end with `--shot` does not exit (see the menu
  shots memory), and the parse is a two-line match over the functions the unit test covers. Checked once
  by hand instead, as above.
- Nothing against the original: this is a front-end flag, and the brief asks for nothing more.

## Next

Nothing.
