# B49 — web lobby ride next

2026-11-21. Task: `./kit plan B49`.

CI's `web` job (pages.yml) stopped compiling: N27's headless lobby ride waits on `Net::next`, which the web
build's openmx-net doesn't have. Its wait is now native only, with an error-returning stand-in for the page
(which never takes `--lobby`). `tools/web.sh --check` builds again; CI re-runs once this lands.
