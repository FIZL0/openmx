# P251 — world mxu closer

2026-11-02. Task: `./kit plan P251`.

The user, twice: the disc-built `world_mxu` has wide flat no-man's-land between maps (Manassas Park;
Hillsboro to Bodie). Three causes: each national kept ~5450 ft of its own ground, but its course sits in
~1000 ft of the middle (≈2200 ft of empty flat each side); the 6 × 5 grid left three cells empty (6.8k
and 20k ft bands); and gaps were per line, set by the line's worst pair. Now `world_mxu.json` asks for
`"trim": 650` (each map cut 650 ft past its placements) and `"layout": "units"` (a packed grid of
units, gaps per facing pair, every wrap seam tried). Its join ground is now wrap-aware (the maps
round the wrap are blended too), so the closer world has no new seam steps. Result: 37720 × 31456 ft →
21853 × 19828 ft, widest join inside 1719 → 1364 ft, steepest seam step 0.959 → 0.614 ft. The user's
installed `mods/tracks/world_mxu` (B2) still shadows it. Done; details in `1-world-mxu-closer-FINAL.md`.
