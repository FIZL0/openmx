# 1 — plan add letter ids (FINAL)

Task K44. 2026-11-16.

## What I did

- The refusal lives in agent-runner (`agent_runner/plan.py`, `check_kind`), not in this repo: `ps2kit/plan.py`
  only hands `./kit plan` to it. Fixed it there and pushed agent-runner `66a5a9f` (version 0.34.1).
- `plan add <ID> …` now takes a task id ending in a letter (MD39b) as KIND when it is an open task's id;
  `next_id` already gave MD39b1. One ending in a digit is taken as before. A letter-ending id that is no open
  task (a typo) is still refused when the repo names its kinds, as openmx does.
- agent-runner's `tests/test_plan.py` gained two checks: an open `Q1a` files `Q1a1`; an unknown `Q9z` is refused.
  The whole file passes (run with this session's `AGENT_RUNNER*`/`KIT_*` variables unset: they point the test's
  scratch repos at openmx's runner config and fail ~20 unrelated checks).
- No change in openmx. Decided against a shim in `ps2kit/plan.py`: it would duplicate the fix and go stale.

## What the original does

Not game work: kit tooling only.

## How it was proved

agent-runner `tests/test_plan.py`, all checks passed, including the two new ones.

## Not verified / not 1:1

- `./kit plan add MD39b …` in this checkout is not tried end to end: the installed tool
  (`~/.local/share/uv/tools/agent-runner`, from git) is still 0.34.0. I did not upgrade it mid-run; the runner's
  own notice says to upgrade after a run, since the other slots import from that venv.

## Next

- After the run: `uv tool upgrade agent-runner`. Then `./kit plan add <letter-ending open id> …` files `<id>1`.
