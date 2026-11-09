# S286 — rider pose settle

2026-11-08. Task: `./kit plan S286`.

This task ported MX vs. ATV's two rider virtuals that the mode machine's respawn and follow call:
vt `+0xd0` (the ragdoll posed off the scene) as `rider::pose` and vt `+0xcc` (the world's bodies
settled) as `rider::settle`. Both reuse code that was already ported. The pose does what MX
Unleashed's ragdoll pose does (`ragdoll_pose::pose`), instruction for instruction. The settle's
per-body commit is `wreck::body_commit`, the same slot already proven on the bike. A new oracle runs
12,000 cases of both functions whole in the interpreter against the port, and every written word
matches. Details: 1-rider-pose-settle-FINAL.md.
