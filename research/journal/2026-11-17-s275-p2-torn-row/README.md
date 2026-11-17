# S275 — p2 torn row

2026-11-17. Task: `./kit plan S275`.

C16's parked P2 parted at 137827 when flown from s272-137810.ram. The port was already bit-exact:
row 137829 was read inside integrate (old velocity, rebuilt centre). `p2_held` now takes that reading,
and P2 holds 90/90. Details in 1-p2-torn-row-FINAL.md.
