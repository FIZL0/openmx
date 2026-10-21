# V29 — biplane camera

2026-10-21. Task: `./kit plan V29`.

The plane's camera was "far too close". It is the same race camera as the bike's on the plane's own
`BiplaneCPresets.ini`, and that part was already exact; what was wrong was the point the shell handed
it. The original's plane writes its camera block with the body's centre of mass where the last substep
began, three feet above and behind the drawn model's origin the shell was giving it, so the port's eye
sat ~8.7 ft from the plane instead of 13.1. Now the block is the original's word for word (point, rows,
velocity, lean, flag, speed), and the camera snaps on every plane reset as the original's does. A
replay of the original's own block, captured off `biplane.p2m2`, puts the port's eye within 0.004 ft of
the original's viewport on every untorn tick. Details in `1-biplane-camera-FINAL.md`.
