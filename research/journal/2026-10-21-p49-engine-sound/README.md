# P49 — engine sound

2026-10-21. Task: `./kit plan P49`.

The bike did not sound like the original because P42 read the sound archive's directory one record out
of step: every name played the file before it (the 125's idle was a high rev, its 2.2 s decel a 0.25 s
clutch), and so did every other sound and song. Fixed, with the original's own sample lengths as the
proof. The engine note is now the original's state machine ported line by line, matched against the
original function over 1000 generated runs, and it follows a PINE-logged ride of the original on 1199
of 1199 frames. The clutch hand is the lever, as the original's is. Recordings of both rides and
their spectrograms are under `context/p49/`; nobody has listened to them side by side yet.
