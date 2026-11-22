# S503 — wreck bike gates

2026-11-22. Task: `./kit plan S503`.

The wreck bike's nearby list in the original ends with twelve race gates. S503 hands them to the port's
search (`WreckBike::tick_gated`), with the toys and the AI riders' bikes. The original never tests a gate:
every gate's nearby bit is clear. So the proof runs the original's node update in the interpreter on the
side wreck's state, with the bike thrown at a gate. With the bits as the game has them it flies through;
with the gate's bit set it strikes the gate. Both runs are bit-exact in the port, 78 ticks each. Details in
the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
