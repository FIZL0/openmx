# P382 — lent bodies on the sim thread

2026-11-21. Task: `./kit plan P382`.

Split-screen rides with the riding ragdoll were kept off the sim thread (B43), because the body lent to
the rider's step went with the tick and the picture drawn meanwhile needed it. The body can be copied
now (its clips and archives shared), so the tick takes the body and the picture draws a copy. Proved the
same ride tick for tick, thread on and off, for two seats (gate test) and a race with AI riders (by
hand). A seat flying or driving another machine still keeps the tick on the loop. Details in
1-…-FINAL.md.
