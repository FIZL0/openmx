# C47 — rerecord bones

2026-11-22. Task: `./kit plan C47`.

C47 re-recorded S74's five bones captures and S201's two riding-entry captures, which were lost when the
contexts merged. It used generated `.p2m2` tapes on PCSX2 copy 2, all with torn 0 and gaps 0. riding_spring
fails on them, and fails the same way at S201's own commit, so the new rides differ from the old ones and this
is not a port regression. The captures are parked in `context/c47/` so the shared gate stays green, and a
follow-up is filed. Details in `1-rerecord-bones-PART.md`.
