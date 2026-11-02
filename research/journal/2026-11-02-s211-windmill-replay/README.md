# S211 — windmill replay

2026-11-02. Task: `./kit plan S211`.

I replayed S167's two windmill captures, south3 and apex, through the port with free09's toys in the
replay world. South: the bike at rest beside the rustic windmill gets the from-rest throw on first
contact. The port matches it to the capture's six printed figures on tick 3, (7.79299, 18.6087,
133.41) ft/s, and wrecks on the same tick. Without the windmill there is no throw.

Apex: the port matches the fall onto the windmill's top through the wreck on tick 24. It also matches
the windmill's ~59,700 ft/s throw of the wreck body, step for step to 1e-3, through tick 41.

The 300.45/100.312 put-back is the off-map cannonball (S59, wreck 13). The port throws the same vector
as the original, but too early (8 against 14, 42 against 85), because the port's ragdoll main bone
reaches the map margin sooner. That is filed as R14. Test:
`tests/replay.rs the_rustic_windmill_throws_a_bike_as_the_original_does`.
