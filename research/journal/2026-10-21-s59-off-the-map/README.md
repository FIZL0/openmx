# S59 — off the map

2026-10-21. Task: `./kit plan S59`.

The original throws the bike back toward the middle of the map (the cannonball, wreck reason 13) when
the rider or the chassis comes within 150 ft of the terrain's edge. The port didn't. Now it does. The
clause is ported as `off_map.rs` and wired into the rider, and the motocross bike's put-back resets its
speed limit. It is proved by a function oracle over 12,000 bikes, by five live captures of the
original (four edges ridden off and one crossing in the air) matched bit for bit through the throw,
the draws, the clock and the put-back, and by a wiring test on the Rider. The biplane needed nothing:
its only "too high" rule is the engine fade that is already ported, and S36 matched its edge. Details
and everything left unproven are in 1-off-the-map.
