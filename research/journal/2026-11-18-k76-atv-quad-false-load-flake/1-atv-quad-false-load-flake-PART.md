# 1 — atv quad false load flake (PART)

Task K76. 2026-11-18.

## What I did

- `-p game-sim --test atv_quad` races nothing. Its "failed under load, passed alone" in S328c's merge gate was
  MD245's deterministic red, then a retest that skipped every test.
- The run log (`context/notes/run.log`, 2026-11-18 01:07:47 – 01:08:33) shows it:
  - The gate ran on the merge built ahead, in `openmx-slots/merge2` (context links to the main
    `context/`). Its failure file (`context/notes/gates/de37724955bb…-3dc977c3977c.failed`) holds one panic:
    `mxu::the_quad_rigged_from_its_files_is_md123as` at `tests/atv_quad/mxu.rs:652`, the inverse-inertia
    bound MD245 is about.
  - The runner's alone retest ran in `openmx-slots/merge`. Its `context/` is the hand-made folder of
    per-entry links from 10-21 (memory: merge-worktree-needs-kit-toml), so every context folder made since
    (`mxvatv-ps2`, `mxvatv-pc`, `untamed`, `tapes`, `c34`, `s195`, …) was missing there. All 123 atv_quad tests
    took their skip path: `merge/context/notes/check.log` says `123 passed … finished in 0.14s`, and
    `check-times.json` records the binary at 0.1 s (it takes ~5 min with the images).
- Alone in this slot (full context, no other load from me), `./kit check -- -p game-sim --test atv_quad` fails
  on that same test and nothing else.
- Fix (outside git: the runner's merge checkout is machine setup): linked each main `context/*` entry missing
  from `openmx-slots/merge/context` (20 links; `notes` left as merge's own folder, as on 10-21).
  Running the built atv_quad binary with `KIT_ROOT=…/merge` and the filter `mxu::` now fails on MD245's test
  as a slot does (10 passed, 1 failed) instead of skipping.
- No repo code changed: nothing in the test needed fixing for load. The red itself is MD245's (slot 1 has it).

## What the original does

Not applicable: test harness and runner setup only.

## How it was proved

- The gate's failure file and the runner log above (same panic, same line, as the slot's solo run).
- The merge checkout's retest log and timing (0.14 s, 123 skips) against the solo run in a slot (minutes,
  real failure).
- After the links: the same binary with `KIT_ROOT` at the merge checkout reports MD245's failure.

## Not verified / not 1:1

- Did not run atv_quad under heavy load (busy loops) to rule out a second, real race: the one failure
  recorded is fully explained by MD245 plus the empty retest, and the binary has no wall clock, ports,
  threads or written files (grep of `tests/atv_quad*` and `game-sim/src` for `Instant`/`SystemTime`/sockets/
  file writes found none).
- New top-level context folders made later will again be missing from `openmx-slots/merge/context` (it links
  entry by entry). Not made durable: the runner (agent-runner) lives outside this repo.
- The task's gate (`./kit check --all`) cannot pass on this branch until MD245 lands, because MD245's test is
  red on main; finished `--blocked --after MD245`.

## Next

Once MD245 merges, rerun `./kit done K76`: the gate should pass with atv_quad running for real.
