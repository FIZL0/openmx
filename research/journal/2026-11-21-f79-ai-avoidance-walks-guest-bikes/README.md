# F79 — ai avoidance walks guest bikes

2026-11-21. Task: `./kit plan F79`.

With several local seats, the first seat's AI riders' scene walk now lists the other seats' bikes after
its own players' (`Sim::ridden_guests`, filled by the shell before seat 0's pass). The brief said the
avoidance "does not sweep" them. In fact the original sweeps no bike: every bike is AI_IGNORE (S326). So
this only makes the walked list match the original's. The AI riders' controls do not change, and no
test can see it from outside the Sim. Details in 1-…-FINAL.md.
