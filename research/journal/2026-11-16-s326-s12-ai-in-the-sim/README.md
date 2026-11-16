# S326 — s12 ai in the sim

2026-11-16. Task: `./kit plan S326`.

The AI lane's scene now walks the riders' bikes after the static world, as the original's kind-3 walk
hands out its moving structure. The task assumed the original sweeps those bikes. It does not: every
bike collidable carries the `AI_IGNORE` class bit (0x8000, the word the bike builder ORs in). So an AI
rider's feelers never stop on another rider's bike. `ai::WorldScene` walks the Sim's bikes
(`ai::Ridden`) flagged `AI_IGNORE`, and `Sim::think` hands them in. The proof uses recording2's image,
with another rider's bike moved across the first AI rider's way at 90 spots:
- the walk, the absence of bike sweeps, and the tick after all matched the original;
- the bundle swept at each bike through the original's per-object sweep and the port's matched too:
  360 sweeps, 153 met, every feeler alike.
Details in file 1.
