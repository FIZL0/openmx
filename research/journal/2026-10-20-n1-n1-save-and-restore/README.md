# N1 — n1 save and restore

2026-10-20. Task: `./kit plan N1`.

Save, restore, hash and resync bytes for the whole ride (`game_sim::snapshot`), with the constants
behind shared `Arc`s the hash skips, proved save/step/restore/step-equal, through a transcription of
ggrs's sync test, and bit-identical on x86-64, aarch64 and wasm32 for a four-rider ride. Blocked on S54:
the "players" are independent `Sim`s until the multi-player ride exists, and its state then needs the
codec and re-pinned vectors. Details in the numbered files; the newest one's suffix says whose move it
is (FINAL · PART · READY).
