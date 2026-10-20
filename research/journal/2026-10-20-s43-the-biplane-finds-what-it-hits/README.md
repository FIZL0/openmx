# S43 — the biplane finds what it hits

2026-10-20. Task: `./kit plan S43`.

Ported the collision object's contact search (clear, add, the sweep back, the heightfield rays, the world
rays) as `game_sim::contact_search`. It was proved bit-exact on the plane's own collision object over
10 000 cases, and the terrain point query over another 10 000. Water isn't in the search: the ground
result's depth is a constant 0. That leaves only the world searches, which stay behind a trait and are
filed as S44 with the wiring into the ride.
