# MD259 — mxatrack tracks

2026-11-21. Task: `./kit plan MD259`.

MXA Track Studio's `.mxatrack` files, dropped in `mods/tracks/`, are now made into track mods
(`mxa_<id>`) by our own reader, and ride: their ground is the file's baked collision heights. Tested
only on packages we make from the format. A real track made in MXA is still needed for the final check.
The ground texture, props, route and stadium are not carried over. Details in the numbered files;
the newest one's suffix says whose move it is (FINAL · PART · READY).
