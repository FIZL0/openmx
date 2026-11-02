# 1 — splash update notice (FINAL)

Task F14. 2026-11-02.

## What I did

- `menu.rs`: the main menu's newer-build lookup (P193's `builds`, with P191's desktop updater and Updates on Always)
  is now `Menu::find_newer`, which the splash's step runs too while nobody has 1P; Square on the splash with a newer
  build out acts on it (`Menu::update_now`, the main menu's Square: the updater's install or release page, the page's
  reload). The page's console line names the screen ("openmx: the splash offers …"), said again when the build or the
  screen changes, so the main menu's line still comes after the splash's. `update_button` says Refresh for the page
  (was Reload), opentennis's word. `build_label()` is the splash's corner: `OPENMX_TAG` (a release, `v0.9.5`) else
  `OPENMX_COMMIT` (a beta's commit, `dev` locally).
- `menu_screens.rs`: `Pen::new_version` draws the card pinned to the frame's top right, as opentennis's: NEW VERSION,
  the build, a lit pill with Square and its word, "Settings and mods stay"; while an install runs its words in place
  of the pill. `Pen::splash_build` draws the label top left (the main menu keeps its own top-right corner). The
  splash's hints gain Square.
- `web/index.html`: version.txt read 5 s after load and every minute (was every 5 minutes); the site's hero has
  `#corner` with the release tag or the commit (from version.txt, "v0.9.5 abc1234" → v0.9.5, "abc1234" → abc1234);
  the `#fresh` banner reads "New version  A newer openMX is out (…). Your disc, settings and mods stay. [Refresh]".
  The game's splash gets the newer build the way its main menu already did (the page posts `latest` to the worker).
- Tests: `tests/web/update.mjs` waits for the splash's line before pressing Start (and writes `OPENMX_SPLASH_SHOT`);
  `tests/web.rs` asserts it; `built()` drops the `OPENMX_VERSION` cargo hands a test (the build script's
  rustc-env), which had stamped the tests' wasm module a release (OPENMX_TAG `v0.9.5`). `tests/menu.rs`:
  `the_splash_names_the_build_and_its_square_needs_a_newer_one`; `updates_setting_round_trips_and_needs_the_updater`
  now wants Refresh.

## What the original does

Nothing: the update notice and the build label are the port's own front end, after opentennis's.

## How it was proved

Looked right on screen (shots in `context/shots/`, not committed):

- desktop: `OPENMX_LATEST="v0.9.6 abc1234" target/debug/openmx --menu --shot …` on a private headless weston:
  `f14-desktop-splash.jpg` — card with Download (a local build cannot install), "dev" top left, hints Quit · Download
  · Start.
- browser: `cargo test -p openmx --test web a_new_build_shows_on_the_menu_not_the_ride` (headless chromium, software
  renderer) passes with the splash's assert; its `OPENMX_SPLASH_SHOT` shows the card with Refresh and "dev".
- site page: a scratch CDP script served web/ with version.txt "v0.9.5 abc1234", then "v0.9.6 def5678" and
  `openmxRecheck()`: `#corner` reads v0.9.5, `#fresh` is up with "(v0.9.6)" and Refresh.
- `cargo test -p openmx --test menu -- splash updates_setting newer_build corner`: 8 passed.

## Not verified / not 1:1

- A tap or click on the card's pill does nothing: openMX's front-end menus take no pointer (only the pause's music
  player does); opentennis's card takes a tap as △. Square (V on the keyboard) works it.
- No real install was run from the splash: the desktop shot is a local build, whose Square is Download; the install
  path is P191's unchanged code, reached by the same `Updater::act` the main menu calls.
- The iPhone build's card was not shot; it draws the same `new_version` with whatever `update_button` says there.

## Next

-
