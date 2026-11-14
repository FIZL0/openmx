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

2026-11-14 (part 2, after MD150, MD149 and MD135 landed): the five cars are in the shell, off by default.
openmx-mods' `import-atv cars` builds the `mxvatv_cars` machine pack (branch `openmx-MD39c`). The shell
reads it with `--mxvatv-car NAME` (settings.txt `mxvatv_car`), and the pause's switch row offers the car,
ridden as `atv_car::Ride` on seat 0's pad. `crates/openmx/tests/mxvatv_cars.rs` checks the off case (no
car, ride unchanged) and holds each car to the in-process Ride bit for bit over 250 ticks on free09. No
drawing, Garage row or MD39b replay through the shell yet (MD157, MD158, MD159). See
`2-mxvatv-cars-in-game-PART.md`.
