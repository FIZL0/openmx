# F31b5i — ragdoll world in the leaf

2026-11-11. Task: `./kit plan F31b5i`.

F31b5i was reopened once S280, S279, S286–S289 and F55 had landed. This session wired the rider
ports from those tasks into the frame tail's rider leaf. That covers:

- the ragdoll world (S280), with its bone collisions on the port's scene (S279);
- the pose and settle (S286);
- the copy's node position;
- the rider's vt `+0x148`, newly ported as `rider::after_step`. It is the hand-over plus the
  wreck's spot node into the record: the bike's `+0x870..+0x8a0`, the words the task names.

C15 ride-2 and flip-1 stay bit-exact, with RAM alike as before. The rider's tail fell from
36.6M/44.3M interpreted instructions to 9.93M/9.53M, and all of what is left is the world step's
wrapper and its callees. No task covered those, so they are filed as F62, and F31b5i is blocked on
it. Details: `1-ragdoll-world-in-the-leaf-PART.md`. The first session's part is in
`../2026-11-08-f31b5i-rider-leaf-on-the-ports/`.
