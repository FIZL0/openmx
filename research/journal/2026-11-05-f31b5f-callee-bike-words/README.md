# F31b5f — callee bike words

2026-11-05. Task: `./kit plan F31b5f`.

F31b5c's census found four interpreter callees that write bike-object words but leave the physics
blocks alone. Each is now either ported or shown unread on C15 ride-2 and flip-1:

- The trick hold (`update::trick_hold`) is ported, with an oracle. The wreck test reads its fraction
  `+0x14e4`.
- The settle (`wreck::settle`) is ported, with an oracle. Besides `+0x1754`, it zeroes the
  wheelie/stoppie clocks, which the frame reads.
- The kind set's `+0x9a8` and the event posts' `+0x460`/`+0x468` are never read. Both replays stay
  bit-exact without either callee.

Only the posts' store into the flags at `+0x444` is left open. It writes back the value already there
and is read by the frame; it is filed as F48. Details are in `1-callee-bike-words-FINAL.md`.
