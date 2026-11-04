# F39 — vehicle update callees

2026-11-04. Task: `./kit plan F39`.

The eight callees the bike's vehicle update still ran in the interpreter (fade, lean, prepare, ride,
sync, snapshot, network, parts) are now `game_sim::handling::update`. Each is proved against the
original over 10 000 random bikes, and both replays stay bit-exact with them in the tail. The leaf check
found that the second "leaf", `0x002cf9f0`, is a manager. Its walk steps a rider whose physics writes
words of the bike's record (+0x870..+0x8a0, +0x920..+0x980). The test pins that exact set, and a
follow-up is filed. The first leaf leaves the bike alone. Details: `1-vehicle-update-callees-FINAL.md`.
