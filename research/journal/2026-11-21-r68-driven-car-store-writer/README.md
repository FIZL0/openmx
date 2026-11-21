# R68 — driven car store writer

2026-11-21. Task: `./kit plan R68`.

R68 asked what moves a driven car's put-back store. It is the car's safe-spot recorder (0x003a1a00):
on a car the human has switched into, each frame it finds flat enough ground under the car it stores
that spot, a flattened nose and the ground's up. A world reset over s342's image puts the baja's body
at that stored spot, ~192 ft from its bay, so S500 puts a driven car back at its moved store; porting
the recorder is S501. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
