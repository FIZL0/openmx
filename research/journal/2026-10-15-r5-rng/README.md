# R5 — rng

2026-10-15. Task: `./kit plan R5`.

The game has no random number generator of its own: it calls the C library's `rand` — a 64-bit LCG with a
single state word — from 224 sites, seeds it from 35 more, and scales the result by 2⁻³¹ at 199 of them
to get a float in `[0, 1)`. All of that is now `crates/game-sim/src/rng.rs`, proved against the game's own
instructions over 10,000 consecutive calls from the seed the image ships, 100 seeds × 100 calls through
the original's `srand`, 10,000 uniform list picks and 2,000 random floats, each compared bit for bit —
and checked to be load-bearing by breaking the port twice on purpose. The gaps are in `1-rng-FINAL.md`;
the one that matters later is that nobody has yet looked at what the 35 reseeding sites seed *with*.
