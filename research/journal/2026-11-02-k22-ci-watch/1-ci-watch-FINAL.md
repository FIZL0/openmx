# 1 — ci watch (FINAL)

Task K22. 2026-11-02.

## What I did

- **Where it lives: agent-runner, as a generic hook, not openmx code.** Nothing about it is openmx's. The
  run loop is what pushes main, already commits plan follow-ups in the main checkout (`reopen_landed`,
  flaky tests), and is the one process that stays up. A `./kit` wrapper would have to fork that loop, and
  opentennis and openmx-mods get it too by adding a single config line.
  agent-runner 0.7.3 (commit e48feed, pushed to FIZL0/agent-runner main):
  - `agent_runner/ci.py`. `newest_finished` asks `gh run list -w <wf> -b <branch>` for the newest
    *finished* run. Cancelled and skipped runs say nothing, so the run before them decides. pages.yml
    has `concurrency` and GitHub cancels the queued runs in between, so a per-push "this sha's run" can
    vanish. `failed_jobs` comes from `gh run view --json jobs`. `log_tail` gives the last 40 lines of
    `--log-failed` up to the last `##[error]`, as `job: text`: it drops the timestamp and the git
    cleanup that comes after the error. `to_file` judges each run once (`<notes>/ci-<wf>.seen`), and
    it files nothing while an open task's line says `` `pages.yml` on main is red ``.
  - `runner.watch_ci(branch)`, called once per tick of the run loop:
    - It does real work at most every 300 s, and only when it gets MERGE without waiting. It never holds
      up a landing, and the loop never waits out a gate for it.
    - It leaves things alone when the main checkout is not on the branch or plan/ has uncommitted
      edits, the same rule as `reopen_landed`.
    - It files with `P.add(ci_kind, …, pri='critical')`, writes the long form `plan/<ID>.md` (run URL,
      commit, failed jobs, log tail in a code block) and commits both as `Plan: <ID>, `pages.yml` on main
      is red`. If the commit fails it puts the `.seen` back, so the next look tries again.
    - The task goes out with the next merge's push. Pushing it on its own would mean pushing main
      outside a gate.
  - The settings are `[run] ci_watch = []` and `ci_kind = "B"`. agent-runner's README and CHANGELOG
    cover them.
- openmx: `ps2kit/runner.toml` `[run] ci_watch = ["pages.yml"]`. pages.yml calls ci.yml as its `ci`
  job, so a ci.yml break shows up as pages.yml's `ci / test` job. Since 033a5fa8, ci.yml no longer runs on
  its own for main pushes.

## What the original does

Not applicable (tooling).

## How it was proved

- **The past red run, 41456844854**, through the live `gh` in this repo: `ci.to_file` with `run list`
  replaced by that run's own JSON (plus a newer in-progress one) and every other call real.
  - It returned the task line `` CI: `pages.yml` on main is red (failure, run 41456844854 on 6cee61ea,
    2026-11-02 13:53 UTC): failed ci / test. … ``.
  - Its long form ended with the alsa-sys pkg-config error and `##[error]Process completed with exit
    code 101.`, with no post-job cleanup after it.
  - A second call returned None (already judged). With an open task holding the marker it also returned
    None.
- **The live call** `newest_finished(pages.yml, main)` returned 41467245448, a success. That is correct:
  3b05a6ba fixed it.
- **agent-runner tests/test_runner.py**: the new `test_ci_watch` runs in its throwaway repo with canned
  `gh` answers. It checks:
  - the throttle;
  - a red newest finished run behind a cancelled one files one critical B task with the long form,
    committed, plan/ clean;
  - the same run is not filed twice;
  - a newer red run files nothing while the task is open, but is still recorded as judged;
  - a green run files nothing;
  - an uncommitted TODO.md is left alone, with no `gh` call;
  - a red run after the task closes files again.
- The whole agent-runner suite passes: test_runner (all checks), test_plan, test_pi, before and after
  the rebase onto 0.7.2.

## Not verified / not 1:1

- **The live run loop has not filed anything yet.** The installed tool here is still 0.7.0. It is
  installed from `~/repos/agent-runner`, so it needs
  `uv tool install --force --python /usr/bin/python3 ~/repos/agent-runner` and a loop restart, which
  the memory notes say to do only between merges. Not done: a runner swap mid-run kills the live
  sessions' Stop hook, and this session runs under that loop. Until then the new
  `ci_watch` key in runner.toml is ignored, which is harmless.
- **Repeat emails are not stopped.** GitHub still sends its failure email for each red push. This
  change turns the break into a task that gets fixed first, and it reads no inbox. Turning off those
  emails is a GitHub notification setting for the user (Settings → Notifications → Actions → "Only
  notify for failed workflows" / off).
- **A first look with no `.seen` file judges whatever run is newest.** If main is red at that moment, a
  task is filed for it. That is intended, but it means a fresh install files for a break that is
  already known.
- **A missing or logged-out `gh` is silent.** `newest_finished` returns None, so nothing is filed and
  nothing is logged.

## Next

None.
