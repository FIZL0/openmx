# 1 — mod store index (FINAL)

Task T1. 2026-11-17.

## What I did

- Ran `openmx --mod-store-admin index` (release build of this branch) with the FIZL0 token in
  `~/.config/openmx/hf-token.txt`. An `index` commit to `main` is allowed from FIZL0; only mod uploads must
  come from mxunleashed.
- Committed `index.json` to FIZL0/openmx-mods `main`: 108 mods, all `refs/pr/1`, none verified, PR #1 pinned
  at commit `0ef66d02…`. Commit:
  https://huggingface.co/datasets/FIZL0/openmx-mods/commit/0ed5ec4ffc5edfa2d8c28a093fda501329e45c5e
- No code change: the admin command and the client's index path (M116) were already there.

## What the original does

Not applicable: the mod store is an openMX addition.

## How it was proved

- Before: `resolve/main/index.json` answered 404. After: it serves version 1, `pulls {"1": {commit}}`,
  108 entries. `refs?include_prs=1` shows PR #1's head = the index's commit, so the client keeps the
  index's copy.
- Live desktop test (`cargo test -p openmx --bin openmx live_store -- --ignored --nocapture`) with a
  temporary, uncommitted print in the store's GET: the listing made 3 requests (index.json, open PRs,
  refs) instead of ~110, and still listed 108 mods. Install, update, remove and Report passed.

## Not verified / not 1:1

- The browser listing (`the_browser_uses_the_live_store`) was not run: it needs the disc, node and
  chromium. It shares the same listing code as the desktop, so it should also drop from ~218 requests to
  about 3 plus redirects, but I did not count them.
- The index goes stale as soon as PR #1 (or any new PR) gets a new commit. Clients then read that PR's
  metas one by one again until someone re-runs `--mod-store-admin index`. Nothing re-runs it on its own.

## Next

Re-run `openmx --mod-store-admin index` after store changes (new or updated PRs, merges to `main`).
