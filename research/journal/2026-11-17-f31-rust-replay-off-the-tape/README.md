# F31 — Rust replay off the tape

2026-11-17. Task: `./kit plan F31`. Follows `../2026-11-03-f31-mxvatv-bike-replay/`.

F31 was reopened once F31a, F31b and F31c landed: the whole SLUS-21104 bike frame now runs in Rust and
replays C15 ride-2 and flip-1 bit-exact. That replay still took the pad reader's words from the capture
rows. This session feeds it from the tapes' ticks instead: every reader word but one (the flip hold), and
the wreck state the reader sees comes from the replay itself. Both rides still replay bit-exact. What is
left (the flip hold, the put-back spot's writer, and running the port in the app's ride) is filed as
F31d, F31e and F31f. Details in 1-rust-replay-off-the-tape-FINAL.md.
