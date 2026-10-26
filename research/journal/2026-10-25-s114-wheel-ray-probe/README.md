# S114 — wheel ray probe

2026-10-25. Task: `./kit plan S114`.

The ride's port-made collision bundle (two wheel lines, a velocity probe and S89's belly_reach
stand-in) is replaced on the wheel side by the original's wheel ray probe over the bike's attachment
points, searched through the bike's nearby search; belly_reach is dropped. The built probe is the
save-state image's to the word, and the port's search finds the image's record. S89's rail rides
part where they did: the contact lanes are not the probes' doing but a ride that has drifted since
tick 2 (S124). The body is still stopped by the port's one velocity probe (S123). The along locked
lanes are re-pinned to HEAD's 278/276 (S120's red). Details in 1-wheel-ray-probe.
