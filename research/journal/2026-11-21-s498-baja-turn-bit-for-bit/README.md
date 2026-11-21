# S498 — baja turn bit for bit

2026-11-21. Task: `./kit plan S498`.

free07's parked baja's turned quat was a few bits off the original's put-back. The port's
quat-from-axes was already the original's; the test's put-back was a second one, and the original's
put-back squares its stored nose and up in place, so it squared them twice. The test now checks the image's
store is the car's copies squared once, puts those copies back so its put-back is the load's, and asserts
the quat bit for bit. Details in 1-baja-turn-bit-for-bit-FINAL.md.
