# S202 — sidepost head ray tick

2026-11-02. Task: `./kit plan S202`.

S139 left sidepost (free09's shed corner post, the bike thrown sideways at the 135 ft/s cap) wrecking
the port with the head ray (reason 6) one tick before the original. It was already fixed when this task
ran. S152 merged the same day as S139, after it, and moved the head ray's scenery test from the
frame-end sweep onto the body rays' contacts. Those rays look from where the tick's sweep back leaves the
bike. Since then the port wrecks on the original's own row, 2481, frozen at the original's exact
position. No sim code changed. S139's test now requires the exact tick. Details in
`1-sidepost-head-ray-tick-FINAL.md`.
