# MD28 — world frame time

2026-10-25. Task: `./kit plan MD28`.

Riding a big world was laggy. File 1 measured it with `--timings` against one disc map and made nine
fixes: tile reads off the main thread, the ground handed over a few chunks a frame, the index lists
kept per piece, far cars posed less often, and more. After that no frame went over 16.7 ms riding
across a join, on a quiet machine. That work merged on 2026-10-25 but was not ticked while main's
gate was red (S119). File 2 (2026-10-27) found that later work had made world_mxu a slideshow again.
Since S83, S114 and S138 the bike searches props each tick, and a world handed it every tile's
shapes. A world of tiles now hands the bike's searches only the shapes near it: `sim` went from
73.7 to 1.1 ms a frame. Traffic cars' poses are left for MD45. Details in the numbered files; the
newest one's suffix says whose move it is (FINAL · PART · READY).
