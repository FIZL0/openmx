# K83 — freestyle event link failure

2026-11-20. Task: `./kit plan K83`.

`freestyle_event` was filed as flaky after P383's merge gate, but it never ran there: its test program
failed to link, like eight others in the same build. The test itself races nothing. The person picked
option A: close K83 into K82's gate fix (log linker stderr, relink once, its own headline). No build task.
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
