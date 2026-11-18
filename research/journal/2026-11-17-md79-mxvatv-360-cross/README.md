# MD79 — mxvatv 360 cross

2026-11-17. Task: `./kit plan MD79`.

MD78 asked for the 360's cross in/out to be read from the stunt record the stunt start uses. That record
is the 360's score, not a fade: 2000 points plus 15 000 a second (R17 proved the call is a score-node push,
and state04's rows 44/45 read 2000/15000). There was nothing to take, so the 0.25 s cross stays. Only the
code comment changed. Finding the real blend is filed as R43. Details in 1-mxvatv-360-cross-FINAL.md.
