# F66 — measure the wreck check

2026-11-17. Task: `./kit plan F66`.

F31b5s's out-of-bounds oracle was shown to really run (12,000 cases, every path, 0 differ), and the
`"wreck check"` switch was measured on the pure replays: off, ride-2 / flip-1 run 20,655 / 20,925
interpreter instructions, all in the check (45 a frame); on, 0.
