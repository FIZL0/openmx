# V8 — rider drawn

2026-10-16. Task: `./kit plan V8`.

The shell drew a bike with nobody on it: V4 read the rider's geometry off the disc and could only
stand it on the pegs in its bind pose, and the animation driving the simulation runs every frame
answered a blend that was thrown away. This task joined the two. `game_sim::rider::Rider` now keeps
the `motion::Frame` its driver answered; `game_render::rider::Body` is a rider's three files with
its clip table bound — 63 clips, all 24 gesture slots and all 3 airborne poses, by the rider's own
`rider_<stunt>` names rather than the vehicle's `bike_<stunt>` — and poses the body from that frame
through `Rig::driven`, the back half of V7's seam. The posed body needs no placement at all: its
clips put it on the bike themselves, within a third of an inch of the chassis's own anchors, so it
is drawn at the bike's transform and `bolt_onto` is left for a body with no clip. A 60 s ride draws
it every frame at ~59.8 fps with no panic, and a stunt's first and last frames are 4.8 apart at the
furthest joint. 462 tests pass, up from 457. What was not compared against the original — which is
everything visual — is in the entry's last section.
