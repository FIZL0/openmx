# 2 — keep B49's fix (FINAL)

Task K86. 2026-11-21.

## What I did

- The person picked A: keep B49's fix and close, unless `tools/web.sh --check` is still red on main (then B).
- Checked: still green, so A. No product code changed.
- B49's fix (9681bc3ac) is still in `crates/openmx/src/lobby_ride.rs`: `next` is native only, and the wasm build
  gets a stand-in that returns "a headless lobby ride needs the native app".

## What the original does

Not applicable: a build question, no game logic.

## How it was proved

`tools/web.sh --check` on this branch (main at 324941bbd, branch up to date with it): builds to
target/web/pkg, exit 0. Only warnings (unused imports in game-render), no errors.

## Not verified / not 1:1

- The 13 browser tests (web.rs, netplay_browser.rs) were not run on their own; `./kit done`'s gate runs them.
- CI's `web` job (ship features, wasm-opt) not re-run here; the local check is the dev build.
