# S22 — g loads read the impact accumulator

2026-10-16. Task: `./kit plan S22`.

The sideways g load grew frame over frame on a bike that was standing still, and crossed
`SideWreckSpeedLimit` about two and a half seconds after the spawn. The wreck test, the limit and
`g_peak` were each already proved, so the fault was in what they were being fed: the original's g
loads come out of `chassis+0x170`, an *impact* accumulator that only a bottoming-out suspension ever
writes and that the ground step clears once a frame, while the port was feeding them `chassis+0x70`,
the chassis' own acceleration — a quantity that is never zero and that the port was unrotating once
per frame instead of once per substep. A live capture of 300 frames of a standing bike settles it:
`+0x170`, `+0x180` and the three g lanes are *exactly* zero on every row while `+0x70` never is. Three
smaller errors turned up with it and are fixed in the same change: the wheels were mounted at
`BodyTireForcePosition` rather than at the model's `TireFront`/`TireRear` nodes, the suspension probe
was two tire radii long instead of one, and both damping rates were being taken as absolute
coefficients rather than as the ratios of critical damping the setter at 0x0025ac50 treats them as.
A standing bike now holds a g load of zero, and on the capture's own track and bike the port settles
within 0.6% of the original's travel and 0.5% of its load, front and rear. Details in the numbered
file.
