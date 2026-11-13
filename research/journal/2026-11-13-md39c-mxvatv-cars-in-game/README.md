# MD39c — mxvatv cars in game

2026-11-13. Task: `./kit plan MD39c`.

MD39c puts MX vs. ATV's five cars in the game as an addition, each built from its own `.phy` and driven by
SLUS-21104's step. The session found that two things this needs do not exist yet, the same two the quad
lacked before MD37d. First, nothing builds an `atv_car` car from its files: every car so far is seeded
from a SLUS-21104 RAM image. Second, the car's step does not run without the interpreter: the update's
hand-outs (MD106e, MD128) and the port-side replay (MD135) are still open. The two gaps are filed as
MD148 (the car built from its files) and MD149 (the car stepped on MX Unleashed's worlds, giving a Ride
type a shell can step). MD39c is blocked until they and MD135 land. No code was changed. See
`1-mxvatv-cars-in-game-PART.md`.
