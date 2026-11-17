# N23b — lend without whole copy

2026-11-17. Task: `./kit plan N23b`.

A riding tick no longer copies the whole collidable list after the path cars move the kept world: an
update keeps the world's generation (it only changes reach, which `lend` refreshes anyway), and each kept
world keeps its own spare list instead of two worlds taking turns with one. Proved by a new kept_world test
(lent lists = fresh build, one whole copy per build) and identical 1P/2P traces. Timing gain ~10%, noisy.
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
