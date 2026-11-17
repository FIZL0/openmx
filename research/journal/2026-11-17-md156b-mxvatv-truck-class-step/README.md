# MD156b — mxvatv truck class step

2026-11-17. Task: `./kit plan MD156b`.

The monster truck's per-frame class step (vt +0x488) is ported as `atv_car::truck_class_step`. A 10,000-case
oracle on the truck image proves it bit-exact, and its part words equal the original's on every one of the truck
drive's 730 updates. Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART ·
READY).
