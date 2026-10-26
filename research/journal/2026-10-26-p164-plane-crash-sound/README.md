# P164 — plane crash sound

2026-10-26. Task: `./kit plan P164`.

The biplane's crash made no sound. The original plays `machine.aud`'s `[biplaneWreck]` through the bike's
wreck-clank emitter: five mono clips, picked without repeats. A clank starts on any wrecked tick whose
collision record touched with a shock peak of at least 5. Its level is the peak's share of 20, as a whole
percent. It is heard from 30 ft to 150 ft, and it holds off while its last clank still plays. The port now
plays it per seat, so in a split and online. A replay of the original's captured crashes starts the clanks on
the original's ticks at its levels, and a plain landing plays none. Details in the numbered files; the newest
one's suffix says whose move it is (FINAL · PART · READY).
