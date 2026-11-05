# C27 — openmx host floats

2026-11-04. Task: `./kit plan C27`.

Sorted the 86 openmx files naming ps2_float: nearly all use F/Vf only as game_sim API types and stay;
audio, particles and the camera target blocks are compared bit for bit and stay. chase::ride_out, the HUD preload meter and the render regions wrap period went to host f32. Frame time unchanged (noise). FINAL in 2.
