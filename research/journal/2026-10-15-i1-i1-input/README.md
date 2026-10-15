# I1 — i1 input

2026-10-15. Task: `./kit plan I1`.

The input layer is ported: `crates/game-sim/src/input.rs` keeps the pad the way the game does — sixteen
controls per pad with `held`, a strength, and the two millisecond timers that are the only record of
*when* something changed, plus the one-frame event queue the front end reacts to. No "pressed this
frame" bitmask, because the original has none: the edge is the event, and after the dispatch only "held
with the held timer at 0" remembers it. The timers turned out to be integers with an unsigned float
round trip every frame, so they count a flat +33 ms at the 30 fps cap rather than accumulating a
fraction — the one thing I had predicted wrongly, and the one a port would get wrong silently. C5's open
question is answered: several controls changing in one frame are queued in **control index order**, not
press order. Proved by a 220-frame recording replayed row for row with every word of every control and
every event compared (zero mismatches, 219 frames), by 98 oracle calls against the game's own thirty
instructions for the timer arithmetic, and by the control map read out of a RAM image. Done; see
`1-i1-input-FINAL.md`, whose "Not verified / not 1:1" list names select's bit, the uninitialised timers
and the two no-event branches.
