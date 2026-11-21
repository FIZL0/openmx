# V137 — first person roll on screen

2026-11-21. Task: `./kit plan V137`.

The first-person camera (P384) worked out a rolled up but the renderer always drew with the world's up.
The render Camera now has an optional `up` (`None` is the world's, as before), the shell hands it the
first-person camera's up, and the projections, the sky and the sun flare draw with it. Leaning the bike now
tilts the horizon on screen. A test proves the tilt is ObjectRollScale × the lean, and shots of free06
checked it by eye. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
