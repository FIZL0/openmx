# 1 — player desktop build (FINAL)

Task P190. 2026-10-29.

## What I did

- `crates/openmx/src/player.rs` (new, desktop only), taken from opentennis's `hst-base/src/disc.rs`
  (`data_dir`, `pick`) without its pack import: openMX reads the ISO itself, so there is nothing to import.
  The disc is chosen once, at start, after the command line parsed (so `--help` never asks):
  1. the first argument when it is not a flag (a file dropped on the binary arrives as that argument),
     taken off `argv()` so the flags parse as before; it must open as a disc (`Disc::open`), else an error;
  2. else, in a dev checkout (a `kit.toml` in the working directory or above), kit.toml's `[game] iso`, as today;
  3. else the path remembered in the data folder's `disc.txt`, if it still opens;
  4. else a dialog asks: zenity, then kdialog (Linux), PowerShell's OpenFileDialog (Windows), AppleScript's
     `choose file` (Mac); with none to show and a terminal on stdin, the terminal asks; else an error saying how
     to start it. A disc from 1 or 4 is written to `disc.txt` when there is no kit.toml.
- With no kit.toml, the data folder is `$XDG_DATA_HOME/openmx` (else `~/.local/share/openmx`),
  `%APPDATA%\openmx`, `~/Library/Application Support/openmx` (opentennis's choice of folders).
- `game_render::disc`: `set_player_disc` (a OnceLock, as the page's `PAGE_DISC`) that `open_configured`,
  `configured_path` and `configured_image` use ahead of kit.toml, so every opener in the shell (lobby, garage,
  front end, ride) follows the player's disc; `kit_toml()` finds the nearest kit.toml.
- `menu::store_dir` returns the data folder once `menu::set_data_dir` set it; everything kept "beside the disc"
  goes through it (settings.txt, controls.txt, mods/, markers, picks, looks, music folder), so all of it moves.
- `openmx --where`: prints the disc and where it came from, the data folder, settings.txt's path and the rider
  name read from it, and stops — for players, and what the test reads.
- tools/build.sh's Linux folder is `openmx` + `README.txt` (no run.sh, no kit.toml); `extras()` still
  lifts out with pages.yml's sed (checked). web/index.html's Linux card, tools/release.sh's header and download
  table, and pages.yml's beta notes say how to start it.

## What the original does

Nothing to port: the front end's start-up is ours.

## How it was proved

`crates/openmx/tests/player_start.rs` runs the release shell itself (`--where`) from a fresh folder with no
kit.toml above it (temp dir, /tmp or /dev/shm, whichever has none), `XDG_DATA_HOME` pointed into it, on this
slot's real disc (skips without one):
- the disc from argv, `disc.txt` written; the next start with no argument takes the remembered one; a file
  that is not a disc is refused and not remembered; nothing remembered, no dialog (PATH emptied) and no
  terminal → the "no disc: start openMX with …" error;
- a settings.txt in the data folder is the one read (rider name round-trips), and without `XDG_DATA_HOME` the
  folder is `~/.local/share/openmx`;
- from the crate (under the checkout's kit.toml) the disc is kit.toml's and settings sit beside it, as before.
Also by hand: the same three cases in a scratch folder, and `--where` in the checkout.

## Not verified / not 1:1

- The dialogs were never shown: this machine has neither zenity nor kdialog, and there is no Windows or Mac
  build to run PowerShell's or AppleScript's picker on. The commands are opentennis's for Linux (which it ships)
  and the standard ones elsewhere, untested here.
- "The menu's own browser" is not built: the task offered a picker *or* it, and the picker plus a terminal
  question was the smaller change. A desktop with no dialog program started from a launcher (no terminal) gets
  only the error on stderr, which nobody sees. Filed as P217.
- "A file dropped on the binary": only that it arrives as the first argument (which is how Windows Explorer
  and the Linux file managers that allow it pass it) is relied on; no drop onto the binary was tried.
- The browser build (wasm32) was not compiled: this machine's toolchain has no wasm32 standard library. The
  new code is `cfg(not(wasm32))` or target-neutral (`PLAYER_DISC`, `DATA_DIR`, `where_only`).
- tools/build.sh's docker build was not run (no change to the build step itself); `extras()` was run alone.
- Settings already kept beside a disc by an earlier build (the old kit.toml tarball) are not moved into the
  data folder: a player of the old tarball who drops kit.toml starts with default settings.

## Next

-
