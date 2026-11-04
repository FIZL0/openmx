# F31c — the ground under C15's rides

2026-11-03. Task: `./kit plan F31c`.

State 4 is a Free Ride on Nat05. The bike's ground there is the map's terrain alone: along C15's
ride-2 and flip-1 no ray of the bike's files a contact with anything. `game_sim::handling::ground`
builds the terrain object from `Env\nat05\Nat05.ps2.trn` (word for word the image's). The wheels ask it
through MD76's `atv_ground::query`. The collision record asks it through a new port, `under`, of the
record's own terrain query. Every one of these queries along both rides answers bit for bit as the
original's in the interpreter, ride-2's first frame (8 wheel queries) included. Details in
1-mxvatv-bike-ground-FINAL.md.
