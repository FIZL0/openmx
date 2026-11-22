# P348 — phone log after p342

2026-11-16. Task: `./kit plan P348`.

Ride world_mxu on the iPhone with a build that has P342 and name what the slow frames spend their time
on. Done on 2026-11-21 by driving the phone from omarchy-desktop (build 5260, ~6 minutes, 38 joins, two
trips away). While riding no frame went over 40 ms, all led by wait (missed refreshes). Every present-led
frame over 50 ms sits at going away or coming back, ~105 ms each: the away rest (up to 100 ms, `AWAY_REST`)
runs inside the draw and is billed to present. Coming back uploaded 28 models at about 1 ms a frame on
average; the first frame back's upload is only bounded (under 7.69 ms and 1.14 ms), not read. The marker
jump could not be done by touch (no d-pad), and synthetic touches do not open the app switcher, so the trips
away were Home and back. Details in
`2-phone-ride-build-5260-FINAL.md`.
