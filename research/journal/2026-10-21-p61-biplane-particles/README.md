# P61 — biplane particles

2026-10-21. Task: `./kit plan P61`.

P58's particles now meet the ground and take the wind, the way the original does. The terrain test is
part of the particle update: it runs on odd ticks, and only for particles whose emitter collides. On
the plane that is the wreck's chunks alone, switched on by the `.dat` entry's `HasCollision = TRUE`
(`wreckchunks.emt` has no `GroundLevel`). Such a particle bounces with `Restitution` and comes to lie
on the ground. Wind is added at emission, not in the update. The track's particle system holds a wind
of zero (read from two save states), so `Wind = T` adds nothing on the disc. The chunks still never
draw live, because the port's plane has no body contacts yet (P63). Details in `1-…-FINAL.md`.
