# MD248 — mxvatv 360 cross from the original

2026-11-18. Task: `./kit plan MD248`.

The port's 360 now crosses in and out the way R43 read the original. The rider fades out of the saved
pose linearly at 2 a second (half a second), the bike at its own 3 a second (a third), and the clips play
at the stunt clock: the rider's over the row's 1.5 s, the bike's over its clip. MD78's quarter-second
smooth step is gone. Proved by running the port beside SLUS-21104's own rider layer, bike animate and
stunt clock on state04, every tick bit for bit (3 new tests in `tests/mxvatv_stunt_cross.rs`). The
original ends the stunt at 1.5 s, which the port does not; filed R47. Details in
1-mxvatv-360-cross-from-the-original-FINAL.md.
