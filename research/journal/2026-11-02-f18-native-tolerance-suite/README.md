# F18 — native-sim tolerance suite

2026-11-02. Task: `./kit plan F18`.

Player builds ride ps2-float's `native` floats (F15), and the gate's proofs stay bit-exact. The native build
now has a regression net of its own. It is three `native_tolerance_*` tests in `crates/game-sim/tests/replay.rs`:

- the S67/S115 landings, held to S67's bound;
- the crash outcomes, held to the original's outcome on its tick;
- the S58/S106 save-state rides, held to a path bound in feet.

The gate builds them natively into `target/native` through a new gate.toml `[[variant]]`, and runs them in the
same pool as everything else. They pass on main. Raising gravity by 0.16% reds 6 of the 10 rides. Details are
in `1-native-tolerance-suite-FINAL.md`.
