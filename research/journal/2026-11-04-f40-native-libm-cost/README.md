# F40 — native libm cost

2026-11-04. Task: `./kit plan F40`.

Measured what ps2-float's libm costs in a native-sim ride before writing a faster one. On free06, 3000
ticks, it is about 0.6 µs a tick in 1P and 1.2 µs in 2P (48 fmodf, 8 sinf, 6 powf a tick at 8–68 ns each),
against 1.4–1.8 ms of sim: under 0.1%. Not worth a second libm, so no code change. The acos table's
~14 ms at load is a one-off. Details in 1-native-libm-cost-PART.md.
