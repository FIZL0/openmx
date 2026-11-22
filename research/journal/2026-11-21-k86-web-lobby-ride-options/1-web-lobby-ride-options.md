# 1 — web lobby ride options (PART)

Task K86. 2026-11-21.

## What I did

- Concept task: no product code changed. Options page: https://claude.ai/artifact/FzhDV8FSDR3ycCpejgrvYe
- Found K86 already fixed on main by B49 (commit 9681bc3ac): `lobby_ride.rs`'s `next` is native only, and the
  wasm build gets a stand-in that returns "a headless lobby ride needs the native app". `--lobby` is a shell
  flag, so a page never reaches it.
- Options laid out: A keep B49's stand-in and close K86 (recommended: no work, no merge risk); B cfg the whole
  `lobby_ride` module out of wasm (about 10 spots in main.rs: the `mod`, two `Args` fields, the `--lobby`
  parse, the start-up block and the ride's `lobby_start` reads; busiest file, merge clashes); C make `meet` a
  per-frame poll step the page drives (big, and nothing uses a headless lobby in the browser).
- Nothing taken from ../opentennis: it has no headless lobby ride.

## What the original does

Not applicable: a build question, no game logic.

## How it was proved

`tools/web.sh --check` on this branch (main at 7ccc31f01) builds to target/web/pkg, exit 0.

## Not verified / not 1:1

- The 13 browser tests (web.rs, netplay_browser.rs) were not run on their own here; only the wasm build they
  need. `./kit done`'s gate runs them.
- CI's `web` job (ship features, wasm-opt) not re-run here; the local check is the dev build.

## Next

A person picks an option on the page. If A, the build task is a no-op: close it.
