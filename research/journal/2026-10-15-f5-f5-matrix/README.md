# F5 — f5 matrix

2026-10-15. Task: `./kit plan F5`.

The eight matrix and quaternion helpers everything with a bone or a camera is built out of —
the 4×4 product, the point transform (one point and a run of them), the look-at frame, a quaternion
into a matrix, the two axes of one on their own, a matrix back into a quaternion, the angular-velocity
derivative, and the spherical interpolation. All eight are in `crates/game-sim/src/math.rs` and all
nine entry points are proved bit for bit against the original over 10,000 generated inputs each in
`crates/game-sim/tests/math.rs`. Nothing is approximate and nothing is left unproved; the one thing
not carried over is the five larger composite helpers that sit beside them in the same region, which
were never in scope. Details, and the two places where the first draft's last bits were wrong, in
`1-f5-matrix-FINAL.md`.
