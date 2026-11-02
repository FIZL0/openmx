# S208 — a woken truck's mesh in the wheel search

2026-11-02. Task: `./kit plan S208`.

S208 asked to put a HasPhysics truck's physics mesh in the ride's wheel search, with its owner, once a
woken truck is simulated. Iteration 1 proved the original's wake sets the mesh's nearby bit, so a wheel
can stand on a woken truck and read the owner branch, and blocked on S186. Iteration 2 (FINAL) wires it:
`game_sim::truck_wheel` searches each physical truck's f.col on the wheel probe's record, names the
truck's body as the wheel's owner and reads its velocity, with the path mesh stood down meanwhile; proved
by `tests/truck_wheel.rs` and mutants. Details in the numbered files; the newest one's suffix says whose
move it is (FINAL · PART · READY).
