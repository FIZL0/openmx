# S386 — race clock and odometer

2026-11-21. Task: `./kit plan S386`.

The race's score manager clock and the statistics tracker's odometer are now ported into game_sim
(`Race::clock`, `Tracker::odometer`), and the results' stats commit reads them in place of the
shell's own drop-to-leave clock and the race card's odometer. The clock holds bit for bit on S13d's
live finish. The odometer's writer (top of the player bike's update) was found and holds bit for bit
on a new live four-bike capture, front wheel touching or not; bikes 0 and 1's rows there are torn by
the recorder, and wrecks were never captured. Details and what is unproven in 1.
