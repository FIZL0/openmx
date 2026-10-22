# S62 — ragdoll moving contact

2026-10-21. Task: `./kit plan S62`.

S62 ported the bone push-out against moving things and bike_box's other-actor frame. Both are proved
word for word against the original on generated cases. The task also captured the wreck bike's whole
object by pointer chain, plus the broad phase's pools. With these, the original's own ragdoll tick
replays the side, back and tip wrecks bit-exactly through the put-back. A second front wreck replays
through its end. The first front wreck still parts 26 ticks in, for a reason not found. No truly slow
tip-over could be ridden. After S61 landed (part 2), the port's own actor runs each s62 wreck on its
own state, 2616 ticks to the bones at rest, bit-equal to the original's tick on every number. On
2029 of those ticks the original is also the capture. The bones' hits against the wreck bike's body
are answered through the port's moving push-out. Still open: a truly slow tip-over, and front past
its 2627.
