# 1 — split ai and car frame tests (FINAL)

Task K49. 2026-11-11.

## What I did

- `crates/game-sim/tests/ai.rs` (5486 lines) → `tests/ai/main.rs` + 14 topic modules, one per section the
  file already had (its `// ---` banners): `roster` S12a, `controls` S12b (was the inline `mod controls`,
  so its test paths are unchanged), `tricks` S12d, `avoidance` S12c, `brain_copy` S12e, `path_queries`
  S282 (not `path`: that name is `game_sim::ai::path`, imported at the root), `character` S283, `tick`
  S281, `seek` S282a, `follow` S282b, `path_nets` S282c, `path_branch` S309, `callouts` S312, `seek_world`
  S313. main.rs keeps the header, the shared `Pick`/`named`/`BUDGET`/roster layout, and every `use` the
  file had (the ones from mid-file sections were hoisted: they were crate-root items already, and later
  sections lean on them, e.g. `Vf`).
- `crates/game-sim/tests/mxvatv_car_frame.rs` (3599) → `tests/mxvatv_car_frame/main.rs` (the shared
  readers, generators, hooks and tag sets, lines 1–2411 unchanged) + `pieces` (MD106/MD106a tests and
  `run`), `update` (MD106d), `own_calls` (MD106b; not `own`, which is an inline address module) and
  `whole` (was the inline `mod whole`, its body now the file, de-indented four spaces as `controls` is: test paths unchanged).
- Cargo auto-discovers `tests/<name>/main.rs` as test target `<name>`, so both are still one binary of the
  same name: gate.toml entries, `--test ai`, check-times keys all match as before. Test paths inside a
  topic gain the module (`ai::tick::…`); gate.toml's `[flaky]` patterns already allow `(.*::)?`, and none
  names these binaries.
- Helpers a later section borrows from an earlier one are `pub(crate)` in their own topic (fns, consts,
  the `Pick` methods `range`/`chance`/`vec`/`fraction`, and the fields of `TickScript`, `Played`,
  `OwnPtrs`), and main.rs globs those topics in (`use tick::*;` …), so every topic sees them through its
  `use super::*;`. Both main.rs headers say: a new task adds its tests in a topic file of its own.
- `ps2kit/cargo_gate.py` select: a file in a test root's own folder (`tests/ai/` of `tests/ai/main.rs`,
  or `tests/atv_quad/` of `tests/atv_quad.rs`) now reaches that binary alone. Without this, editing a
  topic file counted as "a tests/ helper" and selected all of game-sim and every crate using it. Checked
  by calling `select` with a stubbed `changed`: `tests/ai/seek.rs` → `{game-sim: {ai}}`,
  `tests/atv_quad/frame.rs` → `{atv_quad}`, `tests/players/mod.rs` (a shared helper, no such target)
  still → the whole package and its users.
- `atv_car.rs`'s `Proof:` doc line points at the folder.

Nothing taken from opentennis: it has no test file this size to split.

## How it was proved

- Move only: every non-blank line of the two old files (trimmed, with `pub(crate) ` dropped from the new
  side) is in the new files and vice versa, apart from the `mod`/`use` lines the split adds and the two
  inline `mod controls {`/`mod whole {` openers and their closing braces (sorted-line diff).
- Same tests: 31 `#[test]` in ai, 19 in mxvatv_car_frame, before and after, with the same names.
- Same compiler warnings as before (10 unused consts, two needless `mut`, one unused box — all old).
- `./kit check -- -p game-sim --test ai --test mxvatv_car_frame`: TESTS PASSED, 50 tests.

## Not verified / not 1:1

- Whether any of the 50 skipped rather than ran: a passing `./kit check` prints no log, and the tests
  skip without their images; the move does not change what they need, so a skip before is a skip now.
- `research/oracle.txt` comments and open plan task texts still say `tests/ai.rs` /
  `tests/mxvatv_car_frame.rs`. Left alone on purpose: oracle.txt is edited by many tasks at once (a
  merge-collision file itself), the plan texts must not be hand-edited, and the folder of the same name is
  easy to find from the old path.
- The selector change is checked by hand-calling `select`, not by a test: ps2kit has no test suite.

## Next

Nothing.
