# F31b5n — effects leaf

2026-11-09. Task: `./kit plan F31b5n`.

The part walk's effects leaf and its two particle emitter kinds, ported far enough that their draws on
`rand` (the way they reach the bike) are Rust's. Session 1 ported the effects listener's own work and split
the rest into F56–F59. Session 2 (after those landed) moved the emitters' `rand` draws onto
`game_sim::rng`. Session 3 found that F57a had ported the last draw (the first emitter kind's other branch), added
the same check to ride-2, and closed the task. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
