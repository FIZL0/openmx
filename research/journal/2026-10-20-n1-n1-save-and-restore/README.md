# N1 — n1 save and restore

2026-10-20. Task: `./kit plan N1`.

Save, restore, hash and resync bytes for the whole ride (`game_sim::snapshot`), with the constants
behind shared `Arc`s the hash skips, proved save/step/restore/step-equal, through a transcription of
ggrs's sync test, and bit-identical on x86-64, aarch64 and wasm32 for a four-rider ride. Part 1 rode four
independent `Sim`s while S54 was not in; part 2 moved the tests onto S54's one session (`add_player`,
`step_all`), shared the other players' `.phy` behind an `Arc` too, and re-pinned the vectors. Details in
the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
