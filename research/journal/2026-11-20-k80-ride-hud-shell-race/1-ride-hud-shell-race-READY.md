# 1 — ride hud shell race (READY)

Task K80. 2026-11-20. Concept task: research and an options page, no product change.

## What I did

- Read crates/openmx/tests/ride_hud.rs for anything that reads the wall clock, shares a file or port, or
  runs another process.
- Every test but one is pure: `ride_hud::draw`/`sprites`/`Tracker` take `now` as an argument, and
  src/ride_hud.rs, overlay.rs and hud.rs never read the clock themselves (`grep Instant::now` finds none).
  The `Instant::now()` calls in the tests are only a base `t0` that the rest is offset from.
- The one that is not: `each_overlay_draws_its_own` (via `first_frame`, ride_hud.rs:707). It runs the
  real `openmx` shell four times (software and gpu, `--overlay none` and `hud`) and diffs the PNGs. It is
  the copy tests/hud.rs had before K46 (2026-11-09):
  - `current_dir` is the checkout root, so the shell reads that checkout's `settings.txt`,
    `controls.txt`, `mods/`, `markers/` (the main checkout has all four, the user's live ones: track
    free06, render_scale 50, tricks mxvatv, plane modifiers on 11-20).
  - no `--steady-clock`, so the two runs it compares ride different spans of wall clock on a busy
    machine (K46's memory: 0.1 s in one and 0.2 s in the other at load ~40).
  - the `gpu` half shares the machine's GPU with every slot's GPU tests and emulators.
- K46 fixed exactly this in tests/hud.rs: a store of its own under `CARGO_TARGET_TMPDIR` (a `kit.toml`
  naming a symlink to the disc) and `--steady-clock`. hud.rs has not red since.
- hud.rs already proves "the HUD is drawn over the ride frame, both renderers"; ride_hud's shell test adds
  only the score bar's and the meter's boxes.
- Options and a recommendation are on the artifact page (see `./kit done` line).

- Audit skipped: its hits are all pre-existing lines on main (camera.rs, replay.rs, plan/…); this diff is
  only this journal.

## What the original does

Not applicable: test tooling only.

## How it was proved

Not proved yet (concept task). The cause is by reading and by K46's precedent on the same pattern.

## Not verified / not 1:1

- Which assertion red in P383's merge gate: the gate's output was not kept (`.runner/run.log` and the slot
  logs hold no test output for it), so I could not tell a failed shell run from a pixel count short. Both
  come from `each_overlay_draws_its_own`, the only test in the binary not pure in its inputs.
- Not reproduced under load: a concept task does not change code, and six unrelated binaries red in that
  one gate (K80–K85), which points to a very loaded machine more than to one bug.

## Next

The person picks A, B or C on https://claude.ai/artifact/U8dzNU1XqoQpYYr7FLoRWw (recommended: A, copy
K46's own store + `--steady-clock` into ride_hud.rs's `first_frame`). This repo's `./kit done` has no
`--artifact` flag, so K80 is parked `--blocked "DECIDE: …"` with the link instead; the next session on K80
builds the picked option.
