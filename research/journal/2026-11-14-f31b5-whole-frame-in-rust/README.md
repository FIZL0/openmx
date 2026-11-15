# F31b5 — whole frame in rust

2026-11-14. Task: `./kit plan F31b5`.

The bike's whole frame with no interpreter, on C15 ride-2 and flip-1. Both replays run bit-exact with
every port switched on, but the interpreter still runs 546k (ride-2) and 246k (flip-1) instructions: the
emitter batch send, the atv heap, the wreck check, the wreck's hand-outs and the rider leaf's sound and
rand. Those are F31b5q..v; this task is blocked after them. Details in 2.
