# S64 — wheel start state

2026-10-22. Task: `./kit plan S64`.

S64 starts the acceptance run from the save state's whole bike. That means both wheels' spin-step words
and the air state, on top of S8's chassis, drivetrain and balance. It compares the run against a new
quarter-speed capture of the chassis and the tire half of each wheel (`watch/s64-wheels.toml`). Two
things made the brief's tick-1 gap:

- The unseeded air state, which started the port airborne and freed the roll axis for a tick.
- The ride's ground query, now the original's triangle split instead of a bilinear blend.

With both fixed, tick 0 agrees on every lane. The bike's position, which used to part at tick 1 by
0.00086, now agrees through tick 21, and in `x`/`z` on all 118 ticks. Tick 1 still parts by ulps on the
lateral terms, and by 1.3% on pitch acceleration and in the roll quaternion, for a reason not found.
Details in 1-wheel-start-state.
