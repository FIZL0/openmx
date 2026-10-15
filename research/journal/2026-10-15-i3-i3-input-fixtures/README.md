# I3 — i3 input fixtures

2026-10-15. Task: `./kit plan I3`.

Input recording and playback in the port: a **pad tape** is a recorded session as the pad reports and
measured `dt` of every frame, and `Playback` drives the port with one — the pad read before the
simulation, through I1's stateful input layer, so a recorded sequence reproduces the original's controls,
strengths, timers and events rather than a button mask. Proved by replaying three recordings of the real
game (817 frames) against the sixteen controls the game itself recorded, row for row, with the half of
libpad's buffer, the row-0 drop and the tick density all pinned down by mutation. Along the way it fixed
a committed desync between two definitions of `Input::buttons` (S8's control-index mask vs the game's
wire bitmask) that would have pressed the wrong buttons on the first acceptance replay.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
