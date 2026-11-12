# 1 — since merge gate (FINAL)

Task K50. 2026-11-12.

## What I did

- `./kit check --since <rev>` (ps2kit/checks.py): a whole run picks the tests with gate.toml `[select]` from
  what changed since `<rev>` (commits after it plus the tree), not since the shared branch. It is the same
  `cargo_gate.select` that `./kit done` and a bare check use; only the ref it diffs against changes. Slow
  binaries follow the same rule (run only if their `paths` were touched since `<rev>`). It takes the check
  slot first, as `--all` does, since it is now the merge gate every finished task waits on. `--all` beside
  `--since` wins; a `<rev>` that is no commit here prints a line and tests everything.
- ps2kit/cargo_gate.py: when `[select]` ends up choosing everything, the summary now says why
  (`ran every test binary: gate.toml is outside every package` / `git cannot say what changed since …` /
  `the change reaches every package`). Unknown files (outside every workspace package, not `ignore`d), the
  kit itself and a gate.toml change already selected everything; now it is visible in the one-line verdict.
- ps2kit/runner.toml: `[run] test_since = "./kit check --since {base}"` and `full_every_hours = 24`
  (agent-runner 0.24.0, installed here). The runner passes `{base}`, the shared-branch commit the merge is
  on, which already passed a gate; `test = "./kit check --all"` still runs once 24 h pass with no full pass.
  The runner's `retest` (`./kit check --rerun …`) is unchanged and filtered anyway.
- gate.toml's `[select]` comment and cargo_gate's docstring say the merge gate is `--since` now.
- tests/test_gate.py `test_select`: gate.toml change → everything, with the reason; two commits (a library
  change, then a test-only one) → `--since` the first gives the test binary alone, the shared branch gives
  the library's users too; an unknown `<rev>` → everything, with the reason.

The user's question (11-12: "is this something to include in the agent runner .md … how can we share this
efficiency"): it is already in agent-runner 0.24.0's README, "Making it fast" → "Test what changed on
merges, all of it daily", which is the general recipe (`test_since` + a selection the project writes);
this task is openmx's side of it. Nothing more to add there: the picking itself (`[select]`) is
project-specific, and ps2kit's copy is the worked example any kit-based project gets.

opentennis: not looked at for this — its gate has no selection to take from; this is ps2kit's own.

## How it was proved

- `python3 tests/test_gate.py`: 54 passed, 0 failed (the new `--since` cases among them).
- `./kit check --no-gate --since HEAD` on this worktree with everything committed: `TESTS PASSED (ran 0
  test binaries …)`, the CLI path end to end. (`--no-gate` because the slot queue at load ~35 held the
  plain run past 15 min; the slot code is unchanged apart from `first=`.)
- `ps2kit.runner.settings()['run']` shows `test_since` and `full_every_hours = 24` reach the generated
  agent-runner config.

## Not verified / not 1:1

- A real `./kit run` merge through `test_since` was not watched: it only happens after this branch lands
  and the runner restarts with the new config. The first merge after it should log the `--since` command.
- A merge's `{base}` being an ancestor of the merge commit is from agent-runner's code
  (`gate_cmd`), not observed; if it ever is not, `merge-base` still bounds the diff, and a missing commit
  falls back to everything.
- Not measured that a test-only merge now takes minutes: depends on the merge, the slot queue and load.

## Next

Nothing; done.
