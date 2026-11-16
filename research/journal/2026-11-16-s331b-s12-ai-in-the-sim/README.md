# S331b — s12 ai in the sim

2026-11-16. Task: `./kit plan S331b`.

The Sim's AI riders now read their race card, race kind and live flag off `Sim::race`'s laps before
every think, and their race driver drives by the laps, the race track and the track's AI net
(`<track>AI.tdf`, now loaded by the shell). On sx01 with `--racers 4` the three AI riders leave the
grid at the drop and race: they take the lead from the idle human and swap places. Landed.
