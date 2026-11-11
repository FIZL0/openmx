# 1 — pages red was pre b36 dns (FINAL)

Task B37. 2026-11-11.

## What I did

- Read the failed run 43061339843 (on 0bca3c37, the S306 merge). Both red jobs, `windows` and `ci / test`, died in
  their apt install step with exit 100: every Ubuntu mirror "Temporary failure resolving 'azure.archive.ubuntu.com'"
  from 16:47 to at least 16:54 UTC. No build or test ran; no code was at fault.
- That commit is the one *before* B36's merge (e12a873cd), which added `tools/ci-apt.sh` (apt update + install
  retried six times over about 15 minutes of pauses) and moved both apt steps onto it. So the red run had no retry.
- Waited for the next `pages.yml` run on main, 43074562720 on e12a873cd (with the retry): every job green —
  `ci / test`, `windows`, `linux`, `web`, `beta` success (`site` skipped as usual). Its log has no
  `ci-apt: install … retrying` line, so the DNS outage had ended by then and the retry was not exercised.
- No code change: B36 already is the fix. Nothing else to do in this task.

## What the original does

Not applicable (CI only).

## How it was proved

`gh run view 43074562720 --json jobs`: all jobs success on main's head e12a873cd.

## Not verified / not 1:1

- That `tools/ci-apt.sh` actually rides out a DNS outage on a runner: the green run hit no outage (no retry line in
  its log). An outage longer than the script's ~15 minutes of pauses (plus per-attempt DNS timeouts, ~3–5 min each
  here) would still go red; the 13:49–14:06 one was ~17 minutes, inside that total.

## Next

Nothing.
