# C10 — 2p capture

2026-10-26. Task: `./kit plan C10`.

Captured the user's 2p recording, `context/recordings/split/2p.p2m2` (Pad2 = DualShock2), every tick on both
lanes: 12,917 rows at 60 Hz (the state ticks at 60 Hz, one tape field a tick), no gaps, with `watch/2p.toml`.
Indexed 17 wrecks, 17 ragdoll stretches, 4 world contacts, the near misses, 13 tricks, 4 backflips and the
clip stretches (`research/c10/index.md`), saved 137 pre-event states in `context/states/c10/`, and pinned
every event at its tick in `crates/game-sim/tests/two_p.rs`. The ride holds **no rider-vs-rider contact, no
truck ride and no prop hit**, on the kit's PCSX2 or on the recording's own v2.9.52; S157/S158/S159 need a new
recording. Details in `1-2p-capture-FINAL.md`.
