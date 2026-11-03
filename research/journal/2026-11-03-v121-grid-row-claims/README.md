# V121 — the grid rows a session with no AI rider claims

2026-11-03. Task: `./kit plan V121`.

S228a left 216 of P1's 1153 leaning ticks on fly_and_collide posing the head up to 0.77 ft off the
original's. The eased axes matched bit for bit on those ticks, and the clip time matched too. The
cause was which file three of the grid's clip names are read from. V14 measured that with AI riders
on the track, the AI body's `.mcf` claims those names first. fly_and_collide is two humans with no AI,
so the player's own `.mcf` claims them. The drawn body now takes its own claims (entry 1). The
parting spans on the stop and still rows are gone. Every tick still off is a fade out of the restart
snapshot, which this capture cannot check.
