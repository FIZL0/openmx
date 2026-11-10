# MD37b — atv step

2026-10-29. Task: `./kit plan MD37b`.

The ATV's step bit-exact against SLUS-21104. Part 1 ported the quad's own controllers (stand-upright,
wheelie/stoppie, ground push, rear-pair flag) and filed the frame as MD64/MD65; part 2, after those
landed, ported the ATV wheel's suspension and tire forces (atv_wheel.rs) and filed MD74 (wheel callees),
MD75 (the physics frame) and MD76 (Desert Oasis's ground). The captured-ride replay waits on them.
Part 3, after those landed, chained MD75e's whole frame over two 240/300-frame rides bit-exact against
the original's world step in the interpreter; MD37a's csvs are lost (C28), and the strike and wreck are
outside the world step (MD104).
Part 4, after C28 and MD105 landed, replays all 15 of C28's captured rides bit-exact: the original's
whole sim frame and the port's beside it, with the body, engine and wheels equal to every row. Three
rides stop at a strike where the rows tear (C32), and the quad's other words are listed, not asserted
(MD120).
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
