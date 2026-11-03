# F7 — the ragdoll's bones against the knockable props at rest

2026-11-02. Task: `./kit plan F7`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

S206's survey found the bones' search lists free09's sleeping props (their `f.col`, category 0x18, in the
track's static tree) and S240 put only the awake ones in the bones' scene. Read in the interpreter on
recording2: a sleeping prop is listed at a regather wherever the query's cells reach it (flag bit 0x08
clear: no bound), filed in the tree after the plants; the sphere test is the same mesh pair; the strike's
listener wakes the prop and pushes nothing; the push-out is the moving path against its (still) body, as
S240's. Ported as `Statics::knock_keys` + `BikeSearch::gather_knocking` (the list) and
`KnockedProp::resting` (the scene, offered at its place in the walk), fed by `Down::step_posed_among`.
The list (6,000 gathers, order and distances), the sphere pass (20,000) and the wake are bit-exact with
the original. Details in `1-props-at-rest-FINAL.md`.
