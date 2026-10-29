# 1 — web update notice (FINAL)

Task P193. 2026-10-29.

## What I did

- **The page** (`web/index.html`): the version.txt check now keeps the newer build (`newer`). With the game on the
  canvas (`body[data-state=started]`) it goes to the worker as `{ latest }` and the page's own Reload banner stays
  down; the banner is the site page's alone. It is checked again when the game starts (`fresh()`), so a build found
  on the site page moves into the game. `window.openmxRecheck()` reads version.txt at once (opentennis's `hstCheck`).
  Before anything starts the page sends the worker `{ lastBuild }`, local storage's `openmx.build`; the worker's
  `{ build }` writes it back, its `{ reload }` reloads the page.
- **The worker** (`web/worker.js`): three new host calls, `latest()`, `last_build(ours)` (returns the stored build and
  sends back ours) and `reload()`. `tests/web/host.mjs` answers them too (nothing newer, a first visit, a no-op).
- **The module** (`web.rs`): the externs, re-exported as `host_latest`, `host_last_build`, `host_reload`.
- **The menu** (`menu.rs`, `menu_screens.rs`): `newer_build()` (the page's latest, or `OPENMX_LATEST` anywhere, never
  this build's own commit: `newer_than`, opentennis's stale-version.txt rule) and `updated_build()` ("Updated to
  v<version> · <commit>" when local storage's last build is another one, `first_start`; `OPENMX_UPDATED=1` fakes it).
  `Menu::builds` asks both only while the main menu is up (a ride never has `Screen::Main` on top, so nothing is ever
  drawn over a ride); the "Updated to" line is asked once a run and stays 5 s (`UPDATED_TICKS`). They are drawn under
  the P189 build corner, the newer one in peach; the hints become Square Reload · Cross Select, and Square on the main
  menu reloads the page (`reload_page`, nothing on the desktop). What the main menu shows it also logs to the page's
  console (`page_says`), which the chromium test reads.

Taken from opentennis: `crates/hst/src/update.rs`'s `latest()` (the not-our-own-commit filter) and `updated()` (B625,
the last started build in local storage, a first visit says nothing), and `web/index.html`'s `hstLatest` handoff. Changed:
the game here runs in a worker, which has no `window` and no local storage, so the page passes both through messages
instead of the module reading `window.hstLatest`/`localStorage`; the notice is a line under the corner plus a Square
hint rather than opentennis's New version card; no `?next`/service-worker prefetch (B470/B625's launch-time reload),
which this site's sw.js does not do.

## What the original does

Nothing: a front-end addition (the menus are ours to design).

## How it was proved

- `crates/openmx/tests/web.rs` `a_new_build_shows_on_the_menu_not_the_ride` (`tests/web/update.mjs`, headless
  chromium, software renderer, the disc read in place): with `?args=--ride` on the canvas the site's version.txt
  changes and the page rechecks — the banner stays down and the game logs nothing about builds; the next visit, with
  `old0000` stored as the last build, Start past the splash: the main menu offers `v0.0.2 2222222`, says
  "Updated to v… · dev", local storage then holds `dev`, and V (Square on the default modern keyboard) reloads the
  page, whose footer then names the newer build. Run by hand too: the JSON came back as asserted.
- The old page would have failed it: its banner went up whenever version.txt changed, ride or not.
- `crates/openmx/tests/menu.rs` `a_newer_build_is_another_commit`: `newer_than`/`first_start` cases.
- Looked at a screenshot of the main menu with both lines and the Square Reload hint.

## Not verified / not 1:1

- The ride half of the test watches for 4 s after the recheck; it proves the ride shows nothing because the menu only
  asks on `Screen::Main`, not by reading the ride's pixels.
- A pause → Main Menu round trip (the ride's build offered once back on the main menu) was not driven in chromium:
  the test uses a second visit for the menu half instead. The same `builds()` runs whichever way Main is reached.
- Square on a pad and on the phone's touch pad was not pressed; only the keyboard's V. The touch pad's buttons were
  not checked for a Square.
- The real site path (pages.yml's version.txt, a CDN, the service worker's cache) was not exercised: the test's
  server answers version.txt itself and serves no sw.js.
- No pointer tap on the notice: the main menu has no tap targets to hang one on.

## Next

Nothing.
