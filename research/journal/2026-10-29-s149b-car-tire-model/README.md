# S149b — car tire model

2026-10-29. Task: `./kit plan S149b`.

The car's tire model — suspension (contact, bottom-out, spring and dampers, decompression limit),
rolling resistance, brake and ABS, spin and slip, and the surface table's force — ported into
game_sim::car beside the loader. The original steps a tire four times a 30 Hz tick, so a capture row
holds only the last substep: the proof is the original's own tire step under the interpreter on every
captured row of the baja, buggy and truck drives (≈95k words a car, bit-exact), plus the words a row fixes
on its own checked against the row, and ABS on forced windows. The original computes no aligning moment
(MomentScale is never read). Details in 1-car-tire-model-FINAL.md.
