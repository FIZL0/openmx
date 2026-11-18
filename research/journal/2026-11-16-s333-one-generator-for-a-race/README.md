# S333 — one generator for a race

2026-11-16. Task: `./kit plan S333`.

A Sim with a race now has one generator, the race's. The AI riders' brains draw on it after the race
manager's tick, and `spawn_ai` leaves the race on the generator its spawns left. Held against S13d's
sx01 capture: the over tick's tie draw is the frame's first `rand()`. A Sim test shows the race's draws
come first and the brains' follow on the same sequence, tick by tick.
