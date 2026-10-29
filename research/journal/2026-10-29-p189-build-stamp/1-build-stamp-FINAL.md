# 1 — build stamp (FINAL)

Task P189. 2026-10-29.

## What I did

- `crates/openmx/build.rs` (new), taken from opentennis's `crates/hst/build.rs` (B302/P59/M22) without its Windows
  icon half. OPENMX_COMMIT from the environment wins in any profile (tools/build.sh, tools/release.sh and pages.yml
  already set it, so none of the three needed a change); else a release build stamps git HEAD's first 7 characters,
  `+` when `git status --porcelain --untracked-files=no` is non-empty, `unknown` without git; dev/test builds say
  `dev`. Changed from opentennis: the logic is three plain functions (`commit`, `id`, `version`) so a test can mount
  the file; it also emits OPENMX_BUILD_ID (`<crate version> <commit>, 30 Hz`) and OPENMX_VERSION (the tag's, `v`
  stripped, else the crate's version) for the corner. It watches `index` beside HEAD and logs/HEAD.
- The id keeps the crate's version, not OPENMX_VERSION: tools/release.sh already rewrites Cargo.toml to the tag, and
  an unstamped dev id is then exactly handshake.rs's default (`<version> dev, 30 Hz`).
- `main.rs`: `const BUILD_ID` and `openmx_net::handshake::set_build(BUILD_ID)` first thing in `run()` — the
  desktop's, the iPhone's (`ios.rs` → `crate::run`) and the browser's (`openmx_run` → `shell::run`) front ends all
  come through it, before any lobby.
- `web.rs`: `pub const BUILD_ID` at the lib root; `openmx_run` and the page's `Online::new` lobby stamp it too.
- `menu.rs`: `BUILD_CORNER` = `v<version> · <commit>`; `menu_screens.rs` draws it small (DUST, shadowed) in the
  main menu's top-right corner after the band. Looked at in a `--menu --screen main --headless --shot` frame:
  `v1.0.0 · dev` sits clear of the logo and the panel.
- Tests: `tests/build_stamp.rs` (a stamped build's id names its commit, env wins in both profiles, release from git
  with `+`; an unstamped dev build keeps `<version> dev, 30 Hz` and equals handshake's default),
  `tests/menu.rs` `the_main_menus_corner_names_the_build`.

## What the original does

Nothing: the build stamp is the port's own (front end and online play), no disc behaviour.

## How it was proved

Unit tests over the stamp functions and the built test crate's own constants; the corner by eye in a shot.

## Not verified / not 1:1

- A real stamped build (OPENMX_COMMIT=abc1234 cargo build, or a release build from git) was not built and run: it
  recompiles the shell in this slot's target twice for the gate; the functions it runs are the ones the test covers.
- The wasm build (`web.rs`'s `openmx_run` / `Online::new` stamps) was not compiled: no wasm32 target installed in
  this toolchain here. The calls are the same `openmx_net::handshake::set_build(crate::BUILD_ID)` as the desktop's.
- A release build's `+` follows a dirty tree only when HEAD, logs/HEAD or the index change (as opentennis's): editing
  a tracked file without staging it does not rerun build.rs, so an earlier clean stamp can stay. The shipped builds
  all pass OPENMX_COMMIT, so they are unaffected.
- Two builds refusing each other by commit was not run over a real lobby: the refusal is the existing
  `same_build` check on the handshake's id, which the id now carries the commit in.

## Next

-
