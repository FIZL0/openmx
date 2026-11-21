# 2 — own store and steady clock (FINAL)

Task K80. 2026-11-21. The person picked A on the options page: copy K46's fix into ride_hud.rs.

## What I did

- `first_frame` in crates/openmx/tests/ride_hud.rs now does what tests/hud.rs's has done since K46:
  - each run gets a store of its own, `CARGO_TARGET_TMPDIR/p121-<name>/`, holding only a `kit.toml`
    and a `disc.iso` link to the configured disc, and the shell runs there. The checkout's
    `settings.txt`, `controls.txt`, `mods/` and `markers/` no longer decide the frame.
  - the shell rides `--steady-clock`, so the `none` and `hud` runs picture the same two sixtieths of a
    second of ride however long the machine took.
  - a failed run's message now carries the shell's stdout too, as hud.rs's does.
- Nothing taken from ../opentennis: the fix is this repo's own K46 code.

## What the original does

Not applicable: test tooling only.

## How it was proved

- `./kit check -- -p openmx --test ride_hud`: 96 tests passed.
- The shell test did run, not skip: all four `target/tmp/p121-*/shot.png` were written by that run.

## Not verified / not 1:1

- Not run under load: no reproduction of P383's gate failure existed to rerun (its output was not
  kept, see 1-…), so the fix rests on K46's precedent for the same pattern, which has not red since.
- The `gpu` half still shares the machine's GPU with other slots; K46 left that as is too, and hud.rs
  has held with it.
