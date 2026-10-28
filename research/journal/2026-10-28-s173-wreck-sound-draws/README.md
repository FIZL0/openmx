# S173 — wreck sound draws

2026-10-28. Task: `./kit plan S173`.

The wreck's sound picks past the entry, in game-sim: the rider's voice crying `[IMPACT]` on the thrown
body's touches and the bike's `[Wreck]` clank at the entry and on the wreck body's hits, each on a busy
clock of its clip's length plus the streamed tail, so their `rand()` draws stay in step. Both frames and
the entry clank match the original over 10 000 generated cases each in the EE, and driven off a live
capture of S109's reason-11 fall the port picks on the same three ticks as the original (Fall 3726,
clank and IMPACT 3792) and nowhere else. The voice's tail, the port's body inputs and the in-tick draw
order are not proven (see 1). Details in the numbered files.
