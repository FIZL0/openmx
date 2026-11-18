# 1 — replay red was shared data (FINAL)

Task K78. 2026-11-18.

## What I did

- The red test was `the_dallas_rail_rides_replay_from_their_save_states` (gate file
  `context/notes/gates/df705ccb…failed`, the B34c batch's daily whole run at 10:47).
- No merge broke it, so there was nothing to bisect. C39 re-recorded `context/capture/s89/across.csv` (10:45)
  and `context/states/s89-across.eeMemory.bin` (10:42) in place, in the context every worktree shares. Main's
  replay test at 10:47 still found its world with `recorded_on`, and C39's new across ride sits close enough to
  Free11's dirt for that to pick the wrong area. C39's fix to the test (`in_the_world_named("Tracks\\free06.res")`,
  8c0ea50a4) merged at 11:10 with the same batch.
- On this branch (which has C39's merge) `./kit check -- --package=game-sim --test=replay` passes, 124 tests,
  with no code change.
- The selection fix: `gate.toml [select] context = ["capture", "states", "recordings"]`. `ps2kit/cargo_gate.py`
  `context_changed` lists files there written after the base commit's time, and `select` sends each to the test
  binaries naming its folder (`"s89"`) or its file's lead (`"s89-`). Checked: both s89 files reach replay.rs.
  `tests/test_gate.py` covers it (a new capture and state reach their tests; an older file reaches nothing): 74
  passed.
- Nothing in ../opentennis covers this (its gate has no shared-context selection).

## What the original does

Not applicable: kit work.

## How it was proved

- `./kit check -- --package=game-sim --test=replay`: TESTS PASSED, 124 tests.
- C39's test diff names the cause in its own comment (the across ride is near Free11's dirt).
- `python3 tests/test_gate.py`: 74 passed, with the new context case.

## Not verified / not 1:1

- I did not rerun main's pre-C39 replay test against today's s89 files to watch it fail. The timing (data at
  10:42–10:45, red at 10:47, C39's test change at 11:10) and C39's own comment are the evidence.
- The selection matches by mtime, not content. A file touched without a change selects its tests anyway; a file
  copied in with an old mtime (`cp -p`, `mv` from an old place) is missed.
- The words are a guess at how tests name their data. A test that builds a capture's path from a constant
  elsewhere (`private_dir("states").join(CHORD_STATE)`) is reached only if the constant's text holds the
  file's stem.

## Next

None.
