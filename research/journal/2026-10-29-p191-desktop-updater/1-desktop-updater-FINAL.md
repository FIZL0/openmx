# 1 — desktop updater (FINAL)

Task P191. 2026-10-29.

## What I did

- `crates/openmx/src/update.rs` (new), taken from opentennis's `crates/hst/src/update.rs` (B317/B374/M140):
  the beta and release checks, semver `newer`, `found` from GitHub's releases/latest JSON, the signed
  `SHA256SUMS` check (`minisign-verify`, `signed_sum`), the streamed download with its sha256, `swap`
  (Linux rename over the running file; Windows' `.old` dance, kept for when a Windows build exists) and
  the restart with the same arguments, then `exit(0)`. Kept the same tests (local HTTP server, a
  throwaway minisign key).
  - **What I changed from opentennis's:** M140's `Way::Copy` (installing into the user data folder and
    the start-up `relaunch` handover) and Android's `.apk` path are left out. A folder it can't write
    (or a Flatpak, or a Steam library) gets the release page, as the task says. bevy's logging is
    replaced by `println!` (the shell's own way); `reqwest`'s `chunk()` stands in for n0-future's byte
    stream (openmx's reqwest has no `stream` feature); and each job builds its own current-thread
    tokio runtime, as `modstore.rs` does. No "Later" (○) and no Update? popup: in openmx the notice
    itself is the asking. It sits under the corner, where P193 put the browser's, with Square on the
    hint bar.
  - Debug builds never ask (`VERSION`/`BETA_COMMIT` are `None` under `debug_assertions`). This means a
    CI or test build that happens to have `GITHUB_SHA` in its environment never reaches GitHub, and
    never puts a notice into a menu screenshot.
- `build.rs` emits `OPENMX_TAG=v<version>` only when `OPENMX_VERSION` is set (release.sh). It has to:
  `OPENMX_VERSION` is always emitted (the crate's version for the corner), so it can't tell a
  release build from any other.
- The menu reaches the updater through `menu::Updater`, a struct of fn pointers in a `OnceLock` that
  `main.rs` fills at start. This keeps `crate::update` out of `menu.rs`: 18 test binaries `#[path]`-mount
  menu.rs without the shell's other modules. With no updater (the browser, the tests), the menu is
  P193's: Square is Reload and the Updates row is hidden.
- Settings → Ride tab → **Updates** (Ask/Always/Never), `updates = …` in settings.txt (written only
  when not Ask). Always: the main menu starts the install by itself, once per newer build, when it can
  install in place. Never: no check at all.
- `--no-update-check` (in `--help`); `OPENMX_UPDATE_API=<url>` for a test server; `OPENMX_LATEST`
  (P193's) still fakes a notice.
- `PUBLIC_KEY` = `~/.config/openmx/release.pub`'s key (`RWR5k+YX2E…`). `tools/release.sh` now always
  checks the release's signature against it (the `if [ -f update.rs ]` guard is gone). Comments are
  updated in pages.yml (GITHUB_SHA makes the build a beta) and plan/HUMAN.md item 3 (done).

## What the original does

Nothing: the PS2 game has no updater. This is front end, ours to design.

## How it was proved

- Unit tests in `update.rs` (8, `./kit check -- -p openmx --bin openmx -- update::`):
  - semver order over 11 versions, pairwise;
  - a SHA256SUMS signed with a throwaway key verifies, and a flipped byte, a bad signature file, or a
    name missing from the sums is refused;
  - **the built-in PUBLIC_KEY accepts a SHA256SUMS signed with the real `~/.config/openmx/release.key`**
    (signed for the test, `RELEASE_SIG`), and refuses the throwaway key's;
  - a local server's release is found and installed (check → verify → swap; the file is executable;
    the same version, or anything older than the version running, is not offered);
  - a tampered release (wrong key, sums not matching their signature, a signed wrong checksum) leaves
    the old game and no `openmx.new`;
  - no server, an HTML answer, a 404, or JSON with no release say nothing (an Err the menu never
    shows, or None);
  - the half-hour wall-clock rule; Steam's library; `found` needs a `v` tag, no draft, and all three
    files.
- `tests/menu.rs`: `updates_setting_round_trips_and_needs_the_updater`, plus `updates` in the
  all-fields round-trip.
- **End to end, by hand, with the real binary.** A release build stamped `OPENMX_VERSION=v0.0.1`,
  copied alone into a scratch folder, `XDG_DATA_HOME` pointing at a scratch data folder whose
  settings.txt says `updates = always`, `OPENMX_UPDATE_API` set to a python `http.server` serving
  `latest` (v0.0.2), a stand-in `openmx-linux-x86_64` (a shell script that writes its arguments to a
  file), and a SHA256SUMS signed with the real release key. Run: `openmx <iso> --menu --screen main`.
  - The game logged `update check: v0.0.2 is out`. The server saw GET latest, SHA256SUMS, .sig and the
    binary. The game exited 0, the game file was now the script (mode 755), and the script ran with
    the same arguments (`<iso> --menu --screen main`).
  - With the served binary changed after signing, the log said `update: openmx-linux-x86_64's
    checksum doesn't match the signed one`. The game kept running and its file was byte-identical.
  - With `--no-update-check`, the server got 0 requests.
  - On Ask, a headless `--shot` of the main menu showed "New version out: v0.0.2" under the corner and
    "□ Update  ✕ Select" on the hint bar. The Settings Ride tab showed "Updates  Ask" as its last row.
- The browser's build (`cargo check -p openmx --lib --target wasm32-unknown-unknown` with tools/web.sh's
  scratch 1.99.1 toolchain) compiles.

## Not verified / not 1:1

- **Against GitHub itself:** no `v*` release of openmx exists yet (P195 is the first), so the live
  `releases/latest` and `releases/tags/beta` answers were never fetched by the game. The JSON fields
  it reads (`tag_name`, `draft`, `html_url`, `assets[].name`/`browser_download_url`,
  `target_commitish`) are the ones opentennis's updater reads from the same API, which works there.
- **The beta path** (an Actions build asking which commit `beta` is at) is not tested end to end: it
  needs a release-profile build with `GITHUB_SHA` set and the live API. Its Square opens the beta's
  release page (no signed sums on the beta, so nothing to install).
- **The Square press itself** was not driven (no keystrokes reach windows on this desktop). The e2e
  went through Always, which calls the same `act()` that Square's arm calls.
- **The unwritable-folder / Flatpak / Steam → release page branch** was not run, and neither was
  `xdg-open` opening it. `target()` is three plain checks; `steams` has a unit test.
- **Windows:** there is no Windows build. The `.old` rename path is unit-tested on Linux only
  (`swap(…, windows = true)`), and `cleanup()`'s removal of the `.old` file has never run on Windows.
- **The half-hourly re-check** was not observed live (only `due` is tested). The thread sleeps in 5 s
  steps until the wall clock says it is due.
- **Not taken from opentennis's:** M140's install into the data folder for unwritable installs, the
  startup `relaunch` handover, the Update? popup, Later, the lobby-refusal re-offer, and the
  app-resume recheck. The task asks for none of them.

## Next

Nothing for this task. After the first real release (P195), check that a v0.1.0 build offers v0.1.1
from the live API.
