# P31 — p31 the biplane takes gas

2026-10-20. Task: `./kit plan P31`.

The biplane takes gas. `Flight::frame` now runs the original's frame in the original's order (S41's list):
the body and wheels are built bit for bit as the save state has them, and the tire mount's speed is the
signed speed along the nose, not the airspeed. That was the bug: a parked plane rolled backward forever.
In the shell, `--vehicle biplane` flies. The plane is drawn at its body's pose with the prop turning,
the plane's camera follows it, and the HUD shows throttle and airspeed. On free06 it lifts off at about
7 s, climbs to 465 ft, is thrown back from the map's edge, and lands on the heightfield. It flies over
the heightfield (or `Flat`) alone, because S45/S46/S48 are open. No frame is compared with the original
(there is no biplane capture), so `BIPLANE_AGREES_THROUGH` stays 0. Details in file 1.
