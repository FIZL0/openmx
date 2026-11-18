# R40 — throw frame

2026-11-17. Task: `./kit plan R40`.

R24 left R14's south-bones replay ~0.7 ft/s low in y, built while the windmill's throw pulled. The
pull was turned by the wreck bike body as the step left it; the original turns it by the bike node's
matrix, which is the pose before the frame's last substep. Turned by the node, the bones' momentum gap
stays flat through the throw, and the worst bone at tick 23 drops from 0.71 to 0.54 ft. Details in
the numbered file.
