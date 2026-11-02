# S235 — rider push uphill

2026-11-01. Task: `./kit plan S235`.

The user saw the original's rider "helping the bike along" on a slow climb and asked for it. The
original has no push clip: the rider's anim set has none, and the selection machine has no slope arm.
What shows on a slow climb is V36's foot switch: under the stand speed (7.3333 ft/s), and not yawing
past 1.5 rad/s, both foot pins let go, the feet drop off the pegs, and the riding grid blends its
stand pose in under the same speed. A PCSX2 capture of a crawl up free09's ~26° hill shows exactly
that, with no slope or throttle threshold of its own. The port already had the switch and the blend.
A new rider_anim test replays the capture through it to the bit. The task also fixed one wrong input
to the switch: its one-foot lift read the grid's fore-aft mix instead of the request's sideways pose,
so a forward lean on a slow climb lifted the right foot in the port. Details in
`1-rider-push-uphill-FINAL.md`.
