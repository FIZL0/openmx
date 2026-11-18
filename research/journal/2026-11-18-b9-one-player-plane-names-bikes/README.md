# B9 — one player plane names bikes

2026-11-18. Task: `./kit plan B9`.

In the one-player plane ride, the plane's rays struck the AI riders' bikes as static boxes. The
biplane state shows each AI bike is its collision record's, as split's are (N22b). So `Sim::step`
now builds a `Met` naming them. The plane's entries on a bike name it, its resolve pushes that bike,
and the pushed chassis is written back. The plane's search was proved bit for bit against the
original's collide on the state, with a bike moved into the plane at 8 poses. The bikes' own
resolve of the mirrors waits for B9a: the port does not step other riders while player 1 flies.
