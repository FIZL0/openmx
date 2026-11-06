# S285 — rider wreck arm

2026-11-06. Task: `./kit plan S285`.

S285 ports the body of MX vs. ATV's rider wreck arm, which S278b left as one call (`Call::WreckArm`).
That is the impact aim and its force, the holds letting go, the nearby gather (still a call), the bike's
collidable released, the ground under the rider (MD76's query, run as it is), the three pose-target
clauses with their callee `0x004eedc0` ported, and the `+0x5d8` fade. It is now
`game_sim::handling::rider::wreck_arm`, and `modes` runs it. S278b's oracle runs the whole mode machine
with the arm no longer skipped: 60 000 frames, about 25 000 of them in the wreck. Every word and call
matches the original's.
