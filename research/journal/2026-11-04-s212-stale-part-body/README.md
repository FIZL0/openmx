# S212 — stale part body

2026-11-04. Task: `./kit plan S212`.

A toy's hierarchy collision part, on a tick the toy's visibility gate passes over, gave zero motion in
the port; the original's kinematic body is not updated then and keeps answering its last step's motion
(the node's world now against the matrix from the update before last). `Toys::frame` now rebuilds that
pair from the clock's `prev`. Measured in the interpreter on rebuilt S133 free10/nat11 states with the
toys drawn five ticks in seven: every part's pose, velocity and spin bit for bit. Details in
1-stale-part-body-FINAL.md.
