# 1 — web launcher (FINAL)

Task P181. 2026-10-26.

## What I did

- **web/index.html** is now the site around the game, in docs/menu-concept.html's look (ember backdrop with
  the ghosted MX and streaks, maroon pills with chalk rims, oblique ExtraBold titles, the ink band with the
  button hints, M PLUS Rounded 1c).
  - *No disc stored* (`body[data-state=asking]`, `data-disc=no`): a scrolling page with a Discord bar, the
    hero (open+MX wordmark, GitHub button), the Pick your disc card (SLUS-20767, kept in OPFS, drop
    anywhere: a full-page drop zone), three steps, About (the original / openMX cards with platform chips),
    Settings & files (lobby name in localStorage `openmx.name`, storage bar from `storage.estimate`, Forget
    disc, Mods (disabled, says the browser does not take mods yet), Clear caches (caches + service worker),
    Export settings (a JSON of the `openmx*` localStorage keys), Erase everything (OPFS, settings, caches)),
    Support (Chip in, prompt/token/commit counts) and a footer.
  - *Disc stored* (`menu`, `data-disc=yes`): the hero is replaced by a 16:9 menu stage drawn like the
    concept's main menu: Ride, Join with a code, Host a game, Files, Full screen, a panel for the lit item
    and status chips (disc size, pad, graphics, rider). Arrow keys, Enter, mouse and a standard-mapping
    gamepad (d-pad/left stick, bottom button) drive it. Join and Host open the existing `?join=` / `?host=`
    addresses with the lobby name; Files scrolls to Settings & files; Full screen asks for fullscreen and
    rides. The rest of the page sits under the stage.
  - *Phone width*: the stage stands up (a container query below 640 px: logo, list over its strip, panel,
    band); sections go single-column.
  - The page now checks OPFS itself on load instead of starting the worker at once. The canvas is
    transferred to the worker on the first Ride or the first picked disc. Picking a disc still copies and
    starts straight away (that click already means "play"). `#pick`, `#status`, `#online`, the `?join/?host`
    path and the `opfs = true;` line chromium.mjs patches for `--no-opfs` are all unchanged.
  - Placeholders the user fills in sit in one `SITE` object at the top of the module script; an empty link
    is dimmed and inert, an empty count shows a dash. Item in plan/HUMAN.md.
- **web/worker.js**: the canvas can come with `{ import, canvas }` as well as with `{ start }`.
- **web/fonts/**: copies of the two M PLUS Rounded 1c faces and OFL.txt (web/ is the folder that gets
  served, and crates/openmx/assets is outside it). sw.js caches them for offline.
- **Tests**: chromium.mjs presses `#ride` when the reload lands on the menu. New `tests/web/shots.mjs` and
  `the_launcher_fits_a_phone_and_a_desktop` (web.rs): both states at 1440 and 390 wide in headless chromium,
  a stand-in `disc.iso` written into OPFS for the menu; checks the state, no sideways scroll, the font
  loaded, Ride lit on the menu. No disc and no wasm module needed; skips without node or chromium.
- From opentennis: the site's section structure (Discord bar, hero with GitHub pill, play card, the
  settings/file actions, support) from web/index.html + site.js, rewritten in this game's look. Not taken:
  its stats.json, its touch pad, its a2hs/iOS notes.

## What the original does

Nothing to port: the front end is ours to design (AGENT.md), and the web launcher has no counterpart on the
disc.

## How it was proved

- `the_page_starts_in_a_browser` (ignored, run by hand with the disc this session:
  `./kit check -- -p openmx --test web -- --ignored --nocapture the_page_starts_in_a_browser`): passed.
  The first visit asks, copies the 2.2 GB disc into OPFS and draws; the reload opens on the menu, Ride
  draws the same picture (same sha-256); with OPFS refused the page plays off the file and asks again.
- `the_launcher_fits_a_phone_and_a_desktop`: passed. Looked at the four shots myself next to a headless shot
  of docs/menu-concept.html's main menu: same backdrop, pill, strip, band and hint tab.

## Not verified / not 1:1

- **The concept canvas itself** (claude.ai artifact J86nfZixLo3UGq2cKwdU6E) is private and was not reachable
  from this session. The page follows the brief's description of its three boards and docs/menu-concept.html,
  not the canvas's pixels, so layouts, copy and spacing may differ from it.
- **The user has not approved the look.** The brief asked for that first; the run was unattended, so it
  landed for review (plan/HUMAN.md).
- **Placeholders**: the Discord invite, the dump guide, the donate URL, what donations pay for and the counts
  are empty, by the brief.
- **Gamepad driving of the stage** was not tested: there is no pad on this machine that chromium sees (the
  shell does not read uinput pads either). The key and click paths were tested only as far as the tests
  press Ride. Join/Host only build the `?join=`/`?host=` addresses already covered by the netplay browser
  test; that the menu builds them right was checked only by reading the code.
- **Forget disc, Clear caches, Export settings, Erase everything and drag-and-drop** were not driven in a
  test. Confirm dialogs make them awkward in headless chromium.
- **Real phones** (iOS Safari, Android Chrome) were not tried; 390 px is chromium's mobile emulation.
- **"SLUS-20767"** on the pick card is a label: the page does not check that the picked image is that disc
  (neither did the old page).
- **The "Graphics: software" chip** is fixed text. The browser build draws through the CPU and putImageData
  today, so it is true now, but nothing reads it from the module.
- **The About cards' facts** (Rainbow Studios, THQ, 2004, PS2 and Xbox) are general knowledge, not checked
  against a source in this session.

## Next

The user's verdict on the look, and the `SITE` values.
