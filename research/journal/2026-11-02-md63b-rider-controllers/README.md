# MD63b — MX vs. ATV's stand-upright, balance and roll/pitch

2026-11-02. Task: `./kit plan MD63b`.

Diffed SLUS-21104's three rider controllers (low-speed stand-upright, one-wheel balance, wheelie and
stoppie) against `move.rs` clause by clause. The arithmetic is MX Unleashed's in all three. What differs
is where the frames come from (the rigid body's quaternion, not the node's or the chassis' transform)
and that the yaw moments are added on every bike (no "has a transform" test). `handling/step.rs`
`stand_upright`, `balance` and `roll_pitch` call `move.rs` through those frames, and `ride.rs` uses
them on an MX vs. ATV ride only. Each is run against the original off `state04.ee.bin` in
`tests/mxvatv_step.rs`, 10,000 states each, and all three match word for word. Details in the
numbered file.
