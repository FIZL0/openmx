# P392 — stop the beta

2026-11-21. Task: `./kit plan P392`.

The user asked to stop deploying beta builds and remove the beta links. pages.yml's beta job, tools/beta-bundle.sh
and tools/beta-site.yml are gone; the page, robots.txt and release notes no longer name beta.openmx.dev; the game's
updater only checks releases, and only in a release build. openmx.dev still gets each published release; main's
pushes still build (as checks) but deploy nothing. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).
