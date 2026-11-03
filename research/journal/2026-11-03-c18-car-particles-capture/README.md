# C18 — a driven car's particles against the original

2026-11-03. Task: `./kit plan C18`.

I captured the original's car particle manager every frame on three S149a free07 drives: the
monster truck on the gas, the same truck thrown by a shortened map's edge, and the baja on the gas.
The loader's wreck word, +0x754, is never set (only zeroed in the ctors, 0 through the throw), so the
original's car manager never sees a wreck. The port now feeds none.

Run against the captures, the port's manager matches every entry's flag and switch on every compared
row. One more difference turned up: a zone-off tire dust never emits in the original, though P60's
reading said it does. That is fixed too. What's left is 0, 9 and 35 single-tick timing misses at
transitions.

Test: `tests/car_particles.rs the_cars_emitters_follow_the_original`. Scripts: `capture/`.
