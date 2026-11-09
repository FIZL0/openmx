# F31b5i — rider leaf on the ports

2026-11-08. Task: `./kit plan F31b5i`.

F31b5i asked for the frame tail's rider leaf to run on the rider ports instead of the original.
This session wired every rider port that exists into F31b5's every-port replay:

- the mode machine with its wreck arm (`rider::modes`);
- the bone solves (`bone::solve`);
- the part clears and the hand-over;
- the bodies' target;
- a new port of the leaf's wreck spots (`rider::wreck_spots`).

Every word the rider writes into the bike's record now comes from the port. C15 ride-2 and flip-1
stay bit-exact row by row, with RAM alike except for the ground query's ring cache. The leaf is
still about 95% interpreted, because the rider's ragdoll world step (S280) is not ported, nor are
the mode machine's callees (S279, S286–S289). The leaf's own frame body is filed as F55. The task
is blocked on those. Details: `1-rider-leaf-on-the-ports-PART.md`.
