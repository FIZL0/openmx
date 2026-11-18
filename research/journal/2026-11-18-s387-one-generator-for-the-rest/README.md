# S387 — one generator for the rest

2026-11-18. Task: `./kit plan S387`.

In a race, each rider's own draws now come off the race's generator, the one `rand()`. That covers the
edge throw, wreck clank and voice, and ragdoll wreck entry (all off the rider's pool), plus the riding
ragdoll's draws right after them. A plane's frame draws on it too. The body look-around, engine notes
and dust now pick the race's generator before the AI lane's. The quad and car lanes and vehicle_anim are
unchanged: none of them steps beside a race tick. One test proves the edge throw in a race. There is no
live capture of a race wreck. Details in 1-…-FINAL.md.
