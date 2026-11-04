# R30 — untamed lip stick

2026-11-03. Task: `./kit plan R30`.

This task found what writes the stick words in the pad record that Untamed's (SLUS-21701) bike reader
copies. The question was whether the lip's stick (the backflip's and the 360's `+0x12bc`/`+0x12c4`) is an
unlimited stick, apart from the "limited" `+0x1298`/`+0x129c`. If so, the port would have had to feed it
something other than the step's stick.

It is not. The input object fills all of those words from its device's analogue axes, one per bound action.
The shipped bindings put the lip's actions and the limited stick's on the same two axes, so the two pairs
carry the same numbers and neither is limited. The port's feed (the step's stick, which MX vs. ATV's limiter
leaves unsquared, with the d-pad applied) was already right, so `untamed::ride_backflip`/`ride_spin` are
unchanged. The one difference in the original is a player's second input device, and no shipped setup has
one.

A new test, `untamed_lip_stick_is_the_limited_stick`, runs the game's own record fill on the image and
checks the words. Details in [1-untamed-lip-stick-FINAL.md](1-untamed-lip-stick-FINAL.md).
