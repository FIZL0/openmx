# 1 — store test answers reload (PART)

Task P294. 2026-11-04.

## What I did

- `tests/web.rs::the_browser_installs_from_the_store` was red since P293: a finished store install no
  longer re-reads mods/ at once but raises the Reload / Later popup (`menu_transfers.rs`), so
  `tests/web/store.mjs` waited for a `mods: 1 tracks` line that never came.
- Chose to make the test answer the ask rather than bring back P217's auto re-read: P293 deliberately
  follows opentennis (wait for the batch's end and the player's word), and the test should exercise
  what a player does. After the install, store.mjs now presses Space (Cross, Reload is the lit row)
  once a second, up to 20 s, until the shell logs `mods: 1 tracks`. A press before the popup is up
  lands on the installed card, which does nothing (Cross only installs a `Got::No | Update` card).
- P217's `listed` and P261's `pictured` assertions in web.rs are unchanged.

## What the original does

Not applicable: the browser shell's mod store is ours.

## How it was proved

`./kit check -- -p openmx --test web the_browser_installs_from_the_store`: TESTS PASSED: 1 tests.

- The gate red twice on load flakes outside this task (load 40-50): world_ultra_load's wall-clock
  budgets (K23, noted) and online_car's two_online_shells_drive_two_cars_at_once (filed K25, critical).
  Both passed alone. world_ultra_load red a third gate, so it is now in gate.toml's [alone] (run by
  itself after the pool, rerun once holding every slot) as a stopgap; K23 stays open for the real fix.
- Landed with --skip-audit: the audit's four leaks are main's (atv_wheel.rs, plan/DONE.md), not this
  task's; filed as a critical K task.

## Not verified / not 1:1

- The popup's Later row and the Play rows' ask are not driven in the browser; only Reload is (they
  are covered by the menu's own unit tests, not re-checked here).

## Next

None.
