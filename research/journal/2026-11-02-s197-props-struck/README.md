# S197 — props struck

2026-11-02. Task: `./kit plan S197`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

Until now nothing in a ride woke a prop: the bike passed through every one. Each prop's `<model>f.col`
is now posed at its body's node and searched by the bike's body rays in `Sim::step`; a hit files the
bike's entry and the prop's mirror as the original's contact search does, the struck listener wakes the
collection (object's listener first, then the collection's settle), the bike's resolve pushes the prop's
body and takes the push back, and the props' world step resolves the mirror. Proved bit for bit against
S195's knock states 2882 → 2883: the bike's entry and the mirror, both bodies after the push (against the
original's resolve, hooked inside the props node's update), and the ball after its tick. Only the bike in
single-player `Sim::step` strikes; the rest is filed as follow-ups. Done; details in
`1-props-struck-FINAL.md`.
