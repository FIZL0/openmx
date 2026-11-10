# S280 — rider world step

2026-11-10. Task: `./kit plan S280`.

Port the rest of MX vs. ATV's rider ragdoll world step: the 21 pins, the 5 boxes, the two forces (the
rider's and the directed one), the 18 bodies' integrate and the passes, as game_sim::handling::ragdoll
over biplane::World. Four 12,000-case oracles against SLUS-21104 agree word for word, and with S278a's
bone collision the whole world now runs in the port with C15 ride-2 and flip-1 bit-exact. Details in
1-rider-world-step-FINAL.md.
