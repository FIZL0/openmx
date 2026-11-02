# S201 — the riding entry's first ticks

2026-11-02. Task: `./kit plan S201`.

S135 left the riding entry after a put-back unmatched: at T+2 the original's bones sat about 0.15 ft
off the port's. A new capture of S74's ride script on free09 records each part body's start position
and velocity, and it shows where the entry's pose starts the bones. It poses them at the nodes the last
held tick left, not at this tick's animation. Their velocity is measured from drawn matrices that the
held tick's still repose left at each body's centre of mass, so every bone enters moving by its centre
of mass turned back over the frame. `Riding` now does the same. Its starts match the original's bit for
bit, and the worst entry tick over S74's nine put-backs drops from 0.246 ft to 0.044 ft (mean 0.105 →
0.004). `tests/riding_spring.rs`'s entry bounds are tightened to 0.06 ft / 0.1 / mean 0.01.
Details in `1-riding-entry-FINAL.md`.
