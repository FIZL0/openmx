# S251 — woken truck two way contact

2026-11-21. Task: `./kit plan S251`.

A woken `HasPhysics` truck now stands its path mesh down and its node update meets the ridden bikes:
its rays strike their frames, its step pushes them, and its mirrors go on their lists. The wake was run
in the interpreter on a fresh S207 ctruck recording and matches the port bit for bit; the node update
alone reproduces the original's frames. S207 ctruck agrees through tick 50 and parts on 51, when the
rear wheel lands on the truck's f.col. Details in the numbered files; the newest one's suffix says whose
move it is (FINAL · PART · READY).
