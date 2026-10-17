# P11 — the clutch that never comes out

2026-10-17. Task: `./kit plan P11`.

The brief's latch was half-paid before this session started — P8 had put the rider's lever back over
`clutch_in` every frame — and what was left was the other half of the same mistake: the port keeps
two copies of a word the original keeps one of. The original has the rider's clutch lever and the
input layer's rate-limited copy of it, and that second word is both what the engine reads and what
the idle rule stamps a 1 into; because it persists, the input layer's own rate limit is what lets
the clutch back out, at `ClutchOutRateLimit`, over the frames after the throttle opens. There is no
separate release rule and there does not need to be one. The idle rule now writes that word instead
of a private copy, so the clutch is fed out over a quarter of a second rather than dropped in a
frame. P10's test was also riding a bike with a 1.6 ft wheelbase — it took its wheel mounts off the
patched `BodyTireForcePosition` — and wrecking at 18 ft/s into a frozen rider with no wreck body,
which is why its failure read as "the bike will not move"; it rides the real bike now, and the same
test exists through the pad in `openmx`. The canonical platform vectors moved with the ride and were
regenerated.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
