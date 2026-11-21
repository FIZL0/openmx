# K84 — overlay link failure

2026-11-20. Task: `./kit plan K84`.

K84 was filed to make the test `overlay` robust after it "failed under load" in P383's merge gate. It never
ran there: its test program failed to link, with eight others in the same build, and the gate filed that as
flaky. The test itself is pure (no clock, sleep, ports, threads or shared files). The person picked option A
on 2026-11-21: fold K84 into K82's gate fix (log linker stderr, relink once, own headline). No change to the
test; K82 carries a note. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
