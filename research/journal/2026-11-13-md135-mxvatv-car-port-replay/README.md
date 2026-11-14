# MD135 — mxvatv car port replay

2026-11-13. Task: `./kit plan MD135`.

MD107's five car drives (buggy, baja, sandrail, golf cart, monster truck) replay bit-exact by the port's
world-step frame. `tests/mxvatv_car_frame/drives.rs` steps `game_sim::atv_car::frame` beside the
interpreter's world step every frame, 730 frames per drive, about 1.54 M words compared per drive, all
equal. The car's update isn't stepped beside yet, because the class's step is still a call (MD156). See
1-…-FINAL.md.
