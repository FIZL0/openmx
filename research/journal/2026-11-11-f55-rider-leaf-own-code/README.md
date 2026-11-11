# F55 — rider leaf own code

2026-11-11. Task: `./kit plan F55`.

F55 asked for the rider leaf's own code in Rust. F31b5i had left three pieces of it in the
interpreter: the rider's frame around the mode machine, the copy after it, and the extra step. All
three are now in `rider.rs`, put together with F31b5i's `wreck_spots` as `rider::listener_step`.
The rider leaf in `tests/mxvatv_bike_ride.rs` now calls that function. It still runs the world step
and the rider's vt `+0x148` in the interpreter: those are S280's and are not the leaf's own code.
C15's ride-2 and flip-1 stay bit-exact, with the RAM alike as before. A new oracle covers the arms
C15 never takes: 3 × 20 000 cases, and two mutants fail it. Details: `1-rider-leaf-own-code-FINAL.md`.
