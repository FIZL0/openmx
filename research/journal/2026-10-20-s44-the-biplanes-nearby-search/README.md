# S44 — the biplane's nearby search

2026-10-20. Task: `./kit plan S44`.

The brief (the world searches, end to end, wired into the ride) turned out to be five tasks, so it was
split. S44 kept the search loop: `collide_against_search` and its pair filter, ported as
`game_sim::world_search` (`search_nearby`, `accepts`), with the narrow phases, a placed group's parts,
and the listeners and callbacks behind a `Narrow` trait. The proof runs on the plane's own two probes
and ten of its gathered candidates in C7's save state: the filter over 40 000 pairs and the search over
10 000 cases, comparing every call out of it in order, the hit and every flag, all bit-exact. The
broad phase is now S45, the two narrow phases S46 and S47, and the wiring into the ride S48.
