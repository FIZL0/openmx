# S309 — s12 ai path follow branch

2026-11-10. Task: `./kit plan S309`.

The race AI's path follower ported into `ai::follow`: the brain's path reach (vt+0x78), the route branch
of vt+0x88, the route follower vt+0x8c (drift, side step, node speed with its blend) and ai_tick_flag's
path branch with the hand-over to the target seeker. Proved bit for bit against SLUS-21104 on 12,000
generated nets (tests/race.rs) and on 1,203 branches of the sx01 race image's three AI riders
(tests/ai.rs). The first session finished it and blocked only on S308 landing; S308 is on main, so the
second session re-ran the gate and ticked it. Gaps are under Not verified in file 1; S311 and S281 carry on.
