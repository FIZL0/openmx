# S10 — s10 air

2026-10-15. Task: `./kit plan S10`.

The five steps the bike's update runs for a jump — which wheels touch and the crossup angle, the preload
and the backflip off a lip, the moment the stick commands in the air, and the wreck test a landing has to
pass — ported bit-exact into `crates/game-sim/src/air.rs`, together with the game's double-precision arc
cosine and the 30000-entry table it builds at startup. Proved in `crates/game-sim/tests/air.rs`: the table
entry for entry against the game's own builder, and each of the five steps over 8000 generated states,
comparing the whole 4.4 KB bike object word for word after the original and after the port. Three last-bit
differences came out of those two comparisons, including the one in the backflip torque decay that nothing short
of it would have found. Cut, and listed in the journal: the wreck test's hitzone prologue and its
bike-against-bike loop, and the orientation sampling the contact step opens with.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
