# MD39 — mxvatv machines

2026-11-03. Task: `./kit plan MD39`.

MX vs. ATV's five cars (buggy, baja, sandrail, golf cart, monster truck), mapped with the PC exe and
SLUS-21104 but not proved against a drive: there is no SLUS-21104 car capture, no RAM image with a car in
it, and SLUS-21104's frame driver (MD75) is not ported. The cars are MXVehicleCar with MX Unleashed's step
and loader, three loader changes (MXVehicle's [Wrecks] keys first, CofGAI's default from the body,
a [Camera] tail), a per-block field shift, and retuned files. Written up in research/mxvatv-pc.md "The cars
(MD39)" and research/oracle.txt `# MD39`; the five files read through `car::Car::read`
(tests/mxvatv_car_phy.rs). The rest is split: MD39a captures, MD39b the step bit-exact, MD39c in the game.
