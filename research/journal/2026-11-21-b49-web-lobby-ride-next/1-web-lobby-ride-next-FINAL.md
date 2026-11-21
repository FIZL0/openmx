# 1 — web lobby ride next (FINAL)

Task B49. 2026-11-21.

## What I did

- CI's `web` job (pages.yml, run 44877099458) failed to compile: `lobby_ride.rs` (N27's headless lobby ride)
  waits on `Net::next`, which openmx-net only has natively (a page can't block on the network). `main.rs`,
  and so `mod lobby_ride`, is also the web build's crate.
- `lobby_ride.rs`'s `next` is now native only; the web build gets a `next` that returns an error. `--lobby` is
  a shell flag, so a page never reaches it.
- The warnings above the error in the log (unused `mut`, unused functions) are warnings only; left alone.

## What the original does

Not applicable: a build fix, no game logic.

## How it was proved

`tools/web.sh --check` (the same wasm build the `web` job runs, without its release packaging) builds, and
`./kit check --all` passes natively.

## Not verified / not 1:1

- The CI run itself is not re-run here: it runs once this branch lands on main. Local `tools/web.sh --check`
  builds the same crate for wasm32, but not with `ship` features or wasm-opt.

## Next

none
