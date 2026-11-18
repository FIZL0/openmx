# S328c — dust emitters on the sims generator

2026-11-17. Task: `./kit plan S328c`.

The bike's particle emitters draw twice on the game's one generator each frame they place particles
(C33: from tick 17, 4-26 draws a tick). The count now lives in the Sim (`game_sim::dust`, `Sim::dust`),
run per rider after its brain, on the AI lane's or the race's generator. Seeded from C33's image, the
Sim's emitters draw on exactly the original's ticks for every rider while its body agrees (ai0/ai1
through 28, ai2 and the human through the 63-tick window), and from 17 to 28 the Sim's generator
steps exactly those draws. The generator as a whole still parts at 17 because of the render's draws
(S328d). Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
