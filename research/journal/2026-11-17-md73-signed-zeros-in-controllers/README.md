# MD73 — signed zeros in controllers

2026-11-17. Task: `./kit plan MD73`.

Reran MD63b's three MX vs. ATV controller oracles (stand-upright, balance, roll/pitch) over states
full of signed zeros and some `dt` 0. Stand-upright and balance matched. Roll/pitch did not: the
wheelie's pitch integral starts `0 + x` in SLUS-21104, so a `-0` came out `+0`. `step::roll_pitch` now
folds it on the ground. Three new 10,000-state oracles in `tests/mxvatv_step.rs` keep it.
