# MD107 — mxvatv car step

2026-11-09. Task: `./kit plan MD107`.

MD39a's five car drives replayed bit-exact through SLUS-21104's frame, seeded from each drive's RAM image
and fed the tape through the car's input step, ported as `atv_car::input` and proved by an oracle
(`tests/mxvatv_car_replay.rs`). Buggy and baja match on all 730 rows at each frame's end. Truck, sandrail
and golf have rows the recorder read mid-frame. Each such row's words are held at a kept moment about its
frame, except three sandrail rows read between a tire step's clear and its write. The first session (1)
was blocked on MD106 and C29. The second (2) did the work. Still to do: the port's own frame stepped
beside the interpreter (MD134). Details in the numbered files; the newest one's suffix says whose move it
is (FINAL · PART · READY).
