# C7 — c7 biplane capture

2026-10-20. Task: `./kit plan C7`.

biplane.p2m2 (Free06) starts with the human already in the plane, standing on the ground. There is no
switch from the 250. I played it back on the original and captured it to
`context/capture/acceptance/biplane.csv` with a spec of its own, `watch/biplane.toml`:
- the plane's transform, velocity and rates
- the eight controls as the input step writes them
- the engine's rpm and the drawn prop
- the crash state

Every row's input matches the tape (field 2k−1, 1,717/1,717), and a second run repeats the flight except for
one torn row per run. The index (two take-offs, climbs, dives, banks to knife-edge, two crashes: one ended
by the L1 + R1 reset, one by WreckTime) is in 1-c7-biplane-capture-FINAL.md. The addresses are in
research/oracle.txt's C7 block.
