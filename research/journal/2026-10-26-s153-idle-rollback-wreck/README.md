# S153 — idle rollback wreck

2026-10-26. Task: `./kit plan S153`.

The user saw an idle bike roll backwards and the rider bail. The port does not reproduce this on open
ground: across two sweeps of 2312 idle drops (10-26, and 10-27 on e9fc4d5a), every idle wreck is an
impact. These are traffic driving through the rider's head on a road, a toy's kick, falls off ledges,
and fast backward rolls into bumps. One capture of the original's idle roll-back on free09 rides
wreck-free, and the port matches it (`the_captured_idle_roll_back_never_wrecks`). The task waits on the
user saying where it happened (READY).
