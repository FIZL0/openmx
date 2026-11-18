# MD198 — hazard time wall

2026-11-17. Task: `./kit plan MD198`.

The brief said hazard-time.csv's replay was thrown at the map edge on row 238. That was already gone:
MD168b seeds the quad's edge-armed word from the image, and it is off in Free Ride. The replay ran clean
to row 254, then broke on row 255 because the original hits Rocksbury's stadium wall, which the test's
world did not have. The test now places the scene's collision list from the disc. Hazard-time replays bit
for bit to row 254 (was 190). The wall strike from row 255 on is MD198a.
