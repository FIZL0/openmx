# S429 — owner drops jump watch in a wreck

2026-11-18. Task: `./kit plan S429`.

The race card's jump watch (0x80000) now drops while its rider is down and comes back at the respawn,
as the original's ride state machine does. A new owner step before the card feed does it; the feed
stays bit-exact to its oracle. C31's sx01 race test now compares the bit (it was masked).
