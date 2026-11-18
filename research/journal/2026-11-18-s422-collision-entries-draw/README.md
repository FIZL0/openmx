# S422 — collision entries draw

2026-11-18. Task: `./kit plan S422`.

The wreck's collision models and the rider's bone collision entries (foot dust among them) were Quiet
in game_sim::dust. They now emit from their own updates on the shared generator, once per contact, from
the wreck bike's record and the ragdoll's contacts, through the emitter's view cull. Proved per update
against the original's whole frames over S325's nose-dive wreck: 372 updates, 228 draws and 990 view tests,
all bit-exact. The Sim end to end (S476) and the emitter pass's cull (S477) are follow-ups.
