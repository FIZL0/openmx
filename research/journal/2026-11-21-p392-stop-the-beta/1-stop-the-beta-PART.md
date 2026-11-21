# 1 — stop the beta (PART)

Task P392. 2026-11-21. User: 'stop deploying beta builds and remove the beta links from repo'.

## What I did

- `.github/workflows/pages.yml`: the beta job is gone, and with it the `BETA_DISPATCH_TOKEN` secret's use and the
  FIZL0/openmx-beta draft/dispatch. The `site` job is unchanged except its `tag_name != 'beta'` clause: a published
  normal release still goes on openmx.dev. The concurrency group for pushes is `pages-main` now.
- Decided (unattended): pushes to main still run `ci`, `web`, `linux` and `windows`, as checks that the player builds
  still build. Nothing deploys from them; their artifacts stay on the run only. Why: ci.yml has no push trigger of
  its own, so dropping these jobs would end CI on main, and player_build.rs keeps checking these builds are native.
- Deleted `tools/beta-bundle.sh` and `tools/beta-site.yml`.
- `web/index.html`: the beta-host noindex script and the `BETA` switch are gone; downloads always point at the latest
  GitHub release; the build line says "Build …". `web/robots.txt`: no beta mention.
- `tools/release.sh`: no "try the beta" line in release notes, no beta in its header comments.
- `crates/openmx/src/update.rs`: only a build with a release tag (`OPENMX_TAG`) checks, and only for releases. The
  `GITHUB_SHA` path, the commit.txt fetch and its test are gone. Download with no release found opens
  `https://github.com/FIZL0/openmx/releases/latest` (was the beta's download page).
- `crates/openmx/tests/player_build.rs`: wording, and a check that pages.yml names no beta and still deploys the site.
- FIZL0/openmx-beta left alone, as asked.

## How it was proved

- `git grep -i -E 'beta\.openmx|openmx-beta|beta-bundle|beta-site|BETA_DISPATCH'` outside research/plan/journal: empty.
- pages.yml parses as YAML; `./kit check --all` (update.rs unit tests, player_build).

## Not verified / not 1:1

- pages.yml not run on GitHub: no push to main from a task branch. That a release still deploys openmx.dev is read
  from the file (the `site` job is untouched apart from the dropped beta-tag clause), not seen.
- "Treat every build as a release build": read as "only release builds check, and only releases". A build with no
  tag (local, or a GitHub Actions one) still never checks: it has no version to compare, and an untagged local
  release build that checked could replace itself in target/. Not a person's ruling.
- Players already on a beta build keep asking beta.openmx.dev's commit.txt (old binaries); nothing here can change
  that. If that site stays up, they see whatever it last held.
- Not touched (outside this task's files): ci.yml's first comment ("The checks a beta waits for"), menu.rs's
  build_label doc ("beta or local build"), tools/ios-build.sh / ios-ipa.sh comments about "pages.yml's beta IPA"
  (already stale since 11-03), plan/HUMAN.md's beta dispatch token and DNS steps (plan/ is excluded).

## Next

Done when the gate passes.
