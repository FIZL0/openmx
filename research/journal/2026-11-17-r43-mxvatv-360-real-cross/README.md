# R43 — mxvatv 360 real cross

2026-11-17. Task: `./kit plan R43`.

The 360's clips are not started by the 360 step. The stunt id it stores is read by two animators,
each with its own fade. The rider's idle layer (already ported by F62a) saves the pose and fades out
of it linearly at 2 a second: 0.5 s in, and 0.5 s out at the next state's rate. The bike's animate
does the same at 3 a second: 1/3 s in and out. Proved by running SLUS-21104's own code on state04 in
`tests/mxvatv_stunt_cross.rs` (3 tests). The port change is filed as MD248. Details in
1-mxvatv-360-real-cross-FINAL.md.
