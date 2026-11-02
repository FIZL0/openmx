# S234 — flip scores at start

2026-11-01. Task: `./kit plan S234`.

The user asked for front flips and back flips to show and count as soon as they start. The original
does not do that. Its rider tick (`./kit fn 0x27c890`) scores a backflip only when its four-stage
attitude watch sees the bike come round, no more than ~52° nose down. A flip that lands before that
scores nothing, and C8's film shows "BackFlip 10000" only after the rotation.

So this task adds a port's-own switch on the score card, `Score::flip_at_start`, and the app turns it
on. With it, a flip goes on the line the frame `Air::flipping` goes up and is paid once. A landing
banks it and a crash loses it. The port's front flip follows the same rules. With the switch off,
nothing changes, which S79's tests still prove. Details in `1-flip-scores-at-start-FINAL.md`.
