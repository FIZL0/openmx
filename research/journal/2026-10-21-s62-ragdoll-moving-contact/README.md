# S62 — ragdoll moving contact

2026-10-21. Task: `./kit plan S62`.

S62 ported the bone push-out against moving things and bike_box's other-actor frame. Both are proved
word for word against the original on generated cases. The task also captured the wreck bike's whole
object by pointer chain, plus the broad phase's pools. With these, the original's own ragdoll tick
replays the side, back and tip wrecks bit-exactly through the put-back. A second front wreck replays
through its end. The first front wreck still parts 26 ticks in, for a reason not found. No truly slow
tip-over could be ridden. The port's own replay waits on S61's per-tick mode machine, so the task is
blocked after S61.
