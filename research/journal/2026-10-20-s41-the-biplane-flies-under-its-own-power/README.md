# S41 — the biplane flies under its own power

2026-10-20. Task: `./kit plan S41`.

The plane's frame is wired in the original's order: the tires' preforce, the substeps (collision force,
autopilot, engine with its smooth noise, aero, the tires' force listeners), the body integrator, the
measure and the crash test. The recorded pad flies it through I6's mapping.

- Part 1 ported the smooth noise and put the pad on the plane. The rest was blocked on S38 and S39.
- Part 2 is the proof. Over the original's own ground query, the port's frame matches the game's
  `dynamics_world_step` bit for bit for 30 frames from C7's save state
  (`the_plane_flies_frame_for_frame_like_the_original`). The acceptance run now starts on the capture's
  second row (the first row is one frame stale), seeded off that save state.

`BIPLANE_AGREES_THROUGH` stays at 0. The run parts at tick 1 by a few ulp of `body.vel`, because the
game's frame does more than its world step. The contact search (S48) is the suspect. See
`2-…-PART.md`.
