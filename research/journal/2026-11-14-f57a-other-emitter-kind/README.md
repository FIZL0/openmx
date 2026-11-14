# F57a — other emitter kind

2026-11-14. Task: `./kit plan F57a`.

The first emitter kind's other branch (definition option bit 0x40) was ported to Rust as
`game_sim::handling::emitter::other`, replacing the `Call::Other` hand-out. It is proved by a
20 000-case oracle against SLUS-21104's own step. The flip-1 and ride-2 every-port replays pass with it
in Rust. Only the batch's send is still handed out. Details: `1-other-emitter-kind-FINAL.md`.
