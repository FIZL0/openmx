# K22 — ci watch

2026-11-02. Task: `./kit plan K22`.

K22 asked for main's GitHub Actions to be watched, so that a broken CI becomes one task instead of an email
on every push. It is built into agent-runner (0.7.3, `[run] ci_watch`) as a generic setting, not as openmx
code. While the run loop is up, it reads the newest finished run of pages.yml on main every 5 minutes.
When that run is red, it files one critical B task with the failed jobs, and puts the tail of the failed
log in `plan/<ID>.md`. It files no second task while that one is open, and it never holds up a landing.
openmx turns it on in `ps2kit/runner.toml`. It was checked against the past red run 41456844854 through
the live `gh`. The installed agent-runner tool still has to be reinstalled, with the loop restarted,
before it runs here. Details: 1-ci-watch-FINAL.md.
