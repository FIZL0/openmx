# S278b — rider mode machine

2026-11-05. Task: `./kit plan S278b`.

S278b ported the rest of MX vs. ATV's rider mode machine as `game_sim::handling::rider::modes`. That is
the mode choice, the switch between modes (respawn, into the wreck, out of it, into 1/3/4), the pose
list placement, the hold and free part switches with their fades, the lean, the follow, and the
frame-end copies, around S278's walk and clear loop. Its outside calls (the riding reset, the wreck
entry, the rider's pose and settle virtuals, the parts' on/off virtual, rand, and the wreck arm's body)
are behind a `rider::Rider` trait. An oracle runs the whole original function against the port on
60 000 drawn frames of state 4's rider: every written word and every call is the same. The callees are
too big for this task and are filed as S279–S282. Details: `1-rider-mode-machine-FINAL.md`.
