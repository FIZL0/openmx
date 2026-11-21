# V136 — race bodies take the ai grid rows

2026-11-21. Task: `./kit plan V136`.

A race with AI riders out now loads every body (the AI riders' and the human's) on `airider.mcf`'s
grid clip names, as the original's one animation set has them: `turn_stop_Fwd`/`Bck` play
`turn_stop_Mid`, `turn_still_Bck` its own clip. Proved on the original's race-start RAM images (four
with AI riders out, AllAI on and off; one race and nine free rides without): which grid `.mot` files
each loaded. Details in the numbered file.
