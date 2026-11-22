# S134 — wreck into a bike

2026-10-28. Task: `./kit plan S134`.

The wreck bike's search handed its world no other riders' bikes. Part 1 failed to capture a wreck into
free09's AI bikes, because they dodge. Part 2 found one already captured: S93's short split-screen ride,
where P2's wreck bike tumbles into P1's bike on 5999–6001. The original, run whole in the interpreter from
S93's image, matches the capture. `WreckBike::tick_ridden` hands the riders' bikes to the search, with each
rider's record and chassis as the other side. The port's wreck bike replays the original's body bit-exact
for 42 ticks, and without the bikes it parts on the first. Not yet called from the 2p ride (follow-up).
Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
