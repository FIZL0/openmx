# F31b5n — effects leaf

2026-11-09. Task: `./kit plan F31b5n`.

The part walk's effects leaf and its two particle emitter kinds, ported far enough that their draws on
`rand` (the way they reach the bike) are Rust's. Session 1 ported the effects listener's own work and split
the rest into F56–F59. Session 2 (after those landed) moved the emitters' `rand` draws onto
`game_sim::rng`. The one draw left in the interpreter is in the first emitter kind's other branch (F57a),
and the task waits on it. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
