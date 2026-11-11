# S12e — s12 ai in the sim

2026-11-06. Task: `./kit plan S12e`.

This task puts the opponent AI riders into the Sim, stepped each tick in the original's order and proved
against a free09 4-racer replay. S281–S283 have landed. It is now blocked on the AI tick's callouts (S312), the seeker's world read
from the disc (S313) and the pick list (S306) (3). The rider-to-brain copy (`ai::feed`) is ported and proved with a
function oracle (2). Details are in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
