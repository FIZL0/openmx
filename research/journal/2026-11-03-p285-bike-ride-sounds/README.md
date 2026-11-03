# P285 — bike ride sounds

2026-11-03. Task: `./kit plan P285`.

This task listed every sound the original plays for a ridden bike that is not wrecking, and checked
which of them the port plays. The table is in `research/bike-sounds.md`.

The disc has no samples for gear shifts, backfires, rattles, skids, roost, splashes or surfaces, and the
bike has no landing or suspension sound (P66). So the port already matches on all of those, on the engine
states, the clutch, the limiter, the drive-on loop and the Cannonball.

- **Missing:** the material table's collision sounds (prop, fence and bike hits). Filed as an F task.
- **Different:** the port cheers on a trick landing, which the original never does. The original cheers
  and boos only on race place changes. Neither can be heard, because no track turns its crowd on. Filed
  as a low C task.
- **Checked by:** a matched filter over S109's run2.mp4. It finds the wreck's clips and none of the
  landing, suspension or hit clips.
