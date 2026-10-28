# S153 — idle rollback wreck

2026-10-26. Task: `./kit plan S153`.

The user saw an idle bike roll backwards and the rider bail. The port does not reproduce this on open
ground. Three sweeps of 2312 idle drops (10-26; 10-27 on e9fc4d5a; 10-28 on 994b805b, after S151/S152)
turn up only impacts: path cars pushing, knocking or carrying a parked bike (since S151 they are
movers), falls off ledges, and fast backward rolls into bumps. One capture of the original's idle
roll-back on free09 rides wreck-free, and the port matches it (`the_captured_idle_roll_back_never_wrecks`).
Done 10-28. The comparisons with the original are S204 (path cars), S162 (bumps) and S170 (the train).
