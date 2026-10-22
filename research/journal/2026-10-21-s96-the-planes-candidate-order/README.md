# S96 — the plane's candidate order

2026-10-21. Task: `./kit plan S96`.

S48 handed the plane's probes free06's props sorted by distance. The original hands out candidates in the
order its broad phase's walk meets them, and that order is set when the world is built: the static
quadtree's nodes, the chunks and slots inside each node, then the moving list. This task ports the build
(the tree laid over the terrain field, each collidable keyed and inserted in scene order), keeps the
plane's nearby search from frame to frame, and gathers it with S45's proven `broad_phase::gather`. The
proofs are against biplane.p2m2's save state:
- the port's tree holds all 353 scene meshes in the original's nodes, in the original's slot order;
- 5099 gathers match the original's bit for bit;
- the state's own 34 candidates come out of the port's world in order, once stand-ins are added for the
  moving objects the port doesn't have.

Two bugs turned up on the way, both older than this task, and both fixed:
- the scene reader rounded its floats to nearest, where the game truncates;
- props were placed with an orthogonalised basis.

Together they put 345 of the 353 props a bit off.
Details in 1 (FINAL).
