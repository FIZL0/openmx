# P206 — online put back and trick start

2026-11-02. Task: `./kit plan P206`.

Online, nothing set a seat's `said.respawned` or `said.started`. The session runs the riders' frames and
keeps none of what they report. So the biplane pilot's lean never snapped on a put-back, and the online
`put back` and `trick:` lines never printed. `audio::Said` now reads both off the drawn ride, the way it
reads wrecks (P184). It also reads the seat's plane. A new test checks this on the real two-machine
session with rollbacks: a plane put back with L1+R1 is reported once, on the frame it is first drawn back
in its bay, and the pilot's lean snaps there. Details are in the numbered file.
