# A1 — a1 state hash

2026-10-15. Task: `./kit plan A1`.

The state hash: `crates/game-sim/src/hash.rs`. One walk over the whole simulation state, which a state
type describes once (`impl hash::State`) and two sinks read — folded into 64 bits for a per-tick
comparison, or dumped field by field so that `hash::first_difference(&a, &b)` answers "which field first
differed" in one line (`ball.vel[1] 2.8499999 (0x40366666) vs 2.85 (0x40366667)`). `Sim::hash` and
`physics::Ball::hash` now delegate to that walk rather than keeping their own copy of the field list.
Proved by 16 tests in `crates/game-sim/tests/hash.rs` — the FNV-1a reference vectors, the state's byte
stream built by hand, one-bit sensitivity per leaf, dump/hash agreement, and the first-difference order.
One fix on the way: `state_hash`'s multiplier had a hex digit too many, so it was not the FNV-1a it
claimed; corrected while no fixture records a hash yet, which moves the hundred-tick constant.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
