# S6 — s6 motion

2026-10-15. Task: `./kit plan S6`.

Animation driving: which motion each rider state plays, how far into it a frame is, and how two of them
blend. Ported into `crates/game-sim/src/motion.rs` and proved bit-exact against the original over 10,000
generated frames each for the driving, the clip advance and the action-frame window, plus a port-only
test of what the numbers mean in frames at thirty a second. The one output the simulation itself reads is
the shown crossup the airborne blend writes, which the landing test judges. What a clip looks like —
channel sampling, the quaternion slerp, skinning — is the renderer's and is not here; see
**Not verified / not 1:1** in the numbered file for that and the rest.
