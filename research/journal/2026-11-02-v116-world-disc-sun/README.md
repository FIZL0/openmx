# V116 — world disc sun

2026-11-02. Task: `./kit plan V116`.

Under World Sky Global (the default) a world lit every map by its sky tile's sun, so free09 inside
world_mxu_free was a fifth darker than alone and the bike's shadow fell another way. Global now keeps
the sky tile's sky and fog for the whole world, but each map's own sun and ambient (mixed across a
join, as Per Map's), because each map's ground is baked by its own sun. free09 in the world is now lit
0.983 of alone (was 0.826), the shadow falls as alone, and free06 is unchanged.
