# C33 — free09 ai4 capture

2026-11-13. Task: `./kit plan C33`.

This task recorded free09 with four racers from a state taken at frame_counter 2. At that point the
three AI riders and the human still stand on the spawn line. The capture holds the four riders'
bodies (centre, quat, velocity, spin, and the raw body) and rand_state on every tick, for S12e's
replay. The ride was recorded three times at three emulator speeds. The rows two of the three agree
on make `free09-ai4-agreed.csv`, and the state is kept beside it. Scripts are in research/c33. Details
are in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
