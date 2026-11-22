# P379 — phone put back after p377

2026-11-21. Task: `./kit plan P379`.

A phone log of an sx01 race with wrecks and put-backs, from a build with P377 (v0.19.0, build 5648,
installed on the iPhone 15 Pro by the iFizBox rig). Nine wrecks and put-backs; every second's worst frame
stayed under 25 ms through them (21.23 ms at most, all wait-led; the one second over is the ride's first,
33.49 ms right after the load), no `gpu: first drawn` line, and one 5.40 ms
`gpu: slow submit` on the busiest frame with nothing new handed over. No code change needed.
