# S261 — part drawn matrix writer

2026-11-16. Task: `./kit plan S261`.

The writer of each ragdoll part record's drawn matrix (`+0x60`, translation `+0x90`) is a loop Ghidra
never made a function. It sits in vtable slot `+0xb0` and is the rider physics update's first call:
each part's node world rows are copied in, every tick, after the body's mode (and so every entry's
pose) has run. So an entry reads the node as the tick before the last left it. On a re-recorded
recording2, the records equal the row before's nodes on every clean tick. All five trick exits'
start positions and velocities are the port's pose bit for bit. Details in
`1-part-drawn-matrix-writer-FINAL.md`.
