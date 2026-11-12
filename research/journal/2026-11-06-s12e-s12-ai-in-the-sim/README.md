# S12e — s12 ai in the sim

2026-11-06. Task: `./kit plan S12e`.

This task puts the opponent AI riders into the Sim, stepped each tick in the original's order and proved
against a free09 4-racer replay. The rider-to-brain copy (`ai::feed`) is ported and proved with a function oracle
(2). After S312, S313 and S306 landed, `ai::Racer::tick` joins the ported pieces into the AI's whole tick. It is
proved against the original's `ai_update` on free09's three riders, 60 ticks each, interleaved (4). Every riding
brain avoids with the generic vehicle avoidance, which is not ported (S317); the proof stands in the original's
result for it. The Sim lane and the 4-racer replay wait on S317. Details are in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
