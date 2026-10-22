# P107 — markers per machine

2026-10-22. Task: `./kit plan P107`.

Session markers on every machine, a set each. The bike and the plane each have five slots and a
selection of their own, swapped in by the machine that is up, so the slot display, the radar and the
ghosts show that machine's markers only. A plane marker keeps the whole flight; Up puts it back bit for
bit, unwrecked and in the air. The plane's d-pad is freed for the markers under every scheme. Each
machine's set is saved in its own file per track and profile (the plane's named `biplane`), and tapes,
lobby picks and `--peer` wires carry both. Proved by four new tests in crates/openmx/tests/markers.rs;
not verified on screen, and the plane has no ghosting to apply (see 1-markers-per-machine).
