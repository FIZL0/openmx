# MD93g — untamed bike step

2026-11-03. Task: `./kit plan MD93g`.

This task traced the rest of what Untamed's (SLUS-21701) 360 reads from the pad, and which stunt it
starts. MD93f had already covered the arming button, the holds and the backflip's binding. This adds:

- the 360's "bound" is the backflip's byte, so the 360 has no bound control either (`button: None`);
- "bound left/right" match the pad record against threesixtyL/R's classic combos, which are empty;
- the 360 starts stunt 52 (threesixtyL) or 53 (threesixtyR) by direction, and `ride_spin` now hands that
  id in while the clip runs, in place of the backflip's 49.

New: `untamed::spin_stunt` and the stunt constants, and the test `untamed_ride_spin_input_is_the_shipped_bindings`.
Details in [1-untamed-bike-step-FINAL.md](1-untamed-bike-step-FINAL.md).
