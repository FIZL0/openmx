# F15a — a cheaper exact path

2026-11-02. Task: `./kit plan F15a`.

F15's alternative to the native sim: keep the PS2's arithmetic and make it cheaper. ps2-float's `Vf` add, sub,
mul, madd and msub (and `mul_q`, `add_q`, `opmula`, `opmsub`) now run their four lanes as branch-free vector
code. That is the same f64 two-sum and chop, with the step back done as a lane mask. The results are bit for bit
what they were: the ride hash, every game-sim test and the oracle are unchanged. On this desktop the exact sim
drops to about 0.75× its old time. Native is still about 0.47×. No Deck number: the Deck was unreachable again.
Details in `1-simd-exact-FINAL.md`.
