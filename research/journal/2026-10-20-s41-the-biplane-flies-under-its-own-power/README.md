# S41 — the biplane flies under its own power

2026-10-20. Task: `./kit plan S41`.

S41 was meant to wire the plane's whole frame together: the tires, the substeps, the body, the measure
and the crash test. Two of those pieces belong to S38 and S39, and neither has landed (both are still
in progress in other slots), so the wiring itself is blocked. Two parts did not depend on them, and both
are done:
- the recorded pad now flies the plane through I6's mapping, and the acceptance run watches its five
  lanes: three agree on every tick, and the two stick lanes part at 111 and 150, both explained;
- the engine's smooth noise is ported (`game_sim::noise`): the table builder and the sampler, each
  bit-exact on 10,000 oracle cases.

`BIPLANE_AGREES_THROUGH` stays at 0. See `1-…-PART.md` for what remains.
