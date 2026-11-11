# S312 — ai tick callouts

2026-11-11. Task: `./kit plan S312`.

S312 ported the AI tick's callouts that S281 left behind `ai::Steps` (the base update, the stuck
rescue, the hit reaction and the countdown, the listener walk, the steering filter and the rider's
habits), each oracled over 10,000 cases against the PS2 ELF. The work is done, but the gate is red on
base in `mxvatv_car_frame` (MD132), so it is committed unticked. See the PART file.
