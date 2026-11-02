# R15 — does a wheel standing on a prop push it

2026-11-02. Task: `./kit plan R15`.

S210 lets a wheel stand on a knockable prop and read the prop's motion. This task checked whether
the original also pushes the prop back. **It does not.** Every wheel force goes into the bike's
chassis. The only other thing that reaches the owner is the distributor telling the prop's `f.col`
listeners it was touched. Those are the struck wake the search had already run, so they change
nothing. Proved in the interpreter on knock-2897 (`a_wheel_on_a_prop_does_not_push_it`). The port
already behaves this way, so no product code changed. Details in the numbered files; the newest
one's suffix says whose move it is (FINAL · PART · READY).
