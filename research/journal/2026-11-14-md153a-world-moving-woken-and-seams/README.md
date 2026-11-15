# MD153a — world moving woken and seams

2026-11-14. Task: `./kit plan MD153a`.

The rest of MD153. A world's woken HasPhysics trucks are now drawn where the sim's body has them, not on
their clips (proved by a headless ride into free06's truck on world_mxu_free; the mutant draws it 193 ft
off). On a wrapped world the moving list's shapes get up to three seam copies in every set the searches
are handed (wrap_moving.rs). Knocked props across a seam are left to MD166.
