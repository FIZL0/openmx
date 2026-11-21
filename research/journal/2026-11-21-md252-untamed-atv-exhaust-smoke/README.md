# MD252 — untamed atv exhaust smoke

2026-11-21. Task: `./kit plan MD252`.

Untamed's quad list hangs its exhaust smoke on `250_Exhaust_Particle01`, which the KTM 250 chassis lacks.
The node is on the build's exhaust part (`atv_r_250_exhaust.psg`). The shell now looks through the build's
part models for such a node and hangs the emitter on that part where it is drawn. The smoke puffs while the
quad pulls away under 15 ft/s; the shell test proves it. Nobody has picked the puffs out in a picture yet.
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
