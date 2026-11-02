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

**Taken out (S250, 2026-11-02).** The user: "the instant start frontflip and backflip we did is bad,
match original instead of our new patch". S250 removed `Score::flip_at_start`, `turning`, `shown`,
`Sense::flipping`, the `Frame::started` it told, the app's switch, the HUD's `shown` check, P206's
online start read of `score.shown`, and `tests/flip_points.rs`. Flips score as the original's rider
tick does again: on the line the tick after the watch reads 3. P243's lockout stays.
`tests/score.rs` now checks that a flip landed at 300° banks 0, and that a ramp ride's flip shows
only once the watch comes round. The Sim size and riders' wire pins went back to 12088 and 13607.
