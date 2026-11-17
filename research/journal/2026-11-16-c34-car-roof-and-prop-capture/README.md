# C34 — car roof and prop capture

2026-11-16. Task: `./kit plan C34`.

Captured the original resetting the buggy stuck on its roof (free07) and the buggy striking free06's
balls, and replayed both through the port. The roof capture showed the reset runs inside the car's
measure, ahead of the speeds and frame; with that fixed in `car.rs` the whole roof drive is bit-exact,
row by row and carried. The ball drive is bit-exact up to the strike (49 frames) and every ball wakes on
the original's frame, but the struck ball's answer is about 2 % off; that gap is S340.
