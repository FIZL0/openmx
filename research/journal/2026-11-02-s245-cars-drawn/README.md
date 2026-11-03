# S245 — the cars drawn in the app

2026-11-02. Task: `./kit plan S245`.

The app now draws the monster truck, the baja and the buggy. Each car is its livery's model posed by
its four tires (spin, steer, suspension), with its driver in the seat. Both renderers draw it, on a
track and in a world. Each car is parked only on its own track: free09 (truck), free07 (baja) and
free11 (buggy). It stands see-through in the bay `machine.txt` gives it, on the ground there, and a
switch takes it out of that bay. Getting the models to draw right needed a fix in the shared `.psg`
parser: a vertex's node number is a slot in its mesh's node list, which only the cars (and one
shovel) use. Details in the numbered files; the newest one's suffix says whose move it is
(FINAL · PART · READY).
