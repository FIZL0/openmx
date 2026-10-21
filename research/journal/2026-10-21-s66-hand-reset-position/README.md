# S66 — hand reset position

2026-10-21. Task: `./kit plan S66`.

Before this task, the L1+R1 put-back sent the bike to the ride's start line, or to where the wreck body
stopped. The original never does either. It keeps a put-back record: once a tick, after the physics, it
looks under the bike for a clear, level spot and keeps it. Both the chord and the wreck clock set the
bike down on that record as it stood the tick before. The port now keeps the same record
(`Ride::keep_back`, which includes its clearance ring of fourteen probes) and puts the bike back on it
(`Ride::place_back`). The record matches the original bit for bit on 1975 captured ticks on free06 and
free09, riding and down. All nine captured put-backs land on the original's point to the bit. The shell
no longer slides the bike and rider from the wreck to the put-back spot on that tick. The rider's pose
through the ghost is not captured yet; that is S69. Details are in `1-hand-reset-position-FINAL.md`.
