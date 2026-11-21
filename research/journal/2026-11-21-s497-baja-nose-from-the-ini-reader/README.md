# S497 — baja nose from the ini reader

2026-11-21. Task: `./kit plan S497`.

`car_spawn.rs` now reads free07's baja nose through the port's ini reader, and the parked-baja test
checks it bit for bit against the original. The turned quat tightened from 1e-7 to 2 ulps; its last
bits (x one, z two off) are filed as S498. Details in the numbered files; the newest one's suffix says
whose move it is (FINAL · PART · READY).
