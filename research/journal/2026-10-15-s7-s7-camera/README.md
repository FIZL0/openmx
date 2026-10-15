# S7 — s7 camera

2026-10-15. Task: `./kit plan S7`.

The camera for normal play — the 160-byte preset record the disc ships one of per camera, the look-ahead
point it aims at, the eight one-pole lags that make it follow instead of snap, and the limits on its
yaw, pitch, distance and field of view — ported into `crates/game-sim/src/camera.rs` and proved in
`crates/game-sim/tests/camera.rs` with twelve function-oracle proofs, 4000 generated cases each. The
brief's gating question first: the camera does **not** feed the simulation — it reads the rider and the
viewport, writes only its own fields and a view, and reaches neither the random numbers nor the frame
clock — so it was allowed to match by eye. It is bit-exact anyway, everything except the startup sine
table, which needs a double-precision `sin`/`cos` this project does not have yet. Three things worth
knowing before reading the code: the position routine computes a yaw-driven sideways swing and stores
it where nothing reads it (proved dead, left out); this class's ground probe is a `return 0.0f` stub, so
the look-at's ground lift only ever turns a negative zero positive; and the reverse camera is dead data
on the retail disc, because not one of its four keys appears in the camera file. The one bug worth
repeating is why a presentation module was worth making bit-exact: a last-bit difference in the look
swing's angle, from grouping two multiplies the way Ghidra prints them rather than the way the
instructions do, came out as 37 ulps in the look-at point. What is still not 1:1 — the sine table, the
collision push-out, the camera-file reader and two preset keys with no reader — is in the numbered
file's last section.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
