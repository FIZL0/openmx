# S489 — plane holds speed on truck

2026-11-21. Task: `./kit plan S489`.

R48's plane pushing free09's truck lost speed against the original's. Two causes, both fixed: the
truck's launch boost was not taken off the tape's state, and the truck's own rays striking the
plane's hull did not take its brakes off (the original's hit listener calls its enable). The push now
holds to the capture through row 1604 (was 1587). From row 1587 a wing ray that just reaches the
truck in the original misses it in the Sim, 0.0015 ft apart; that is left (file 1).
