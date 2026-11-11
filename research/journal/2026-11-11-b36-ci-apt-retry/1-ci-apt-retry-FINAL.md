# 1 — ci apt retry (FINAL)

Task B36. 2026-11-11.

## What I did

- Read the logs of the red `pages.yml` run 43032578762 (2d145981) and of the next one, 43033165818 (ec16d514, which
  was also red). In both, `windows` and `ci / test` fail in their apt step and never reach cargo: the runner couldn't
  resolve `azure.archive.ubuntu.com` for any mirror (security/archive), so `apt-get install` exited 100. The outage ran
  from 13:49 to 14:06 UTC. `linux` and `web` install nothing with apt, so they stayed green. The runs before
  2d145981 (13:16 and earlier) were green on the same steps. No code was at fault.
- Added `tools/ci-apt.sh <packages>`: apt update + install with `Acquire::Retries=3`, the whole thing retried after
  pauses of 0/30/60/120/240/420 s, about 15 minutes in all, then exit 100. `ci.yml`'s libasound2-dev step and
  `pages.yml`'s MinGW step now call it. A short DNS outage turns into a slower job, not a red main.
- The gate (`kit done`) was red in a test I never touched: `tests/mxvatv_car_frame.rs`'s
  `all_six_pieces_are_mx_unleasheds` and `all_six_pieces_with_the_tire_step_are_mx_unleasheds` fail every run,
  "preload: fired, degenerate seen only 17 (18) times" against the per-branch floor of 20. The bit-exact
  comparison itself passes. Main has been red here since ec16d514, MD126's merge, which resolved a conflict in that
  file. The new controls piece changes the generated stream, and that branch is rare. A seed scan at 3,000 frames
  gave 11–22 for the six-piece run (5 of 8 seeds under 20). Nothing on main or on any task branch fixed it (MD127
  touches the file but not these tests). So I added `pieces_over(cases, …)` (`pieces` = 3,000), and the two six-piece
  tests run 6,000 frames. That gave 30–35 and 40–49 over three seeds each, with the generator and the other tests'
  streams unchanged. The file's 13 tests pass, about 10 s more in all.
- Took nothing from opentennis: its workflows run plain `apt-get` too (checked ci.yml and pages.yml).

## What the original does

Not applicable (CI only).

## How it was proved

- Both workflow files still parse as YAML (python yaml.safe_load). `bash -n tools/ci-apt.sh` passes.
- `./kit check --all` (the gate in `kit done`); it doesn't cover the workflows.

## Not verified / not 1:1

- The new step hasn't run on a GitHub runner yet. It first runs on the merge to main, which is the proof.
- If an outage outlasts the ~15 minutes of retries, the run is still red. Then it's just a re-run, not a code fix.
- The car frame floor fix only gives the rare branch more frames. "Preload: fired" itself sits near 25–37 per 3,000
  frames, so a later change to the generator could bring a 3,000-frame `pieces` run near the floor again.

## Next

Check that the first pages.yml run on main after this merge is green.
