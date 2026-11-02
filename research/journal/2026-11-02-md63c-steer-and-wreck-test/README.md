# MD63c — MX vs. ATV's steering hinge and wreck test

2026-11-02. Task: `./kit plan MD63c`.

Diffed SLUS-21104's steering hinge (`atv_wheel_steer_integrate` and the dispatch around it) against
`wheel.rs`, and its wreck test (`atv_bike_wreck_test`, which folds MX Unleashed's state machine and
wreck test into one function) against `air.rs` and `state.rs`. The hinge differs only in its `0 + x`
sums: a −0 rate or angle comes out +0. The wreck test differs in five places: reason 15 comes first,
the 360's torque counts beside the backflip's, the trick endings' gate has no no-hander term, the hits
use 2.9/4.5 reaches with rider-only throws, and a scenery hit needs the g loads past their limits.
Reason 14 is the same as `state.rs`. All of these are ported in `handling/step.rs` and used on MX vs.
ATV rides. Four oracles in `tests/mxvatv_step.rs` run them against the original off `state04.ee.bin`,
and all four match. Details in the numbered file.
