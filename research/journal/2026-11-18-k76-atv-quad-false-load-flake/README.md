# K76 — atv quad false load flake

2026-11-18. Task: `./kit plan K76`.

`-p game-sim --test atv_quad` was filed as a load flake after S328c's merge gate. It races nothing: the gate
failure was MD245's deterministic red (the file rig's inverse-inertia bound), and the "passed alone" retest ran
in the runner's merge checkout whose `context/` lacked the newer folders, so all 123 tests skipped. Linked the
missing context entries there (machine setup, outside git), waited for MD245, and the test now passes for real
in a slot. No repo code changed. Details in the numbered files.
