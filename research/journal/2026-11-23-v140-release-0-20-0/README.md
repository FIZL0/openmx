# V140: release 0.20.0

Taken in a control session after the run was stopped and idle.

- The run had been started bare, not with `./kit run`, so P346, V63, C46, F83, F85, S508 and S504 merged with no
  gate. `./kit check --all` on main with them (b3887e476) passed: 7509 tests. A first try was void: the take
  worktree had no kit.toml, disc or context; they were linked from the main checkout.
- RELEASE_NOTES.md's bullets written from the commits since v0.19.0.
- `PATH=$HOME/.cargo/bin:$PATH OPENMX_RELEASE_BRANCH=task/V140 tools/release.sh 0.20.0 publish`: v0.20.0 published
  with 11 assets, https://github.com/FIZL0/openmx/releases/tag/v0.20.0. pages.yml's release run and FIZL0/apps
  passed; openmx.dev/version.txt reads `v0.20.0 5fd7d65`.

**Not verified**: the downloads were not started on any platform.
