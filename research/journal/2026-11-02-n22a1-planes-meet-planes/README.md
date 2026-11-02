# N22a1 — planes meet planes

2026-11-02. Task: `./kit plan N22a1`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

In split and online two players' biplanes flew straight through each other: N22a ported only a bike
striking a flown plane. In the original both planes are records on the level's collision node, both
search with the other's hull in their world, and their resolves interleave substep by substep inside
one dynamics step. That is ported now: `flight::frames_together` steps every flown plane in one world
step, each plane's search striking the others' hulls (`flight_search::Scene::among_planes`, struck id
`PLANES`), each force listener resolving against the other planes' bodies as `Side::Body(Weighed)`.
The shell (split `main.rs`, online `net/rides.rs`) now reads every flying seat's pad, flies all the
planes together, then finishes each seat. With no tape of two planes meeting, the reference is the
original's own code run on s93's splitscreen image with one plane's velocity poked so it flies into the
other: the lists, all eight resolve calls a tick, and both bodies over the whole 80-tick touch are bit
for bit. Details in `1-planes-meet-planes-FINAL.md`.
