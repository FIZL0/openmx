# P216a — kiowa shell ground

2026-11-02. Task: `./kit plan P216a`.

The shell's Kiowa now stands, lands and wrecks on the original's ground: C14a's collision object
(`kiowa::Collision`, `kiowam.col`'s rays) searched before each tick, and `kiowa::Grounded` as its
surroundings (`contact::resolve` on the record each substep, the record's touching and entries, the
rotor's wreck rays in the ground). The port's spring stand-in (KIOWA_SPRING and its four friends) is
gone. The crash test's MinWreckSpeed now fires in the shell. The wreck itself is still the port's
(the blades hidden, a fresh helicopter built on the put-back) until S220a1 lands: filed as P216a1,
blocked after S220a1.
