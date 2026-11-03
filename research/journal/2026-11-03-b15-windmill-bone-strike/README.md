# B15 — the windmill's strike on the port's bones

2026-11-03. Task: `./kit plan B15`.

From S211's south3 image, the windmill strikes the port's ragdoll on tick 5 and flings it. I
re-captured the original from the same state with its bones moved. Its bones are never struck: they
ride a tick on the thrown bike, enter the wreck carried away from the windmill, and go with the wreck
body at no more than 149 ft/s. The port's bones enter at rest at the windmill's south face (B19's
entry). Two of them touch the windmill's body, which has never stepped, and its mover (place × 30)
flings them. With the body stepped, the same bones touch and stay slow. So the strike is B19's, met
by S167's unstepped body. Nothing in the simulation changed.

Test: `tests/replay.rs the_windmill_strikes_the_ports_bones_left_at_its_face`.
