# P270 — poof cut

2026-11-03. Task: `./kit plan P270`.

The user, on 2026-11-03: "when poofing it should reset your character without animation, right now it
teleports you smoothly and causes visual issues especially when tricking". A put-back (the L1+R1
chord, the wreck clock's reset, the wreck page's) already cut the picture to the new spot (S74).
After the cut, though, four things carried the old pose on:

- the riding ragdoll sprang the rider over from the trick (S90);
- the bike's own clip faded back from the trick's pose for a third of a second, and the grips and the
  rider held to them slid with it;
- after a wreck, the body was drawn in its wreck pose, then its rolling pose, for a tick each;
- the camera chased the bike down from where it had been in the air.

A new switch, `Rider::cut_put_back`, is off in game-sim (the original's behaviour) and turned on by
the shell. With it on, the poof's first drawn frame shows the riding pose at the new spot, and the
camera is cut there too. Entry 1 has the details.
