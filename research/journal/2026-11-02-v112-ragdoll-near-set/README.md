# V112 — ragdoll near set

2026-11-02. Task: `./kit plan V112`.

A wrecked rider's bone search on a world of tiles now takes the near set (`World::wooded_near`), as the
wreck body has since V99. The rider is proved the same as with the whole world's set on a generated
tiled world. The 12–29 ms single-frame sim peaks V99 saw around each world_ultra wreck were this
search's regathers, keying all 19,041 shapes. After the change a regather keys 33 and the peaks are
gone. Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
