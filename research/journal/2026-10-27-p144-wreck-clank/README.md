# P144 — wreck clank

2026-10-27. Task: `./kit plan P144`.

The bike's `[Wreck]` clank used to play once, at full level, on every wreck entry. It now plays as the
original's bike emitter does. At the entry it plays only when the riding impulse is at least
MinImpulse. While the bike is down, it plays on a wreck-body hit that touches at MinImpulse or over,
once the emitter is idle, at |impulse|/MaxImpulse of full volume. I proved this with a live capture of
the emitter's gates over S109's fall. In that capture the entry is silent, one clank comes at the
bike's touchdown at level 100, and the three later hits are quiet. The new tests replay those rows.
Details in 1-wreck-clank-FINAL.md.
