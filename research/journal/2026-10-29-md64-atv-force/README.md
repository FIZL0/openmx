# MD64 — atv force

2026-10-29. Task: `./kit plan MD64`.

MD64 ported the quad's three listeners from SLUS-21104 into `crates/game-sim/src/quad.rs`: the
PreProcess (`ground_head`), the Force body (`ground_step`) with its four-way contact distributor
(`wheel_contacts`), and the PostProcess (`air_state`). Each is proved bit-exact by a 10,000-state oracle
in `tests/atv_quad.rs`. The bike, engine, wheel and vehicle calls they make are not ported (MD63, the
wheel port). They are handed out as `quad::Call`s to a `Moto`, and the oracles check their order and
arguments. The update's tail and the tire .phy reader were split off as MD64a and MD64b.
