# MD124 — mxvatv car step

2026-11-11. Task: `./kit plan MD124`.

Ported the car's tire step (suspension, roll, slip, surface force, forces) into `atv_car::Pieces`, using
MX Unleashed's tire step on MD39b's field map. Under the frame oracle, the step is bit-exact unhooked
in tests/mxvatv_car_frame.rs: 3,000 frames on its own, and 3,000 more with all six pieces unhooked,
covering 20 tire branches. Tires that stand on a mover still go to `rest` (MD130). Details are in
1-mxvatv-car-step-FINAL.md.
