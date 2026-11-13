# F31b5i — rider leaf census closed

2026-11-13. Task: `./kit plan F31b5i`.

F31b5i was reopened again after F62a and F62b landed. Those, with F62, ported the last of what the
rider leaf left to the interpreter: the world step's wrapper, the idle layer, the clip advance and the
clip start. This session checked that the task's acceptance now holds. It does: C15 ride-2 and flip-1
replay bit-exact with the rider leaf on the ports, RAM alike. The rider part of the tail interprets 0
instructions on ride-2. On flip-1 it interprets 1,823, and all of those are rand, the cue and sound.
`leaf_census` now asserts that last part outright. Details: `1-rider-leaf-census-closed-FINAL.md`.
The earlier parts are in `../2026-11-08-f31b5i-rider-leaf-on-the-ports/` and
`../2026-11-11-f31b5i-ragdoll-world-in-the-leaf/`.
