# 1 — beta dispatch token refused (PART)

Task B50. 2026-11-21.

## What I did

- Read the failed beta job (run 44880776180): `gh: Bad credentials (HTTP 401)` on the first call made with
  `BETA_DISPATCH_TOKEN`, then `base64: invalid input` and the misleading "deploy.yml is older" error.
- FIZL0/openmx-beta's `deploy.yml` is already tools/beta-site.yml's B43 one (read it with the local gh login):
  the workflow is fine, only the token is bad.
- The token was already refused at 01:04 (run 44847312592's beta job: "BETA_DISPATCH_TOKEN was refused (expired
  or wrong?)"), but before B43 that was a warning. B43 made the token required, so the job now fails.
  The secret was set 2026-10-28.
- `.github/workflows/pages.yml`: the beta job first checks the token with one plain call and, on a refusal, says
  "BETA_DISPATCH_TOKEN can't read FIZL0/openmx-beta (gh's error)", with what to do on a 401 (plan/HUMAN.md).
  B43 dropped the old refusal warning when it made the token required.
- `plan/HUMAN.md`: the token item no longer says it is optional (it is needed since B43) and says what to do when
  it expires.
- Wizard for the person: `context/notes/wizards/B50.sh` (make the token, set the secret, re-run the job).

## What the original does

Not applicable: CI only.

## How it was proved

- `pages.yml` parses as YAML. The new check's line, run here with `GH_TOKEN=bogus`, fails and captures
  `gh: Bad credentials (HTTP 401)` (the same text as the CI log); with this machine's login it passes.
- The check prints gh's own error, so a 404 or an outage isn't passed off as an expired token (spec review).

## Not verified / not 1:1

- The new refusal check has not run in CI: it only runs on a push to main, after this merges.
- That a new token makes the beta job green: needs a person to make one (fine-grained tokens can't be made by
  API). Why the old token is refused (expired early, revoked, or wrong value) can't be seen: secrets are
  write-only.
- I did not set the secret to this machine's own gh login token, which would work: it can write every FIZL0
  repo, too broad for a CI secret without the owner's say.

## Next

The person runs `./kit wizard B50`: a new fine-grained token (FIZL0/openmx-beta, Contents read/write) as
FIZL0/openmx's `BETA_DISPATCH_TOKEN`, then re-runs the failed beta job.
