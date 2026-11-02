# S210 — wheels stand on the knocked props

2026-11-02. Task: `./kit plan S210`.

The ride's wheel probe now searches the knockable props' `f.col` meshes where their bodies stand. A
wheel on one sets its owner to the prop's collision object and reads the prop's motion through
`PropSet::under`, and the hit wakes the prop. Proved against the original in the interpreter. Details
in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
