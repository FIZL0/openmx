# D1, D2 — the disc

2026-10-15. Tasks: `./kit plan D1`, `./kit plan D2`.

Identified the dump and filled in `kit.toml`, then listed and extracted the disc. The disc is unusually
simple: four files, one of which is 1.4 GB and holds everything the game ever loads. `./kit doctor` is
clean apart from the BIOS, which is the one thing I cannot produce — see `2-the-bios-READY.md`. D1 is
blocked on it; D2 is done.

Every address and offset in this repo is only true of the dump recorded in `1-the-disc-FINAL.md`.
