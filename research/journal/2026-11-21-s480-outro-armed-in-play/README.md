# S480 — outro armed in play

2026-11-21. Task: `./kit plan S480`.

S334 made the tip's popup stop the race, but in a ride the Sim's race never got an outro scene, so its tip
never fired and the stop never came. Now the race's Over runs the activity's race end inside the Sim's race
tick: the best player's place picks the outro scene, the race takes that scene's length (read off the disc:
the event's `[RaceEndDitties]` sounds and the app's stream list) as its stage, and the tip arms 2.05 s before
the scene's end — sx01's third place gives 6.46, as S13d's capture has it. A scene with nothing to play arms
the tip 7 s on. The shell leaves the ride when the popup is up rather than on its own 7 s clock. Details in
`1-outro-armed-in-play-FINAL.md`.
