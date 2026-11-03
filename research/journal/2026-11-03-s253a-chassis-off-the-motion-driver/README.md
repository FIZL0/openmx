# S253a — the drawn chassis posed off the motion driver

2026-11-03. Task: `./kit plan S253a`.

**The finding.** S253's M + U row lands as it was probed. Each tick, a ridden bike is now posed off
the rider's `animation`, the motion driver's frame, through an `anim::Rig` over the bike's own clips
(`Chassis::ride`); the five drives run after it. On fly_and_collide's drawn-lean test, the root's roll
in hard turns goes from 0.888 to 1.019 of the original's and the lean from 0.920 to 0.960. At rest the
bike keeps `bike_air_Mid`'s 1.49° roll, measured at (0.0259, 0.9996, −0.0082), which is S8's root up.

The stance machine still runs beside the rig. It poses the states the motion driver's switch has no
arm for: pack extras and 360s, the pack's backflip clip, and a trick the bike has no clip for (V98).

The details and the figures are in `1-the-chassis-FINAL.md`.
