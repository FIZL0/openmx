# F15 — native sim

2026-11-02. Task: `./kit plan F15`.

A concept build whose simulation runs on the host's plain IEEE `f32` instead of the PS2's arithmetic, for a
faster game that feels close enough. It landed on main switched off: ps2-float's `native` feature (default
off, so exact builds and their tests are unchanged), openmx's `native-sim` feature that turns it on and says
so (menu corner, new `--version`, the first line printed, the iPhone log's first line), an online build id
that makes a native build and an exact one refuse each other, and `tools/build.sh linux --native-sim`. On
this desktop the sim takes about 0.55–0.65× the exact time. 191 of game-sim's 1028 tests go red with it on,
nearly all by 1–200 ulp. A ridden tape agrees for about 4 s and then rides its own way. No Deck number and no
hands-on feel yet: the Deck was not reachable, and feel needs a person. Details in
`1-native-sim-FINAL.md`.
