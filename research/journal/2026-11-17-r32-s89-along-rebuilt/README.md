# R32 — s89 along rebuilt

2026-11-17. Task: `./kit plan R32`.

S89's s89-along RAM image was in no slot, so tests/ragdoll_props.rs, ragdoll_cars.rs and traffic.rs (and
physics_cars, physics_car_struck, bike_col's S168/S208, eco's third state) skipped everywhere. Rebuilt it
in copy 3 from biplane.p2m2's state with a tape and S76's teleport, saved at the old state's train clock;
all those tests run and pass on it. Installed in the shared context/states and kept in kept-context/r32.
Details in 1-s89-along-rebuilt-FINAL.md.
