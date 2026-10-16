# V2 — v2 side by side

2026-10-15. Task: `./kit plan V2`.

The port's picture put beside the original's, from one save state: `moment.py` reads the human's bike
transform and the frame's own camera (`0x0046a540` / `0x0046a580`, both new) out of the state's EE RAM,
`openmx --moment` poses one frame from those numbers at 4:3 so the two projections are the same, and
`side_by_side.py` joins them. The geometry matches — place, pose, scale, camera, field of view, horizon;
what the port is missing is the rider, the HUD, the dirt texture, the sky, the shadows and two props, and
its terrain stops at the shell's radius. Nine differences, listed in `1`. The two tools are committed so
the next subsystem can be shown the same way; the images are under `context/shots/` and are not.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
