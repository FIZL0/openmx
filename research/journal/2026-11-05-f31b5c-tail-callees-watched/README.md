# F31b5c — tail callees watched

2026-11-05. Task: `./kit plan F31b5c`.

The vehicle update's, wreck's and update callees' calls that F31b4a, F31b4b and F39 still handed the
interpreter are now watched on every row of C15 ride-2 and flip-1. Each call is checked for any word
of the bike's physics blocks it changes. The engine's and wheels' resets are now Rust
(`wreck::engine_reset`, `wreck::wheel_reset`, with oracles), and the put-back's sync is F39's port.
Every other callee was seen to leave the physics blocks alone, except the put-back's ground reset on
flip-1 row 193. That reset is split off as F31b5e. The bike-object words some callees write are filed
as F31b5f. Details in `1-tail-callees-watched-FINAL.md`.
