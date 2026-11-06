# P316 — mxvatv pilot seat

2026-11-05. Task: `./kit plan P316`.

MX vs. ATV's fliers were drawn with MX Unleashed's pilot seat, so the Spitfire's pilot (skins 3-4)
stuck out through the bottom of the plane. SLUS-21104's pilot request bases the body on the model's
matrix rather than the centre of mass, so the root sits at the skin's model's `PelvisBoxPosition`
itself. That was read in the request's disassembly and seen on all four MD80a flier images. openmx now
seats MX vs. ATV's plane and helicopter there: Spitfire (0, 4.4, 0.7), model-one plane (0, 3.2, −3.0),
Kiowa model one (1, 3, 5), Little Bird (−1, 5.1, 1.5). MX Unleashed's own seats are unchanged. Details
in 1-mxvatv-pilot-seat-FINAL.md.
