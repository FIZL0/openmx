# F42 — trick gear net virtuals

2026-11-04. Task: `./kit plan F42`.

F39 left four bike virtuals running in the interpreter: the gear (vt +0x358), the trick step (vt
+0x4e4, which the lean calls), the net check (vt +0x3e8) and the net unpack (vt +0x3f0). All four are
now ported in `game_sim::handling::update`, and `tests/mxvatv_bike_ride.rs`'s `UpdateInRam` serves them
with the ports. A 10 000-bike oracle shows them bit-equal to the originals, and four mutants fail it.
Both replays stay bit-exact, with the gear and trick step called about 900 times each. The net pair is
proved only by the oracle, because no replay has a remote bike. Details:
`1-trick-gear-net-virtuals-FINAL.md`.
