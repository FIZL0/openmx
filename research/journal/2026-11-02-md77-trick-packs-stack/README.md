# MD77 — trick packs stack, and Cross and Square combine

2026-11-02. Task: `./kit plan MD77`.

The user's brief: we should be able to have several packs on. R1 is already a modifier, but Square and
Cross aren't yet, so Untamed can fill that gap. Options can enable several packs, which is how more
tricks get handled. The game can warn when there are too many for the buttons, but it can also take two
buttons at once like the original: Square and Cross together open up a lot more.

Now every installed trick pack has its own On/Off row on Settings → Ride, by its `mod.json` name. The
packs on stack: the first in folder order wins a stunt name two of them have. The two extras (Cross and
Square on the new layout) combine with each other and with the original's three, for 96 slots. When the
packs on have more stunts than that, the Tricks row and each lit pack row say how many go without a button.
Online rooms carry the packs on as comma-joined ids, with one hash over all of them.

See [1-stacked-packs-FINAL.md](1-stacked-packs-FINAL.md).
