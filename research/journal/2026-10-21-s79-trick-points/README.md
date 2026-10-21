# S79 — trick points

2026-10-21. Task: `./kit plan S79`.

The original's score card is now ported into game-sim as `Rider::score`, carried by replays and
rollback and covered by the state hash. It covers:

- tricks at 2500 a second, multiplied by the combo's node count;
- the 10 000-point backflip;
- wheelies and stoppies after a second on one wheel;
- the bank on a clean landing;
- a wreck losing the whole combo.

It is bit-exact against the original's functions over 10 000 random operations, and against
recording2's playback row by row. That includes a trick landed into a wreck at frame 3562 that
scores nothing. The freestyle time limit and target are not done; they are filed as a follow-up.
Details in `1-trick-points-FINAL.md`.
