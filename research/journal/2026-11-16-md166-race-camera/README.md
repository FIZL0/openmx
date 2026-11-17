# MD166 — race camera

2026-11-16. Task: `./kit plan MD166`.

The task asked which camera SLUS-21104 hands a machine in a race, assuming the `RC_` files. It is not
the race camera: every race and free ride rides the follow camera on the machine's `FC_` file, which
MD82b already ported as `game_sim::camera::ModFollow`. The race camera is Hill Climb's alone, so nothing
new was ported and the shell (already on `ModFollow` in races) is unchanged. Two new captures, an Open
Class race behind the ATV and behind a trophy truck, replay bit for bit in `tests/mxvatv_camera.rs`.
The hill-climb camera is parked as MD185 (later). Details in the numbered files; the newest one's suffix
says whose move it is (FINAL · PART · READY).
