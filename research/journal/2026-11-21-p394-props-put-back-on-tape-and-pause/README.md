# P394 — props put back on tape and pause

2026-11-21. Task: `./kit plan P394`.

S475's Props Put Back switch now rides on a tape as a `props_put_back FRAME on|off` line, so a tape ridden
with it replays with it, and a change made under the pause takes as the pause closes on every seat's prop
set (split screen too), off included. Proved by game-sim and menu tests and a headless record/replay on
free09; the pause was not ridden end to end in the shell (see the FINAL file).
