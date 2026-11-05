# F43a — anim controller

2026-11-05. Task: `./kit plan F43a`.

The rider's animation controller calls the bike's vehicle update makes every frame (stop, play, blend)
were the last three anim calls F43 left in the interpreter. They are now ported, with their channel
samplers, key decode and slerps, as `game_sim::handling::anim`. The update rig serves them with the port.
A 10 000-case oracle against the original matches word for word; the key tables match the image; both
C15 replays stay bit-exact. Details: [1-anim-controller-FINAL.md](1-anim-controller-FINAL.md).
