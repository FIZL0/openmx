# S316 — s316 free ride seek load

2026-11-13. Task: `./kit plan S316`.

A free ride now loads its AI seeker world at the map's load. `Sim::seek` holds it, and
`Sim::load_seek` builds it by calling `ai::seek::load` (S313), with the Sim's riders as the owners
(player 1 is kind 1, the others kind 2) and the slot count. The shell calls it after the freestyle
event. The test checks the owners and the slot count (the activity's players: the riders and one
more) against recording2's image, and checks that the rest is S313's proven load. Nothing steps the
world yet; that is S323. Details are in the numbered files; the newest one's suffix says whose move it
is (FINAL · PART · READY).
