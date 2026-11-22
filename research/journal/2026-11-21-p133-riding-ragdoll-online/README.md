# P133 — riding ragdoll online

2026-11-21. Task: `./kit plan P133`.

Online seats drew the rider in the kit's clip pose, while offline seats (since S90) draw it from the riding
ragdoll. Now each shell steps a drawn riding ragdoll per seat beside the online ride, for the picture only:
held to the grips of the bike it drew and pulled to the body it posed, stepped once each session tick on
the seat as shown. The ride itself is untouched (no desync). A headless online test shows both seats on
both machines drawn from the riding nodes, and an idle online seat within 0.03 ft of an offline one.
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
