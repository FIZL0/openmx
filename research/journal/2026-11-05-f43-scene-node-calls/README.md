# F43 — scene node calls

2026-11-05. Task: `./kit plan F43`.

The bike's vehicle update still handed ten scene-node calls to the interpreter: place, set, spin, aim,
frame, part from frame, position, axes, point into frame and rotate. They are now Rust
(`game_sim::handling::node`). A 10 000-forest function oracle proves them against the originals, and a
100 000-angle oracle proves the spin's sine and cosine. C15's ride-2 and flip-1 replays stay bit-exact
with the ports serving the update. The anim controller's stop/play/blend (F43a) and the record's search
and attachment move (F43b) are split off, each a deep tree of its own. Details in the numbered files;
the newest one's suffix says whose move it is (FINAL · PART · READY).
