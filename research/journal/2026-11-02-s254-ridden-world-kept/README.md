# S254 — ridden world kept

2026-11-02. Task: `./kit plan S254`.

The Deck's split screen spent ~37% of sim per seat rebuilding the riding bike's nearby world
(`refresh_ridden` -> `world_flown`) from every prop and tree each tick. `refresh_ridden` now takes its
world from `kept_world`, as `refresh_flown` does, and only puts the ignore list on the lent copy's ray
probe. Sim on this machine, 2P headless free06 at spawn, `OPENMX_THREADS=1 --timings --frames 600`:
1.64 -> 1.06 ms a frame. Details in 1-kept-world-FINAL.md.
